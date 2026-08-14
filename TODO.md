# Todos
2. some ambiguity|routines for examining entities
3. environment effects
4. unorthodox solutions
5. high stakes reward
6. easter eggs (hidden commands & effects)
7. remove 'move abilities' and use routines
8. more machine learning
9. i want the description of what i'm seeing as i'm walking in the stack, not with x | examine
10. when a stack boss is defeated, the stack should collapse, and the player gets kicked out, and the entrance is deleted (no need to keep in memory), new stack appears somewhere nearby
11. structures to keep base
12. next zone unlocks are research -> upgrade base to zone 2
13. one button to fuse all items
14. When selecting a non-combat routine to run on a party member, show their stats on the 'run a routine' popup
15. show equipped slots on main battle page
17. extract a routine screen should show a list of routines to extract per entity
20. too easy to level up, lets slow that down a bit


# Bugs
8. attacking groups of enemies shouldn't pull in nearby bosses
7. attacks to one group shouldn't overflow to next group, after that group has been defeated.
6. entities spawning stuck on unmovable terrain
3. when the battle log goes beyound the visual buffer, it should scroll, it currently gets truncated
4. heals in battle log should be green
1. A maxed-fusion Gold affixed copy overflows the inventory popup: the widest
   the shipped assets can build is 1311px into a 1243px body at zone 10, and
   the row runs off the right edge taking its equip tag with it. The excess is
   `equip_preview_tag`'s `" - maxed"`, appended on the stated grounds that
   this screen has the room. Pre-existing, measured 2026-08-13;
   `no_shipped_inventory_row_overflows_its_popup` covers every tier below
   maxed and excludes that case by name.
2. The roster's widest row overflows the Party popup the same way, and by far
   more: a maxed-fusion rare program with a full custom name, a zone tag, a
   quality tier, the wield mark, an activity and CRITICAL measures 1636px into
   the same 1243px body — 393px over. Pre-existing and worse than Bug 1
   because the name half is player-authored rather than shipped, so no census
   over the assets can bound it. Measured 2026-08-13, when the `w|a|m` loadout
   cell was added; the cell is 54px of that total and the row was already
   339px over without it. The fix is a chop, not a shorter tag — the row has
   six optional tags and drops none of them.
