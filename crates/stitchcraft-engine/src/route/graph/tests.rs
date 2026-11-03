use stitchcraft_core::Budget;

use super::*;
use crate::route::tests::p;

/// A graph of pieces, each of element `element`, from one point to the next of `lines`.
fn graph(directed: bool, lines: &[(usize, &[(f64, f64)])]) -> Graph {
    let mut graph = Graph::new(directed);
    for &(element, points) in lines {
        for (index, pair) in points.windows(2).enumerate() {
            graph.piece(element, index, [p(pair[0].0, pair[0].1), p(pair[1].0, pair[1].1)]);
        }
    }
    graph
}

/// The jumps of `graph`, by the places they join.
fn jumps(graph: &Graph) -> Vec<[usize; 2]> {
    graph.links.iter().filter_map(|link| if let Link::Jump { ends } = link { Some(*ends) } else { None }).collect()
}

#[test]
fn ends_at_one_point_are_one_place_which_belongs_to_the_last_element_ending_there() {
    let mut graph = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0)]), (1, &[(1.0, 0.0), (2.0, 0.0)])]);
    // -0 and 0 are one coordinate.
    graph.piece(2, 0, [p(2.0, 0.0), p(-0.0, 0.0)]);
    let places: Vec<(Point, usize)> = graph.places().iter().map(|place| (place.at, place.element)).collect();
    assert_eq!(places, [(p(0.0, 0.0), 2), (p(1.0, 0.0), 1), (p(2.0, 0.0), 2)]);
    assert_eq!(graph.link(2), Some(Link::Piece { element: 2, index: 0, ends: [2, 0] }));
    assert_eq!(graph.link(3), None);
    assert_eq!(graph.places_on(1, &mut Budget::DEFAULT.meter()).unwrap(), [1, 2]);
    assert_eq!(graph.places_on(2, &mut Budget::DEFAULT.meter()).unwrap(), [0, 2]);
}

#[test]
fn jumps_join_the_groups_by_a_shortest_tree_added_group_by_group() {
    // 3 groups along a line, 3 and 1 mm apart: the tree takes the 1 mm jump first, but the first group's
    // jump is added first.
    let mut apart = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0)]), (1, &[(4.0, 0.0), (5.0, 0.0)]), (2, &[(6.0, 0.0), (7.0, 0.0)])]);
    apart.join_apart(&mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(jumps(&apart), [[1, 2], [3, 4]]);
    // A tree has no jump between groups already joined: here not the 5 mm one from the first to the last.
    let mut even = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0)]), (1, &[(3.0, 0.0), (4.0, 0.0)]), (2, &[(6.0, 0.0), (7.0, 0.0)])]);
    even.join_apart(&mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(jumps(&even), [[1, 2], [3, 4]]);
    // Of jumps as long, the earlier groups' first; of places as near, the first of each group's.
    let mut ties = graph(false, &[(0, &[(0.0, 0.0), (2.0, 0.0)]), (1, &[(1.0, 3.0), (1.0, 6.0)]), (2, &[(1.0, -3.0), (1.0, -6.0)])]);
    ties.join_apart(&mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(jumps(&ties), [[0, 2], [0, 4]]);
    // One group needs no jump.
    let mut one = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)])]);
    one.join_apart(&mut Budget::DEFAULT.meter()).unwrap();
    assert!(jumps(&one).is_empty());
}

#[test]
fn with_preserve_order_each_element_is_joined_to_the_next_one_way() {
    let mut ordered = graph(true, &[(0, &[(0.0, 0.0), (1.0, 0.0)]), (1, &[(9.0, 0.0), (8.0, 0.0)]), (2, &[(3.0, 0.0), (4.0, 0.0)])]);
    ordered.join_in_order(&[0, 1, 2], &mut Budget::DEFAULT.meter()).unwrap();
    // From the first's nearest place to the second's, and from the second's to the third's.
    assert_eq!(jumps(&ordered), [[1, 3], [3, 5]]);
    let meter = &mut Budget::DEFAULT.meter();
    // The jumps go forwards only, the pieces both ways.
    assert_eq!(ordered.shortest(0, 5, meter).unwrap(), Some((vec![0, 1, 3, 5], vec![0, 3, 4])));
    assert_eq!(ordered.shortest(5, 0, meter).unwrap(), None);
    assert_eq!(ordered.shortest(3, 2, meter).unwrap(), Some((vec![3, 2], vec![1])));
}

#[test]
fn a_route_ends_at_the_place_nearest_its_point_or_the_leftmost_or_rightmost_the_first_of_ties() {
    let graph = graph(false, &[(0, &[(0.0, 5.0), (0.0, 0.0), (5.0, 0.0)]), (1, &[(5.0, 5.0), (4.0, 5.0)])]);
    assert_eq!(graph.end(None, true, None), Some(0));
    assert_eq!(graph.end(None, false, None), Some(2));
    assert_eq!(graph.end(Some(p(4.4, 4.0)), true, None), Some(4));
    assert_eq!(graph.end(Some(p(2.5, 2.5)), false, Some(&[3, 2, 1, 0])), Some(3), "4 places as near: the first listed");
    assert_eq!(graph.end(None, false, Some(&[1, 0])), Some(1));
    assert_eq!(graph.end(None, true, Some(&[])), None);
}

#[test]
fn the_way_with_fewest_links_is_found_from_both_ends_at_once() {
    let meter = &mut Budget::DEFAULT.meter();
    // From the start, 3 places at once; then from the end, which meets them at the third.
    let star = graph(false, &[(0, &[(0.0, 0.0), (1.0, 1.0)]), (1, &[(0.0, 0.0), (1.0, 0.0)]), (2, &[(0.0, 0.0), (1.0, -1.0), (2.0, 0.0)])]);
    assert_eq!(star.shortest(0, 4, meter).unwrap(), Some((vec![0, 3, 4], vec![2, 3])));
    assert_eq!(star.shortest(4, 4, meter).unwrap(), Some((vec![4], Vec::new())));
    // No way between 2 groups.
    let apart = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0)]), (1, &[(5.0, 0.0), (6.0, 0.0)])]);
    assert_eq!(apart.shortest(0, 3, meter).unwrap(), None);
    // A long chain: each side looks in turn, and the ways meet in the middle.
    let chain = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 0.0)])]);
    assert_eq!(chain.shortest(0, 4, meter).unwrap(), Some((vec![0, 1, 2, 3, 4], vec![0, 1, 2, 3])));
    assert_eq!(chain.shortest(4, 0, meter).unwrap(), Some((vec![4, 3, 2, 1, 0], vec![3, 2, 1, 0])));
}

#[test]
fn the_rest_is_walked_depth_first_from_each_place_of_the_way_out_and_back() {
    // A triangle at the start, and the way from it: out round the triangle, there and back along the link
    // that closes it, back, then along the way.
    let triangle = graph(false, &[(0, &[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (0.0, 0.0)]), (1, &[(0.0, 0.0), (-1.0, 0.0)])]);
    let meter = &mut Budget::DEFAULT.meter();
    let (places, links) = triangle.shortest(0, 3, meter).unwrap().unwrap();
    let steps: Vec<(usize, usize, usize)> = triangle.walk(&places, &links, meter).unwrap().iter().map(|s| (s.from, s.to, s.link)).collect();
    assert_eq!(steps, [(0, 1, 0), (1, 2, 1), (2, 0, 2), (0, 2, 2), (2, 1, 1), (1, 0, 0), (0, 3, 3)]);
    // A piece from a place to itself is walked over and not sewn.
    let mut looped = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0)])]);
    looped.piece(1, 0, [p(1.0, 0.0), p(1.0, 0.0)]);
    let steps: Vec<(usize, usize)> = looped.walk(&[0, 1], &[0], meter).unwrap().iter().map(|s| (s.from, s.to)).collect();
    assert_eq!(steps, [(0, 1)]);
}

#[test]
fn a_copy_without_directions_lists_the_links_with_earlier_places_first() {
    // Place 2's own links come after its link with place 0, added later.
    let mut graph = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0)]), (1, &[(2.0, 0.0), (3.0, 0.0)])]);
    graph.piece(2, 0, [p(3.0, 0.0), p(0.0, 0.0)]);
    let lists = graph.undirected(&mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(lists, [vec![(1, 0), (3, 2)], vec![(0, 0)], vec![(3, 1)], vec![(0, 2), (2, 1)]]);
}

#[test]
fn a_group_s_root_is_found_and_the_way_to_it_halved() {
    let mut joined = [0, 0, 1, 2];
    assert_eq!(root(&mut joined, 3, &mut Budget::DEFAULT.meter()).unwrap(), 0);
    assert_eq!(joined, [0, 0, 1, 1]);
    assert_eq!(root(&mut joined, 0, &mut Budget::DEFAULT.meter()).unwrap(), 0);
}

#[test]
fn every_search_charges_the_meter() {
    let chain = graph(false, &[(0, &[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]), (1, &[(5.0, 0.0), (6.0, 0.0)])]);
    let small = Budget { max_work: 1, ..Budget::DEFAULT };
    assert!(chain.clone().join_apart(&mut small.meter()).is_err());
    assert!(chain.places_on(0, &mut small.meter()).is_err());
    assert!(chain.shortest(0, 2, &mut small.meter()).is_err());
    assert!(chain.walk(&[0], &[], &mut small.meter()).is_err());
    assert!(root(&mut [0, 0], 1, &mut Budget { max_work: 0, ..Budget::DEFAULT }.meter()).is_err());
}
