//! Thread palettes and nearest-colour matching.
//!
//! Some formats store a palette index per colour block instead of a colour (PES stores Brother PEC
//! indices). Writing such a format means picking, for each thread, the palette entry that looks most like
//! it: the smallest CIEDE2000 difference ([`color`]), ties broken by the lower index so the choice never
//! depends on iteration order.
//!
//! A machine knows threads only by their entries, so 2 different threads written as one entry are one
//! thread to it: it shows the same colour for both, and a change between them reads as a stop. So the
//! threads of a design are given entries together ([`Palette::assign`]): different threads different
//! entries while they last, the nearest pairs first. A thread read from such a file already is an entry
//! ([`Palette::entry_of`]), and is written back as it, machine-special entries included.

pub mod color;

mod brother_pec;

pub use brother_pec::BROTHER_PEC;

use std::collections::{BTreeMap, BTreeSet};

use crate::profile::PaletteId;
use crate::thread::{Rgb, Thread};
use color::{Lab, ciede2000};

/// One palette entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    /// The index the format stores.
    pub index: u8,
    /// The thread's name, as the machine shows it.
    pub name: &'static str,
    /// The colour matching measures against.
    pub color: Rgb,
    /// Whether automatic matching may choose this entry (false for machine-special entries).
    pub matchable: bool,
}

/// The entries a design's threads are written as ([`Palette::assign`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assigned {
    /// The entry of each thread, in order.
    pub entries: Vec<&'static PaletteEntry>,
    /// How many different threads there are.
    pub threads: usize,
    /// How many of them have an entry another of them has too: the different threads less the entries
    /// they have.
    pub shared: usize,
}

/// A table of thread colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Which palette this is.
    pub id: PaletteId,
    /// Its name.
    pub name: &'static str,
    /// The entries, by ascending index.
    pub entries: &'static [PaletteEntry],
}

impl Palette {
    /// The matchable entry closest to `color` (CIEDE2000; lower index on ties), or `None` for a palette
    /// without matchable entries.
    pub fn nearest(&self, color: Rgb) -> Option<&'static PaletteEntry> {
        let target = Lab::from_rgb(color);
        let mut best: Option<(&'static PaletteEntry, f64)> = None;
        for entry in self.entries.iter().filter(|e| e.matchable) {
            let distance = ciede2000(target, Lab::from_rgb(entry.color));
            if best.is_none_or(|(_, d)| distance < d) {
                best = Some((entry, distance));
            }
        }
        best.map(|(entry, _)| entry)
    }

    /// The entry with `index`.
    pub fn entry(&self, index: u8) -> Option<&'static PaletteEntry> {
        self.entries.iter().find(|e| e.index == index)
    }

    /// The entry `thread` is, if it is one: the entry with its colour and its name, as a file read with this
    /// palette names its threads. Entries matching never picks are included.
    pub fn entry_of(&self, thread: &Thread) -> Option<&'static PaletteEntry> {
        let name = thread.name.as_deref()?;
        self.entries.iter().find(|e| e.color == thread.color && e.name == name)
    }

    /// The entries `threads` are written as, one for each, in order (REQ-THREAD-002). A thread that is an
    /// entry ([`Palette::entry_of`]) is written as that entry. The others get the entries matching may pick:
    /// different threads different entries while they last, the nearest pairs first. So a thread gets its
    /// nearest entry unless a thread nearer to it took it, and then the nearest one left. Ties go to the
    /// thread that comes first, then to the lower index. Equal threads, of one colour and name, share an
    /// entry. Once the entries run out, each thread left is written as its nearest entry, which another
    /// thread has too.
    ///
    /// `None` when a thread needs an entry and the palette has none that matching may pick. Costs a colour
    /// difference for each different thread and each entry matching may pick.
    pub fn assign<'t>(&self, threads: impl IntoIterator<Item = &'t Thread>) -> Option<Assigned> {
        // The different threads, by colour and name, in the order they first come, and the entry of each.
        let mut index: BTreeMap<(Rgb, Option<&str>), usize> = BTreeMap::new();
        let (mut order, mut distinct): (Vec<usize>, Vec<(&Thread, Option<&'static PaletteEntry>)>) = (Vec::new(), Vec::new());
        for thread in threads {
            let next = distinct.len();
            let i = *index.entry((thread.color, thread.name.as_deref())).or_insert(next);
            if i == next {
                distinct.push((thread, self.entry_of(thread)));
            }
            order.push(i);
        }
        let mut taken: Vec<u8> = distinct.iter().filter_map(|(_, entry)| entry.map(|e| e.index)).collect();
        let choices: Vec<(&'static PaletteEntry, Lab)> = self.entries.iter().filter(|e| e.matchable).map(|e| (e, Lab::from_rgb(e.color))).collect();
        let mut pairs: Vec<(f64, usize, &'static PaletteEntry)> = Vec::new();
        for (i, (thread, _)) in distinct.iter().enumerate().filter(|(_, (_, entry))| entry.is_none()) {
            let color = Lab::from_rgb(thread.color);
            pairs.extend(choices.iter().map(|&(entry, lab)| (ciede2000(color, lab), i, entry)));
        }
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.index.cmp(&b.2.index)));
        for (_, i, entry) in pairs {
            if !taken.contains(&entry.index)
                && let Some((_, slot)) = distinct.get_mut(i)
                && slot.is_none()
            {
                *slot = Some(entry);
                taken.push(entry.index);
            }
        }
        let entries = distinct.iter().map(|&(thread, entry)| entry.or_else(|| self.nearest(thread.color))).collect::<Option<Vec<_>>>()?;
        let used = entries.iter().map(|e| e.index).collect::<BTreeSet<_>>().len();
        let shared = entries.len().saturating_sub(used);
        Some(Assigned { entries: order.iter().map(|&i| entries.get(i).copied()).collect::<Option<_>>()?, threads: entries.len(), shared })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spot checks against the published table (pyembroidery's PEC thread set).
    #[test]
    fn req_thread_001_brother_pec_matches_its_source_at_spot_checks() {
        let p = &BROTHER_PEC;
        assert_eq!(p.entries.len(), 64);
        for (index, name, rgb) in [
            (1, "Prussian Blue", 0x0E1F7C),
            (5, "Red", 0xED171F),
            (20, "Black", 0x000000),
            (29, "White", 0xF0F0F0),
            (37, "Leaf Green", 0x66BA49),
            (54, "Emerald Green", 0x00673E),
            (61, "Fresh Green", 0xE3F35B),
            (64, "Applique", 0xFFC8C8),
        ] {
            let entry = p.entry(index).unwrap();
            assert_eq!((entry.name, entry.color), (name, Rgb::from_hex(rgb)), "index {index}");
        }
        assert!(p.entries.windows(2).all(|w| w[0].index + 1 == w[1].index), "indices are 1..=64 in order");
        assert!(p.entry(0).is_none());
    }

    #[test]
    fn req_thread_001_every_thread_colour_matches_itself() {
        for entry in BROTHER_PEC.entries.iter().filter(|e| e.matchable) {
            assert_eq!(BROTHER_PEC.nearest(entry.color).map(|e| e.index), Some(entry.index), "{}", entry.name);
        }
    }

    #[test]
    fn applique_entries_are_never_chosen() {
        let pink = BROTHER_PEC.nearest(Rgb::from_hex(0xFFC8C8)).unwrap();
        assert!(pink.index < 62, "chose {}", pink.name);
    }

    /// The palette indices `threads` are written as.
    fn assigned(threads: &[Thread]) -> Vec<u8> {
        BROTHER_PEC.assign(threads).unwrap().entries.iter().map(|e| e.index).collect()
    }

    fn thread(rgb: u32) -> Thread {
        Thread::new(Rgb::from_hex(rgb))
    }

    #[test]
    fn req_thread_002_distinct_threads_get_distinct_entries_the_nearest_pairs_first() {
        // The nearest entry of both reds is Red, #ED171F, the second itself: it goes to the nearer, whatever
        // the order, and the other takes its next nearest, Vermilion. So with White, #F0F0F0, and white.
        assert_eq!(assigned(&[thread(0xD81B24), thread(0xED171F)]), [28, 5]);
        assert_eq!(assigned(&[thread(0xED171F), thread(0xD81B24)]), [5, 28]);
        assert_eq!(assigned(&[thread(0xFFFFFF), thread(0xF0F0F0)]), [40, 29]);
        // Equal threads share their entry, and a thread alone gets its nearest.
        assert_eq!(assigned(&[thread(0xD81B24), thread(0x0A55A3), thread(0xD81B24)]), [5, 2, 5]);
        assert_eq!(assigned(&[thread(0xFF0000)]), [BROTHER_PEC.nearest(Rgb::from_hex(0xFF0000)).unwrap().index]);
        assert_eq!(assigned(&[]), Vec::<u8>::new());
        // Names tell threads apart too: 2 threads of one colour are 2 threads.
        assert_eq!(assigned(&[Thread::named(Rgb::from_hex(0xD81B24), "Isacord 1902"), thread(0xD81B24)]), [5, 28]);
    }

    #[test]
    fn req_thread_002_a_thread_that_is_an_entry_is_written_as_that_entry() {
        // As a PES file is read: each thread has its entry's colour and name, the appliqué steps included,
        // which colour matching never picks. Entries 63 and 64 share a colour; their names tell them apart.
        let read: Vec<Thread> = [62, 63, 64, 5].iter().map(|&i| BROTHER_PEC.entry(i).unwrap()).map(|e| Thread::named(e.color, e.name)).collect();
        assert_eq!(read.iter().map(|t| BROTHER_PEC.entry_of(t).map(|e| e.index)).collect::<Vec<_>>(), [Some(62), Some(63), Some(64), Some(5)]);
        // Red read from a file keeps Red, and a thread of its colour without its name takes the next nearest.
        let mut threads = read;
        threads.extend([thread(0xED171F), thread(0xFFC864)]);
        assert_eq!(assigned(&threads), [62, 63, 64, 5, 28, 12]);
        assert_eq!(BROTHER_PEC.entry_of(&thread(0xED171F)), None);
        assert_eq!(BROTHER_PEC.entry_of(&Thread::named(Rgb::from_hex(0xED171F), "Blue")), None);
    }

    #[test]
    fn req_thread_002_when_the_entries_run_out_threads_share_their_nearest() {
        // The 61 colours matching may pick, each its own entry, and 2 threads more: their nearest entries,
        // Red and White, are taken, so each shares its nearest.
        let mut threads: Vec<Thread> = BROTHER_PEC.entries.iter().filter(|e| e.matchable).map(|e| Thread::new(e.color)).collect();
        assert_eq!(threads.len(), 61);
        threads.extend([thread(0xD81B24), thread(0xFFFFFF)]);
        let indices = assigned(&threads);
        assert_eq!(indices.get(..61), Some(&(1..=61).collect::<Vec<u8>>()[..]));
        assert_eq!(indices.get(61..), Some(&[5, 29][..]));
        let counted = BROTHER_PEC.assign(&threads).unwrap();
        assert_eq!((counted.threads, counted.shared), (63, 2));
        // Equal threads are one thread, and share nothing with another.
        let counted = BROTHER_PEC.assign(&[thread(0xD81B24), thread(0xD81B24)]).unwrap();
        assert_eq!((counted.threads, counted.shared), (1, 0));
    }

    #[test]
    fn req_thread_001_matching_is_perceptual_not_rgb_distance() {
        let name = |rgb| BROTHER_PEC.nearest(Rgb::from_hex(rgb)).map(|e| e.name);
        // Distance in RGB would pick "Red" (#ED171F) for pure red and "Deep Gold" for CSS orange. People
        // see Vermilion (ΔE00 3.4, against 5.0 for Red) and Pumpkin (4.4) as closer, and so does CIEDE2000.
        assert_eq!(name(0xFF0000), Some("Vermilion"));
        assert_eq!(name(0xFFA500), Some("Pumpkin"));
        assert_eq!(name(0x808080), Some("Gray"));
        assert_eq!(name(0xFFFFFF), Some("White"));
    }
}
