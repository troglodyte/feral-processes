//! The list. Every first-time message in the game, in one place: add a
//! `FtueEntry` here and nothing else changes. See the module doc in `mod.rs`
//! for the trigger kinds and the rules a message follows.

use super::{FtueEntry, FtueEvent, FtueTrigger};
use crate::components::GlyphColor;

pub const ENTRIES: &[FtueEntry] = &[
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::BaseFounding),
        title: "Your Base",
        body: "Your Home stands, with floor laid around it. Grow the base outward tile \
                   by tile.\n\n [b] for the base report: build, craft, \
                   and assign entities to structures.\n\n Programs you own and are not in the \
                   party will live and work in the base.",
        // The same art the zone map now draws the anchor with, so
        // the tutorial and the map agree on what to look for. The
        // prose no longer names a character for the same reason:
        // `#` is the *fallback*, not what the player will see.
        sprite: Some("anchor"),
        glyph: '#',
        color: GlyphColor::Cyan,
    },
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::FirstDescent),
        title: "The Stack",
        body: "Below the zone field, the ground stops becomes a stack (a dungeon). Each \
                   frame (level) down is more dangerous than the last.\n\n Something is \
                   scanning your progress, the 'Trace' climbs the deeper and longer you stay, \
                   and what it draws is not friendly, encounters will become more frequent. \
                   You sense something big, humming with energy below.\n\nPress [g] for the \
                   map of the frame you are standing in.",
        sprite: None,
        glyph: '>',
        color: GlyphColor::Cyan,
    },
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::FirstRaid),
        title: "GC Entropy Sweep",
        body: "The garbage collector has found your base. A sweep comes to clean up stray \
                   data, ...your structures, and it will keep coming.\n\nStaff posted at a \
                   machine defend it, and take damage doing so. There must be a way to defend \
                   against it.",
        sprite: None,
        glyph: '!',
        color: GlyphColor::Red,
    },
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::FirstTownRaid),
        title: "They Know Where You Live",
        body: "A settlement you have wronged has sent people for your stores. They do not \
                   come to break your machines — they come to carry off what your base runs \
                   on, and they will keep coming while the grudge stands.\n\nWhat you have \
                   built to turn a sweep away turns them away too, and so does a neighbour \
                   who likes you. Standing can be repaired.",
        sprite: None,
        glyph: '!',
        color: GlyphColor::Red,
    },
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::FirstWorkOrder),
        title: "The Order Is Filed",
        body: "You have declared to your semi sentient programs what to craft, and what to \
                   keep in stock. Every tick, the base recomputes which machines that order \
                   needs, who is free to stand at them, and how far along each one is. \n\nA \
                   standing order is a level the base keeps topped up rather than a batch it \
                   fills once. \n\nIf the line stalls, the order screen names the machine it \
                   is waiting on.",
        sprite: None,
        glyph: '&',
        color: GlyphColor::Yellow,
    },
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::LowPower),
        title: "Power Low",
        body: "Your Power reserve is under half. It drains a little every tick whatever \
                   you are doing, and every routine you run comes out of the same \
                   pool.\n\nFrom here your own attacks start to weaken, and at zero you \
                   take Integrity damage every tick until you find some.\n\nPress [e] to \
                   drain a Power Cell for 25 — two Core Fragments compiles one. [r] powers \
                   down and fills it outright: free inside your base, one Power Outlet \
                   anywhere else.",
        sprite: None,
        glyph: '%',
        color: GlyphColor::Yellow,
    },
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::DownedProgram),
        title: "Program Down",
        body: "A program of yours went down and is benched. It keeps its levels and \
                   everything it has learned, but it cannot be put back in your party, take \
                   a post, or work.\n\nA Repair Bay is what brings one back: a downed \
                   program walks there itself and is written up to full Integrity a tick at \
                   a time. Until one stands in your base, benched is where it stays — you \
                   can still sell it or pull a routine out of it.\n\n[b] for the base \
                   report, then Deploy a structure on the Build page.",
        sprite: None,
        glyph: 'r',
        color: GlyphColor::Red,
    },
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::FirstStatic),
        title: "Static",
        body: "That is Static, not the ground itself — interference riding on \
                   top of the terrain, tied to the whole biome you are standing in rather \
                   than to this one tile.\n\nIt runs on its own clock. Given time it clears, \
                   and something else may settle over the same ground later. There is \
                   nothing here to switch off.",
        sprite: None,
        glyph: '~',
        color: GlyphColor::Orange,
    },
    FtueEntry {
        on: FtueTrigger::Event(FtueEvent::LearnedImage),
        title: "Image Learned",
        body: "You captured an image — a downed program's shape and stats, kept for \
                   later. Run your Emulate routine to wear it for a while: it swaps your \
                   attack, mitigation, reach and routines for the image's own, and \
                   everything else about you stays yours.\n\nEvery image you extract is \
                   another shape to become. You can only wear one at a time, and dropping \
                   it back off costs a turn.",
        sprite: None,
        glyph: 'i',
        color: GlyphColor::Cyan,
    },
];
