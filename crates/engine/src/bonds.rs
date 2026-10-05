//! What one program's opinion of another means.
//!
//! The band is **derived on every read, never stored**, `settlements::
//! relations`'s rule: a retune of the thresholds re-bands every existing
//! save. Every consequence is a named query on `Bond` with an exhaustive
//! match and no `_` arm, so a new rung fails to compile until each query
//! answers it.

use serde::{Deserialize, Serialize};

use crate::tuning::{BOND_CLOSE_AT, BOND_ENEMY_AT, BOND_FRIEND_AT, BOND_RIVAL_AT};

/// Ordered from the bottom so the thresholds read as the ladder they are.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum Bond {
    Enemy,
    Rival,
    Neutral,
    Friend,
    Close,
}

/// Negative thresholds are `<=` and positive ones `>=`, so a zero opinion —
/// a real answer, for a program nothing has happened about — is always
/// `Neutral`.
pub fn band(opinion: f32) -> Bond {
    if opinion <= BOND_ENEMY_AT {
        Bond::Enemy
    } else if opinion <= BOND_RIVAL_AT {
        Bond::Rival
    } else if opinion >= BOND_CLOSE_AT {
        Bond::Close
    } else if opinion >= BOND_FRIEND_AT {
        Bond::Friend
    } else {
        Bond::Neutral
    }
}

impl Bond {
    /// Every rung, bottom first, so a report can seed a band that nobody
    /// stood on with a zero.
    pub const ALL: [Bond; 5] = [
        Bond::Enemy,
        Bond::Rival,
        Bond::Neutral,
        Bond::Friend,
        Bond::Close,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Bond::Enemy => "Enemy",
            Bond::Rival => "Rival",
            Bond::Neutral => "Neutral",
            Bond::Friend => "Friend",
            Bond::Close => "Close",
        }
    }

    /// Whether a worker holding this band of a neighbour keeps off the tiles
    /// beside it. Signed: a fondness never triggers it.
    pub fn avoids(self) -> bool {
        match self {
            Bond::Enemy | Bond::Rival => true,
            Bond::Neutral | Bond::Friend | Bond::Close => false,
        }
    }

    /// Whether this program leaving costs the holder something.
    pub fn grieves(self) -> bool {
        match self {
            Bond::Friend | Bond::Close => true,
            Bond::Enemy | Bond::Rival | Bond::Neutral => false,
        }
    }

    /// Whether this program leaving is a weight off the holder.
    pub fn relieved(self) -> bool {
        match self {
            Bond::Enemy | Bond::Rival => true,
            Bond::Neutral | Bond::Friend | Bond::Close => false,
        }
    }
}

/// How a program stopped being on the roster, for the grief its friends
/// feel. `Game::note_departure` is the one reader.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Departure {
    /// Killed under Permadeath.
    Fell,
    /// Sold, extracted, or spent on a build or in the study.
    LetGo,
    /// Consumed as a fusion parent.
    Fused,
}

impl Departure {
    /// The memory a friend or close friend is left holding.
    pub fn grief_def(self) -> &'static str {
        match self {
            Departure::Fell => "lost_in_battle",
            Departure::LetGo => "let_go",
            Departure::Fused => "became_part_of",
        }
    }
}

/// What a rival or enemy is left holding, whatever way the program left.
pub const RELIEF_DEF: &str = "rid_of";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_edge_is_half_open_toward_neutral() {
        let below = |x: f32| x - 0.001;
        let above = |x: f32| x + 0.001;

        assert_eq!(band(BOND_ENEMY_AT), Bond::Enemy, "-15 itself is an enemy");
        assert_eq!(band(above(BOND_ENEMY_AT)), Bond::Rival);
        assert_eq!(band(BOND_RIVAL_AT), Bond::Rival, "-3 itself is a rival");
        assert_eq!(band(above(BOND_RIVAL_AT)), Bond::Neutral);
        assert_eq!(band(below(BOND_FRIEND_AT)), Bond::Neutral);
        assert_eq!(band(BOND_FRIEND_AT), Bond::Friend, "8 itself is a friend");
        assert_eq!(band(below(BOND_CLOSE_AT)), Bond::Friend);
        assert_eq!(band(BOND_CLOSE_AT), Bond::Close, "20 itself is close");
    }

    #[test]
    fn a_zero_opinion_is_neutral() {
        assert_eq!(band(0.0), Bond::Neutral);
    }

    #[test]
    fn the_queries_split_the_ladder_where_the_spec_says() {
        let all = [
            Bond::Enemy,
            Bond::Rival,
            Bond::Neutral,
            Bond::Friend,
            Bond::Close,
        ];
        let avoids: Vec<Bond> = all.into_iter().filter(|b| b.avoids()).collect();
        let grieves: Vec<Bond> = all.into_iter().filter(|b| b.grieves()).collect();
        let relieved: Vec<Bond> = all.into_iter().filter(|b| b.relieved()).collect();
        assert_eq!(avoids, [Bond::Enemy, Bond::Rival]);
        assert_eq!(grieves, [Bond::Friend, Bond::Close]);
        assert_eq!(relieved, [Bond::Enemy, Bond::Rival]);
    }

    #[test]
    fn all_lists_every_rung_bottom_first() {
        assert!(Bond::ALL.windows(2).all(|w| w[0] < w[1]));
        for opinion in [-100.0, BOND_RIVAL_AT, 0.0, BOND_FRIEND_AT, 100.0] {
            assert!(Bond::ALL.contains(&band(opinion)));
        }
    }
}
