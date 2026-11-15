//! The search for the fewest thread changes (`REQ-AUTO-037`): the batches it finds, the wishes it grants,
//! and what it does with less work than it needs.

use stitchcraft_core::Budget;

use super::*;

const RED: Thread = Some(Rgb::new(200, 0, 0));
const BLUE: Thread = Some(Rgb::new(0, 0, 200));
const GREEN: Thread = Some(Rgb::new(0, 160, 0));

/// A run of elements each sewn in one of `threads`, where each of `rules` sews its first element before its
/// second, and each of `wished` would; no thread is sewing when it starts.
fn run(threads: &[Thread], rules: &[(usize, usize)], wished: &[(usize, usize)]) -> Run {
    let mut before = vec![Vec::new(); threads.len()];
    for &(first, then) in rules {
        before[then].push(first);
    }
    Run { threads: threads.iter().map(|&thread| [thread; 2]).collect(), before, wished: wished.to_vec(), sewing: None }
}

/// The order found for `run` with the default budget.
fn found(run: &Run) -> Found {
    fewest(run, &mut Budget::DEFAULT.meter()).unwrap()
}

/// How many times the thread changes as `batches` of `run` sew, the thread at the start counted too.
fn changes(run: &Run, batches: &[Vec<usize>]) -> usize {
    let mut sewing = run.sewing;
    let mut changes = 0;
    for &index in batches.iter().flatten() {
        let [first, last] = run.threads[index];
        changes += usize::from(first != sewing);
        sewing = last;
    }
    changes
}

#[test]
fn req_auto_037_each_thread_sews_once_when_no_rule_holds_an_element_back() {
    let threads = run(&[RED, BLUE, RED, GREEN, BLUE], &[], &[]);
    assert_eq!(found(&threads), Found { batches: vec![vec![0, 2], vec![1, 4], vec![3]], granted: vec![] });
    // The thread sewing at the start sews first.
    let sewing = Run { sewing: BLUE, ..run(&[RED, BLUE, RED, GREEN, BLUE], &[], &[]) };
    assert_eq!(found(&sewing).batches, [vec![1, 4], vec![0, 2], vec![3]]);
    assert_eq!(changes(&sewing, &found(&sewing).batches), 2);
    // A run of no elements sews none.
    assert_eq!(found(&run(&[], &[], &[])), Found { batches: vec![], granted: vec![] });
}

#[test]
fn req_auto_037_a_thread_waits_when_sewing_it_first_would_bring_it_back() {
    // Red, blue, and a red that sews after the blue: red first, as the first free element has it, comes back
    // to red after the blue. Blue first sews the reds together.
    let waiting = run(&[RED, BLUE, RED], &[(1, 2)], &[]);
    assert_eq!(found(&waiting).batches, [vec![1], vec![0, 2]]);
    // Each of 3 threads holds an element of the next back, and the last one of the first: the first thread
    // sews twice, and no order has fewer changes.
    let ring = run(&[RED, BLUE, GREEN, BLUE, GREEN, RED], &[(0, 3), (1, 4), (2, 5)], &[]);
    let batches = found(&ring).batches;
    assert_eq!(batches, [vec![0], vec![1, 3], vec![2, 4], vec![5]]);
    assert_eq!(changes(&ring, &batches), 4);
    // An element that waits for one of its own thread sews in the same batch, after it.
    let chain = run(&[RED, BLUE, RED, RED], &[(3, 2), (2, 0)], &[]);
    assert_eq!(found(&chain).batches, [vec![1], vec![3, 2, 0]]);
}

#[test]
fn req_auto_037_a_wish_is_granted_where_it_costs_no_thread_change() {
    // A blue column wished after a red fill drawn after it: red first costs nothing.
    let column = run(&[BLUE, RED], &[], &[(1, 0)]);
    assert_eq!(found(&column), Found { batches: vec![vec![1], vec![0]], granted: vec![(1, 0)] });
    // A blue column wished after a red fill, and a red column wished after a blue fill: one of them sews
    // first, or each thread would sew twice. The first wish is granted.
    let both = run(&[RED, BLUE, BLUE, RED], &[], &[(0, 1), (2, 3)]);
    assert_eq!(found(&both), Found { batches: vec![vec![0, 3], vec![1, 2]], granted: vec![(0, 1)] });
    // The second is when the first can't be: the first fill sews after its column by a rule.
    let held = run(&[RED, BLUE, BLUE, RED], &[(1, 0)], &[(0, 1), (2, 3)]);
    assert_eq!(found(&held), Found { batches: vec![vec![1, 2], vec![0, 3]], granted: vec![(2, 3)] });
    // A column and its fill of one thread sew in one batch, the fill first.
    let own = run(&[RED, RED, BLUE], &[], &[(1, 0)]);
    assert_eq!(found(&own), Found { batches: vec![vec![1, 0], vec![2]], granted: vec![(1, 0)] });
}

#[test]
fn req_auto_037_a_column_waits_for_a_later_batch_of_its_thread_to_sew_after_its_fill() {
    // Red, a blue after it and a red after that: red sews twice. A red column wished after the blue fill is
    // free from the start, and waits for the second red batch.
    let waits = run(&[RED, BLUE, RED, RED], &[(0, 1), (1, 2)], &[(1, 3)]);
    assert_eq!(found(&waits), Found { batches: vec![vec![0], vec![1], vec![2, 3]], granted: vec![(1, 3)] });
    // A blue column wished after the last red has no later blue batch to wait for: it sews with the blue,
    // and the other wish is granted all the same.
    let one = run(&[RED, BLUE, RED, RED, BLUE], &[(0, 1), (1, 2)], &[(2, 4), (1, 3)]);
    assert_eq!(found(&one), Found { batches: vec![vec![0], vec![1, 4], vec![2, 3]], granted: vec![(1, 3)] });
    assert_eq!(changes(&one, &found(&one).batches), 3);
}

#[test]
fn req_auto_037_an_element_in_several_threads_is_a_batch_of_its_own_between_its_threads() {
    // An element that starts in red and ends in blue, a blue and a red: the red, the element, the blue.
    let several = Run { threads: vec![[RED, BLUE], [BLUE; 2], [RED; 2]], ..run(&[RED, BLUE, RED], &[], &[]) };
    assert_eq!(found(&several).batches, [vec![2], vec![0], vec![1]]);
    assert_eq!(changes(&several, &found(&several).batches), 1);
    // With nothing of the thread it ends in, it sews alone; and one that isn't free yet waits.
    let alone = Run { threads: vec![[RED, BLUE], [RED; 2]], ..run(&[RED, RED], &[(1, 0)], &[]) };
    assert_eq!(found(&alone).batches, [vec![1], vec![0]]);
    // Sewn when its first thread isn't sewing, it costs a change: green, then the element and the blue.
    let apart = Run { threads: vec![[RED, BLUE], [BLUE; 2], [GREEN; 2]], sewing: GREEN, ..run(&[RED, BLUE, GREEN], &[], &[]) };
    assert_eq!(found(&apart).batches, [vec![2], vec![0], vec![1]]);
    assert_eq!(changes(&apart, &found(&apart).batches), 1);
}

#[test]
fn elements_that_wait_for_each_other_sew_in_no_batch() {
    // No rule of the order makes 2 elements wait for each other. Were they to, the search gives no batch and
    // grants no wish, and the caller sews the run as it did before.
    let knot = run(&[RED, BLUE, RED], &[(0, 1), (1, 0)], &[(2, 0)]);
    assert_eq!(found(&knot), Found { batches: vec![], granted: vec![] });
}

#[test]
fn req_auto_037_a_search_with_too_little_work_gives_no_order() {
    // Every amount of work less than the search needs fails, at each place it spends some, and every
    // amount from there on finds the same order.
    let both = || run(&[RED, BLUE, RED, RED, BLUE], &[(0, 1), (1, 2)], &[(2, 4), (1, 3)]);
    let with = |work: u64| fewest(&both(), &mut Budget { max_work: work, ..Budget::DEFAULT }.meter());
    let needed = (0..100_000).find(|&work| with(work).is_ok()).unwrap();
    assert!(needed > 3 * STATE && needed < 20 * STATE, "{needed}");
    assert!((0..needed).step_by(7).all(|work| with(work).is_err()));
    assert_eq!(with(needed).unwrap(), found(&both()));
}

#[test]
fn the_elements_sewn_are_a_bit_each() {
    assert_eq!([Done::none(0).0.len(), Done::none(1).0.len(), Done::none(64).0.len(), Done::none(65).0.len()], [0, 1, 1, 2]);
    let mut done = Done::none(130);
    for index in [0, 63, 64, 129] {
        assert!(!done.has(index));
        done.add(index);
        assert!(done.has(index));
    }
    assert_eq!(done.0, [1 | 1 << 63, 1, 2]);
    assert!(!done.has(1) && !done.has(62) && !done.has(65) && !done.has(128));
    // An element past the last is never sewn.
    done.add(500);
    assert!(!done.has(500));
}

#[test]
fn an_element_waits_for_another_itself_or_through_those_between() {
    let before = vec![vec![], vec![0], vec![1], vec![]];
    let waits = |first: usize, then: usize| super::waits(&before, first, then, &mut Budget::DEFAULT.meter()).unwrap();
    assert!(waits(1, 0) && waits(2, 1) && waits(2, 0) && waits(3, 3));
    assert!(!waits(0, 1) && !waits(0, 2) && !waits(3, 0) && !waits(2, 3));
    // A unit for each element looked at: 2, 1 and 0.
    let work = |units: u64| super::waits(&before, 2, 0, &mut Budget { max_work: units, ..Budget::DEFAULT }.meter());
    assert!(work(2).is_err() && work(3).is_ok());
    // An element that 2 others wait for is looked at once.
    let diamond = vec![vec![], vec![0], vec![0], vec![1, 2]];
    let work = |units: u64| super::waits(&diamond, 3, 9, &mut Budget { max_work: units, ..Budget::DEFAULT }.meter());
    assert_eq!((work(3).is_err(), work(4)), (true, Ok(false)));
}

#[test]
fn the_search_counts_what_is_left_and_what_is_missed() {
    let several = Run { threads: vec![[RED; 2], [BLUE; 2], [RED, BLUE], [GREEN; 2], [BLUE; 2]], ..run(&[RED; 5], &[(0, 1)], &[(1, 0), (3, 0), (4, 3)]) };
    let search = Search::new(&several);
    let meter = || Budget::DEFAULT.meter();
    assert_eq!(search.plain, BTreeMap::from([(RED, vec![0]), (BLUE, vec![1, 4]), (GREEN, vec![3])]));
    assert_eq!(search.wishes, [vec![1, 3], vec![], vec![], vec![4], vec![]]);
    // 3 threads have elements left, less one for the element in 2 threads; with it sewn, all 3.
    let mut done = Done::none(5);
    assert_eq!(search.left(&done, &mut meter()), Ok(2));
    done.add(2);
    assert_eq!(search.left(&done, &mut meter()), Ok(3));
    done.add(1);
    done.add(4);
    assert_eq!(search.left(&done, &mut meter()), Ok(2));
    // What may sew next: the thread of each free element once, and the element in several threads.
    assert_eq!(search.steps(&Done::none(5), &mut meter()), Ok(vec![Move::Thread(RED), Move::Element(2), Move::Thread(GREEN), Move::Thread(BLUE)]));
    assert_eq!([search.starts(Move::Thread(GREEN)), search.starts(Move::Element(2)), search.starts(Move::Element(9))], [GREEN, RED, None]);
    // Element 0 sewn first misses its 2 wishes, and after 1 and 3 none. 3 and 0 in one batch miss the wish
    // of 3 alone, and in 2 batches too.
    let missed = |sewn: &[usize], batches: &[Vec<usize>]| {
        let mut done = Done::none(5);
        sewn.iter().for_each(|&index| done.add(index));
        search.missed(done, batches, &mut meter()).unwrap()
    };
    assert_eq!([missed(&[], &[vec![0]]), missed(&[1, 3], &[vec![0]]), missed(&[1], &[vec![3, 0]]), missed(&[1], &[vec![3], vec![0]])], [2, 0, 1, 1]);
    assert_eq!(missed(&[1], &[vec![0], vec![3]]), 2);
}

/// The fewest thread changes of any order of `run`'s elements that its rules allow, by trying every order:
/// `sewn` the elements placed so far, `sewing` the thread sewing after them.
fn fewest_of_every_order(run: &Run, sewn: &mut Vec<usize>, sewing: Thread) -> usize {
    if sewn.len() == run.threads.len() {
        return 0;
    }
    let mut fewest = usize::MAX;
    for index in 0..run.threads.len() {
        if sewn.contains(&index) || !run.before[index].iter().all(|earlier| sewn.contains(earlier)) {
            continue;
        }
        let [first, last] = run.threads[index];
        sewn.push(index);
        fewest = fewest.min(usize::from(first != sewing) + fewest_of_every_order(run, sewn, last));
        sewn.pop();
    }
    fewest
}

#[test]
fn req_auto_037_no_order_of_a_small_run_changes_thread_less_than_the_one_found() {
    // 300 runs of 2 to 7 elements in 3 threads, some sewn in 2 of them, with rules and wishes drawn between
    // them: the batches found sew each element once, keep every rule and every wish granted, and change
    // thread as seldom as the best of every order the rules allow.
    let mut rng = stitchcraft_core::rng::SplitMix64::new(1322);
    let mut draw = |below: u64| (rng.next_u64() % below) as usize;
    let mut held_back = 0;
    for case in 0..300 {
        let count = 2 + draw(6);
        let thread = |index: usize| [RED, BLUE, GREEN][index];
        let threads: Vec<[Thread; 2]> = (0..count).map(|_| if draw(8) == 0 { [thread(draw(3)), thread(draw(3))] } else { [thread(draw(3)); 2] }).collect();
        // The elements wait only for those before them in an order drawn for the run, so none waits for
        // itself.
        let mut places: Vec<usize> = (0..count).collect();
        (1..count).rev().for_each(|last| places.swap(last, draw(last as u64 + 1)));
        let pairs: Vec<(usize, usize)> = (0..count).flat_map(|first| (first + 1..count).map(move |then| (first, then))).collect();
        let mut before = vec![Vec::new(); count];
        let mut wished = Vec::new();
        for (first, then) in pairs {
            match draw(6) {
                0 | 1 => before[places[then]].push(places[first]),
                2 => wished.push((places[draw(count as u64)], places[then])),
                _ => {}
            }
        }
        wished.retain(|(first, then)| first != then);
        let run = Run { threads, before, wished, sewing: [None, RED, BLUE][draw(3)] };
        let found = found(&run);
        let order: Vec<usize> = found.batches.iter().flatten().copied().collect();
        let place = |index: usize| order.iter().position(|&sewn| sewn == index).unwrap();
        let mut each = order.clone();
        each.sort_unstable();
        assert_eq!(each, (0..count).collect::<Vec<_>>(), "case {case}");
        for (then, earlier) in run.before.iter().enumerate() {
            assert!(earlier.iter().all(|&first| place(first) < place(then)), "case {case}: {order:?} breaks a rule of {:?}", run.before);
        }
        assert!(found.granted.iter().all(|&(first, then)| place(first) < place(then)), "case {case}: {order:?} breaks a wish of {:?}", found.granted);
        assert!(found.granted.iter().all(|wish| run.wished.contains(wish)), "case {case}");
        let fewest = fewest_of_every_order(&run, &mut Vec::new(), run.sewing);
        assert_eq!(changes(&run, &found.batches), fewest, "case {case}: {:?} {:?} sews {:?}", run.threads, run.before, found.batches);
        // Taking each thread while it lasts, then the thread of the first free element, as the order did
        // before, changes thread more often on some of them.
        let (mut sewn, mut sewing, mut greedy) = (Vec::new(), run.sewing, 0);
        while sewn.len() < count {
            let free = |index: &usize| !sewn.contains(index) && run.before[*index].iter().all(|earlier| sewn.contains(earlier));
            let next = (0..count).filter(free).find(|&index| run.threads[index][0] == sewing).or_else(|| (0..count).find(free)).unwrap();
            greedy += usize::from(run.threads[next][0] != sewing);
            sewing = run.threads[next][1];
            sewn.push(next);
        }
        assert!(greedy >= fewest, "case {case}");
        held_back += usize::from(greedy > fewest);
    }
    assert!(held_back > 20, "{held_back}");
}
