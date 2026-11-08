use super::*;
use crate::route::tests::{p, stroke};
use crate::route::{Route, Tool, route};
use crate::design::DesignSettings;
use stitchcraft_core::Budget;

fn show(elements: &[Element], options: Options) {
    let routed = route(elements, &Route { tool: Tool::Runs, options, start: None, end: None }, &DesignSettings::default(), &Budget::DEFAULT);
    for piece in &routed.pieces {
        let points: Vec<String> = piece.element.shape.path().points().map(|q| format!("({:.4} {:.4})", q.x(), q.y())).collect();
        let params: Vec<String> = piece.element.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!("{} from {} under {} [{}] {}", piece.element.name.clone().unwrap_or_default(), piece.from, piece.under, params.join(" "), points.join(" "));
    }
    println!("{:?}", routed.diagnostics);
}

#[test]
fn scratch_networks() {
    let _ = p(0.0, 0.0);
    println!("== net1");
    let net1 = [
        stroke("e0", &[(10.0, 32.0), (48.0, 32.0)], &[]),
        stroke("e1", &[(16.0, 12.0), (16.0, 35.0)], &[]),
        stroke("e2", &[(16.0, 18.0), (30.0, 32.0)], &[]),
        stroke("e3", &[(32.0, 10.0), (44.0, 18.0), (44.0, 8.0), (34.0, 16.0)], &[]),
    ];
    show(&net1, Tool::Runs.defaults());
    println!("== net1 trim");
    show(&net1, Options { trim: true, ..Tool::Runs.defaults() });
    println!("== net1 unsplit");
    show(&net1, Options { break_up: false, ..Tool::Runs.defaults() });
    println!("== net1 in order");
    show(&net1, Options { preserve_order: true, ..Tool::Runs.defaults() });
    println!("== net7 trim");
    let net7 = [
        stroke("a", &[(10.0, 50.0), (50.0, 50.0)], &[]),
        stroke("b", &[(30.0, 55.0), (35.0, 60.0)], &[]),
        stroke("c", &[(29.6, 55.35), (22.0, 59.0)], &[]),
    ];
    show(&net7, Options { trim: true, ..Tool::Runs.defaults() });
}
