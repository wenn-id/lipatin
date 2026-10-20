//! Real patterns from Lipatin's engine (written by `node tests/export-nest-fixtures.mjs`), nested on 65×100 cm with
//! margin 10 and gap 6. The nested count must never lose to `impose()`, and every layout must pass the final check.

use lipatin_nest::{Pt, Sheet, check, nest};
use serde::Deserialize;

#[derive(Deserialize)]
struct Job {
    name: String,
    sheet: Sheet,
    #[serde(rename = "bboxCount")]
    bbox_count: usize,
    polys: Vec<Vec<Pt>>,
}

fn jobs() -> Vec<Job> { serde_json::from_str(include_str!("fixtures.json")).unwrap() }

fn count(name: &str) -> (usize, usize, String) {
    let job = jobs().into_iter().find(|j| j.name == name).unwrap_or_else(|| panic!("no fixture {name}"));
    let l = nest(&job.polys, &job.sheet).expect("at least one box fits");
    (l.count, job.bbox_count, l.label)
}

#[test]
fn never_below_impose_and_always_valid() {
    let jobs = jobs();
    assert!(jobs.len() >= 10);
    for job in &jobs {
        let l = nest(&job.polys, &job.sheet).unwrap_or_else(|| panic!("{}: nothing fits", job.name));
        let (uw, uh) = job.sheet.usable();
        check(&l, uw, uh, job.sheet.gap).unwrap_or_else(|e| panic!("{}: {e}", job.name));
        assert!(l.count >= job.bbox_count, "{}: nested {} < impose {}", job.name, l.count, job.bbox_count);
        assert_eq!(l.placements(job.sheet.margin).len(), l.count, "{}", job.name);
    }
}

#[test]
fn reverse_tuck_interlocks() {
    let (n, bbox, _) = count("rte 90x60x30");
    assert_eq!((bbox, n), (9, 12));
}

// impose() mixes a rotated and an unrotated block here; a single lattice used to lose (9, 36, 112)
#[test]
fn mixed_blocks_catch_up_with_impose() {
    for (name, want) in [("tray 150x100x50", 11), ("ste 40x30x15", 42), ("tray 40x30x15", 116)] {
        let (n, bbox, label) = count(name);
        assert!(n >= want && n >= bbox, "{name}: {n} ({label}), impose {bbox}");
    }
    let (_, _, label) = count("tray 150x100x50");
    assert!(label.contains(" | "), "tray 150x100x50 should use two blocks, got {label}");
}

// a deep hardbox: the wrap's side arms are as long as the base is wide, so crosses interlock
#[test]
fn hardbox_wrap_interlocks_on_deep_boxes() {
    let (n, bbox, _) = count("hb_lid 60x60x60 wrap: Pembungkus alas");
    assert_eq!(bbox, 8);
    assert!(n >= 12, "{n}");
    let (n, bbox, _) = count("hb_shoulder 60x60x60 board: Alas · board V-groove");
    assert!(n >= 32 && bbox == 24, "{n} vs {bbox}");
}
