use super::*;

/// An item sitting in an `Equipment` slot: *which copy* went on, and the
/// gear level its stat bonus was scaled for when it did.
///
/// The level is captured at equip time (see
/// `items::EquipmentStats::scaled_for_level`) — like a wild program's
/// zone-scaled stats, it doesn't retroactively change if the player breaches
/// deeper afterward; re-equip to pick up a newly unlocked level. That is
/// exactly why it sits *beside* `GearCopy` rather than inside it: the level
/// is a property of the moment, not of the copy, and a copy back in cargo
/// has no level at all.
///
/// **Storing the copy whole is what keeps the bonus symmetric.**
/// `Game::apply_equipment_delta` writes the bonus into `Stats`, so the
/// unequip has to subtract precisely what the equip added; every property
/// that scales it lives in this one value, so a path cannot subtract a
/// bonus computed from fewer properties than it added. See `GearCopy`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquippedItem {
    pub copy: GearCopy,
    pub level: u32,
}

/// What's currently equipped in each slot, on the player or on any program
/// they own — `Game::check_wearer` is the rule. Each slot's level-scaled
/// stat bonus (see `EquippedItem`, `Game::equipment_of`) is added directly
/// onto `Stats`/`Decompiler` when equipped and subtracted back on unequip —
/// mirroring how leveling directly mutates `Stats` elsewhere, rather than
/// maintaining a separate "base stats" layer.
///
/// Inserted on demand by `Game::equip` rather than at every spawn site: its
/// absence already reads as an empty loadout everywhere it is consulted, so
/// a program grows one only when it first wears something. Because the bonus
/// lives in `Stats`, **no stats operation may run while it is there** — see
/// `Game::gear_bonus` and `Game::strip_gear`.
#[derive(Component, Default, Clone)]
pub struct Equipment {
    pub weapon: Option<EquippedItem>,
    pub armor: Option<EquippedItem>,
    pub module: Option<EquippedItem>,
}

impl Equipment {
    pub fn slot_mut(&mut self, slot: EquipmentSlot) -> &mut Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => &mut self.weapon,
            EquipmentSlot::Armor => &mut self.armor,
            EquipmentSlot::Module => &mut self.module,
        }
    }

    pub fn get(&self, slot: EquipmentSlot) -> Option<EquippedItem> {
        match slot {
            EquipmentSlot::Weapon => self.weapon.clone(),
            EquipmentSlot::Armor => self.armor.clone(),
            EquipmentSlot::Module => self.module.clone(),
        }
    }
}

/// Player-only: every carried copy of gear that is *not* interchangeable
/// with a plain one, a row per `(GearCopy, qty)`. A copy earns a place here
/// by having been fused (`Game::fuse_item`) or by having dropped at a rare
/// tier (`Game::grant_gear_drop`) — `GearCopy::is_plain` is the predicate,
/// and it is the only thing that decides.
///
/// **`Inventory` is by definition the plain-copy store**, and that is the
/// seam that keeps this out of the production chain entirely: recipes,
/// `Stock`, `assembler_system`, hauling and banking read `Inventory` and
/// therefore cannot encounter a special copy, so none of them needs a tier
/// or rarity rule. That was true of fusion alone and stays true now that a
/// copy has two ways to be special — which is the whole reason rarity was
/// added as a property of the *copy* rather than of the item.
///
/// Keyed by value rather than by position. Two copies with equal
/// `GearCopy`s are genuinely interchangeable, so an index would identify
/// nothing the key does not — and it would be the positional-index trap
/// `BattleState::planned` documents, for no gain.
///
/// `EquippedItem` holds the same key for a copy that is worn rather than
/// carried; this ledger agrees with it rather than shadowing it, which is
/// what the per-`ItemId` predecessor got wrong.
#[derive(Component, Default, Clone)]
pub struct GearCopies {
    pub copies: Vec<(GearCopy, u32)>,
}

impl GearCopies {
    pub fn add(&mut self, copy: GearCopy, qty: u32) {
        // Saturating for the reason `Inventory::add` is.
        if let Some(row) = self.copies.iter_mut().find(|(c, _)| *c == copy) {
            row.1 = row.1.saturating_add(qty);
        } else {
            self.copies.push((copy, qty));
        }
    }

    pub fn count(&self, copy: &GearCopy) -> u32 {
        self.copies
            .iter()
            .find(|(c, _)| c == copy)
            .map(|(_, q)| *q)
            .unwrap_or(0)
    }

    /// Removes up to `qty` of `copy`, returning how many were actually
    /// removed. Drops the row at zero for the reason `Inventory::take`
    /// does — every screen lists these rows.
    pub fn take(&mut self, copy: &GearCopy, qty: u32) -> u32 {
        let Some(pos) = self.copies.iter().position(|(c, _)| c == copy) else {
            return 0;
        };
        let taken = self.copies[pos].1.min(qty);
        self.copies[pos].1 -= taken;
        if self.copies[pos].1 == 0 {
            self.copies.remove(pos);
        }
        taken
    }

    /// Every special copy held, for the cargo total. These are carried like
    /// anything else, so they count against the Buffer figure the player
    /// reads.
    pub fn total(&self) -> u32 {
        self.copies.iter().map(|(_, qty)| *qty).sum()
    }
}

/// The rare-spawn tier a creature rolled when it was created, if any — see
/// `Game::roll_rarity`. A creature that rolled ordinary has no such
/// component at all, which reads as `Ordinary`, so no test fixture or
/// hand-built bundle has to know this exists.
///
/// **This is a record of a multiplier already spent, not a live one — at
/// two sites now, not one.** `stat_mult` is applied at spawn, inside
/// `Game::spawn_wild_creature_scaled`, and baked into `Stats` there the same
/// way `Potential`'s three stat rolls are. `Game::promote_rarity` is the
/// second and last: a nemesis mark that ratchets this field up a rung
/// multiplies `Stats` by the *step* between the old and new tier's
/// `stat_mult`, never by the new tier's absolute value — reapplying that
/// would compound the spawn roll a second time. `Game::load` restores the
/// resulting numbers verbatim and `Game::fuse_companions` derives them from
/// its parents, so neither of those may apply this at all — the shape is
/// `EquippedItem::fusion_tier`, whose doc makes the same argument. Applying
/// it outside those two sites compounds the bonus on every reload,
/// invisibly, because a stat carries no record of where it came from. The
/// regression to head off is a later reader finding a multiplier that
/// appears to go unused and "finishing the job"; `a_shiny_survives_a_save_
/// round_trip` asserts the stats come back *unchanged* for exactly that
/// reason.
///
/// Player-facing text says Optimized/Overclocked rather than silver/gold,
/// which is the `MessageKind::Raid` / "GC Entropy Sweep" split: the enum and
/// the save field name the colours, everything a player reads names the
/// thing. `label` is the one place that translation happens.
///
/// **Append new tiers, never reorder.** The derived `Ord` *is* the worth
/// order and `Game::fuse_companions` inherits `max(a, b)` off it, so moving
/// a variant silently changes what a fusion produces. The save no longer
/// cares — since v29 the payload is field-named RON and an enum is written
/// by name — but `the_tiers_order_by_how_good_they_are` pins the half that
/// still bites.
///
/// Since 0.8.9 this axis covers **gear as well as programs**: a dropped
/// weapon rolls a tier the same way a wild program does, and reads in the
/// same words and the same colour. `EquipmentStats::for_rarity` is the gear
/// side of `stat_mult`.
#[derive(
    Component, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub enum Rarity {
    #[default]
    Ordinary,
    Silver,
    Gold,
    Platinum,
    Prismatic,
}

impl Rarity {
    /// Every tier, worst first. One definition, for the reason
    /// `EquipmentSlot::ALL` gives: the spawn ladder, the gear roll and the
    /// colour table all walk it, so a sixth rung cannot reach some of them
    /// and miss others — and `rank` is defined as the position in it.
    pub const ALL: [Rarity; 5] = [
        Rarity::Ordinary,
        Rarity::Silver,
        Rarity::Gold,
        Rarity::Platinum,
        Rarity::Prismatic,
    ];

    /// How far up the ladder this tier sits, `Ordinary` being 0.
    ///
    /// Read by `EquipmentStats::for_rarity`'s floor, which needs a *count*
    /// of rungs rather than a multiplier — the same job
    /// `ITEM_FUSION_MIN_BONUS_PER_TIER` does with a fusion tier, which is
    /// already a number. Derived from `ALL` rather than written out again,
    /// so the two cannot disagree.
    pub fn rank(self) -> u32 {
        Rarity::ALL.iter().position(|r| *r == self).unwrap_or(0) as u32
    }

    /// What this tier multiplies stats by — every one of a creature's four,
    /// or an item's three (see `EquipmentStats::for_rarity`).
    ///
    /// Deliberately one ladder for both. An Overclocked program and an
    /// Overclocked weapon are the same promise to the player, and a retune
    /// that moved only one of them would make the shared colour and the
    /// shared word a lie.
    pub fn stat_mult(self) -> f32 {
        match self {
            Rarity::Ordinary => 1.0,
            Rarity::Silver => SILVER_STAT_MULT,
            Rarity::Gold => GOLD_STAT_MULT,
            Rarity::Platinum => PLATINUM_STAT_MULT,
            Rarity::Prismatic => PRISMATIC_STAT_MULT,
        }
    }

    /// How the tier reads to a player, or `None` for an ordinary one —
    /// `Option` rather than an empty string so a caller has to decide what
    /// "no tier" looks like in its own context, the way `fusion_color`
    /// returns `Option` so a louder rule can compose with it.
    ///
    /// The words continue the compiler vocabulary the first two set, which
    /// is what lets one ladder cover programs and gear without either
    /// reading oddly: a program and a weapon are both things that were
    /// compiled better than usual.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Rarity::Ordinary => None,
            Rarity::Silver => Some("Optimized"),
            Rarity::Gold => Some("Overclocked"),
            Rarity::Platinum => Some("Unrolled"),
            Rarity::Prismatic => Some("Bare-Metal"),
        }
    }
}

#[cfg(test)]
mod rarity_tests {
    use super::Rarity;

    /// Walks `ALL` rather than naming pairs, so a sixth rung is covered the
    /// moment it is added instead of leaving a test that still passes while
    /// checking five of six.
    #[test]
    fn rarity_multiplies_every_stat() {
        assert_eq!(Rarity::Ordinary.stat_mult(), 1.0, "ordinary must be inert");
        for pair in Rarity::ALL.windows(2) {
            assert!(
                pair[1].stat_mult() > pair[0].stat_mult(),
                "{:?} must be worth more than {:?}",
                pair[1],
                pair[0]
            );
        }
    }

    #[test]
    fn only_a_rare_tier_has_a_label() {
        assert_eq!(Rarity::Ordinary.label(), None);
        for tier in Rarity::ALL.into_iter().skip(1) {
            assert!(
                tier.label().is_some(),
                "{tier:?} needs a player-facing name"
            );
        }
    }

    /// Every rung's label is distinct. Two tiers reading the same way is the
    /// failure that survives every other test here — the stats would differ,
    /// the colour would differ, and the word on screen would not.
    #[test]
    fn no_two_tiers_read_the_same() {
        let mut seen: Vec<&str> = Rarity::ALL.into_iter().filter_map(|r| r.label()).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(before, seen.len(), "two rare tiers share a label: {seen:?}");
    }

    /// `Game::fuse_companions` inherits `max(parent_a, parent_b)`, so the
    /// derived `Ord` has to agree with the tiers' worth. Reordering the
    /// variants would silently make a fusion of a gold and a silver
    /// produce a silver.
    #[test]
    fn the_tiers_order_by_how_good_they_are() {
        for pair in Rarity::ALL.windows(2) {
            assert!(
                pair[1] > pair[0],
                "{:?} must outrank {:?}",
                pair[1],
                pair[0]
            );
        }
        assert_eq!(Rarity::Silver.max(Rarity::Gold), Rarity::Gold);
        assert_eq!(Rarity::default(), Rarity::Ordinary);
        assert_eq!(
            Rarity::ALL[0],
            Rarity::Ordinary,
            "ALL must start at the inert rung — `rank` and the spawn ladder both index off it"
        );
    }

    /// `rank` is what the gear floor multiplies, so it has to be the
    /// position in `ALL` and not a second hand-written ladder.
    #[test]
    fn rank_is_the_position_in_the_ladder() {
        for (i, tier) in Rarity::ALL.into_iter().enumerate() {
            assert_eq!(tier.rank(), i as u32, "{tier:?} is out of step with ALL");
        }
    }
}
