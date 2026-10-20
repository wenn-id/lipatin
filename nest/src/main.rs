//! Nesting prototype for Lipatin: how many copies of a real dieline outline fit on a plano sheet,
//! compared with today's bounding-box count from `impose()`.
//!
//! The outline is the union of the panels exported by `tests/export-outline.mjs`. Copies are placed on a
//! lattice (one motif repeated along two vectors). A motif is the outline at 0° or 90°, or a pair of the
//! outline with a copy turned 180° and slid into it, which is how tuck-end boxes interlock on a press sheet.
//! Every placement is checked again at the end: all pairs at least `gap` apart and inside the usable area.
//!
//!   cargo run --release -- jobs.json [out-dir]

use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use rayon::prelude::*;
use serde::Deserialize;
use std::{fmt::Write as _, fs, path::PathBuf, time::Instant};

type Pt = [f64; 2];

#[derive(Deserialize)]
struct Sheet { w: f64, h: f64, margin: f64, gap: f64 }

#[derive(Deserialize)]
struct Job {
    model: String,
    name: String,
    size: [f64; 3],
    #[serde(rename = "bboxCount")]
    bbox_count: usize,
    sheet: Sheet,
    polys: Vec<Vec<Pt>>,
}

/// A closed polygon with its bounding box, in mm.
#[derive(Clone)]
struct Shape { pts: Vec<Pt>, bb: [f64; 4], copy: usize }

impl Shape {
    fn new(pts: Vec<Pt>) -> Self {
        let mut bb = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for p in &pts { bb[0] = bb[0].min(p[0]); bb[1] = bb[1].min(p[1]); bb[2] = bb[2].max(p[0]); bb[3] = bb[3].max(p[1]); }
        Shape { pts, bb, copy: 0 }
    }
    fn map(&self, f: impl Fn(Pt) -> Pt) -> Self { Shape { copy: self.copy, ..Shape::new(self.pts.iter().map(|&p| f(p)).collect()) } }
    fn of_copy(mut self, copy: usize) -> Self { self.copy = copy; self }
    fn shifted(&self, d: Pt) -> Self { self.map(|p| [p[0] + d[0], p[1] + d[1]]) }
}

/// One or more boxes placed together; the unit that the lattice repeats. A box can be several shapes
/// when its panels only touch at a corner, so each shape carries the index of the box it belongs to.
#[derive(Clone)]
struct Motif { parts: Vec<Shape>, bb: [f64; 4], label: String, copies: usize }

impl Motif {
    fn new(parts: Vec<Shape>, label: String) -> Self {
        let mut bb = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for s in &parts { bb[0] = bb[0].min(s.bb[0]); bb[1] = bb[1].min(s.bb[1]); bb[2] = bb[2].max(s.bb[2]); bb[3] = bb[3].max(s.bb[3]); }
        let copies = parts.iter().map(|s| s.copy).max().map_or(0, |c| c + 1);
        Motif { parts, bb, label, copies }
    }
    fn w(&self) -> f64 { self.bb[2] - self.bb[0] }
    fn h(&self) -> f64 { self.bb[3] - self.bb[1] }
}

// ---------- geometry ----------

fn cross(o: Pt, a: Pt, b: Pt) -> f64 { (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]) }

fn point_seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
    ((p[0] - a[0] - t * dx).powi(2) + (p[1] - a[1] - t * dy).powi(2)).sqrt()
}

fn seg_seg_dist(a: Pt, b: Pt, c: Pt, d: Pt) -> f64 {
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) { return 0.0; }
    point_seg_dist(a, c, d).min(point_seg_dist(b, c, d)).min(point_seg_dist(c, a, b)).min(point_seg_dist(d, a, b))
}

fn inside(p: Pt, s: &Shape, off: Pt) -> bool {
    let n = s.pts.len();
    let mut hit = false;
    for i in 0..n {
        let a = [s.pts[i][0] + off[0], s.pts[i][1] + off[1]];
        let b = [s.pts[(i + 1) % n][0] + off[0], s.pts[(i + 1) % n][1] + off[1]];
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] { hit = !hit; }
    }
    hit
}

/// True when shape `a` and shape `b` moved by `d` are at least `gap` apart (and neither contains the other).
fn separated(a: &Shape, b: &Shape, d: Pt, gap: f64) -> bool {
    let bb = [b.bb[0] + d[0], b.bb[1] + d[1], b.bb[2] + d[0], b.bb[3] + d[1]];
    if bb[0] >= a.bb[2] + gap || a.bb[0] >= bb[2] + gap || bb[1] >= a.bb[3] + gap || a.bb[1] >= bb[3] + gap { return true; }
    let (na, nb) = (a.pts.len(), b.pts.len());
    let lim = gap - 1e-9;
    for i in 0..na {
        let (p, q) = (a.pts[i], a.pts[(i + 1) % na]);
        let sb = [p[0].min(q[0]) - gap, p[1].min(q[1]) - gap, p[0].max(q[0]) + gap, p[1].max(q[1]) + gap];
        if sb[2] < bb[0] || sb[0] > bb[2] || sb[3] < bb[1] || sb[1] > bb[3] { continue; }
        for j in 0..nb {
            let r = [b.pts[j][0] + d[0], b.pts[j][1] + d[1]];
            let s = [b.pts[(j + 1) % nb][0] + d[0], b.pts[(j + 1) % nb][1] + d[1]];
            if r[0].max(s[0]) < sb[0] || r[0].min(s[0]) > sb[2] || r[1].max(s[1]) < sb[1] || r[1].min(s[1]) > sb[3] { continue; }
            if seg_seg_dist(p, q, r, s) < lim { return false; }
        }
    }
    !(inside(a.pts[0], b, d) || inside([b.pts[0][0] + d[0], b.pts[0][1] + d[1]], a, [0.0, 0.0]))
}

fn motif_separated(m: &Motif, d: Pt, gap: f64) -> bool {
    let bb = m.bb;
    if d[0].abs() >= bb[2] - bb[0] + gap || d[1].abs() >= bb[3] - bb[1] + gap { return true; }
    m.parts.iter().all(|a| m.parts.iter().all(|b| separated(a, b, d, gap)))
}

/// Union of the panels, outer rings only. Panels share edges, so this is the die's outer cut line.
fn outline(polys: &[Vec<Pt>]) -> Vec<Shape> {
    let mut acc = MultiPolygon::<f64>::new(vec![]);
    for p in polys {
        // snap to 0.001 mm so edges shared by two panels coincide exactly and merge
        let snap = |v: f64| (v * 1000.0).round() / 1000.0;
        let ring: Vec<Coord<f64>> = p.iter().map(|q| Coord { x: snap(q[0]), y: snap(q[1]) }).collect();
        acc = acc.union(&MultiPolygon::new(vec![Polygon::new(LineString::from(ring), vec![])]));
    }
    acc.0.iter().map(|poly| {
        let mut pts: Vec<Pt> = poly.exterior().0.iter().map(|c| [c.x, c.y]).collect();
        pts.pop(); // closing point
        // drop collinear points left by shared panel edges
        let mut i = 0;
        while pts.len() > 3 && i < pts.len() {
            let n = pts.len();
            if cross(pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]).abs() < 1e-7 { pts.remove(i); } else { i += 1; }
        }
        Shape::new(pts)
    }).collect()
}

fn normalized(parts: &[Shape], f: impl Fn(Pt) -> Pt + Copy) -> Vec<Shape> {
    let moved: Vec<Shape> = parts.iter().map(|s| s.map(f)).collect();
    let m = Motif::new(moved.clone(), String::new());
    moved.iter().map(|s| s.shifted([-m.bb[0], -m.bb[1]])).collect()
}

// ---------- search ----------

/// Smallest x in [lo, hi] with ok(x), assuming ok(hi) and that ok stays true once it turns true.
fn smallest(lo: f64, hi: f64, ok: impl Fn(f64) -> bool) -> f64 {
    let (mut lo, mut hi) = (lo, hi);
    if ok(lo) { return lo; }
    while hi - lo > 0.05 { let mid = (lo + hi) / 2.0; if ok(mid) { hi = mid; } else { lo = mid; } }
    hi
}

/// Checks every lattice copy whose box comes within `gap` of the copy at the origin, however many rows that spans.
fn lattice_ok(m: &Motif, ax: f64, b: Pt, gap: f64) -> bool {
    let (rw, rh) = (m.w() + gap, m.h() + gap);
    if b[1] <= 1e-6 { return false; }
    let mut j = 0;
    while j as f64 * b[1] < rh {
        let shift = j as f64 * b[0];
        let i0 = if j == 0 { 1 } else { ((-rw - shift) / ax).floor() as i64 };
        let i1 = ((rw - shift) / ax).ceil() as i64;
        for i in i0..=i1 {
            if !motif_separated(m, [i as f64 * ax + shift, j as f64 * b[1]], gap) { return false; }
        }
        j += 1;
    }
    true
}

struct Layout { count: usize, motif: Motif, offsets: Vec<Pt> }

/// Places the lattice on the usable area and keeps the horizontal phase that fits the most copies.
fn place(m: &Motif, ax: f64, b: Pt, uw: f64, uh: f64) -> (usize, Vec<Pt>) {
    let per = m.copies;
    let mut best = (0, vec![]);
    for k in 0..24 {
        let sx = ax * k as f64 / 24.0;
        let mut offs = vec![];
        let mut j = 0;
        while j as f64 * b[1] + m.h() <= uh + 1e-9 {
            let oy = j as f64 * b[1] - m.bb[1];
            let ox = sx + j as f64 * b[0] - m.bb[0];
            let i0 = ((-ox) / ax).ceil() as i64;
            let i1 = ((uw - m.w() - ox) / ax + 1e-9).floor() as i64;
            for i in i0..=i1 { offs.push([ox + i as f64 * ax, oy]); }
            j += 1;
        }
        if offs.len() * per > best.0 { best = (offs.len() * per, offs); }
    }
    best
}

fn best_lattice(m: &Motif, uw: f64, uh: f64, gap: f64) -> Layout {
    let ax = smallest(0.0, m.w() + gap, |x| x > 1e-6 && (1..=((m.w() + gap) / x).ceil() as i64).all(|i| motif_separated(m, [i as f64 * x, 0.0], gap)));
    let scan = |xs: Vec<f64>| -> Vec<(f64, f64)> {
        xs.into_par_iter().map(|bx| (bx, smallest(0.0, m.h() + gap, |by| lattice_ok(m, ax, [bx, by], gap)))).collect()
    };
    let coarse = scan((0..(ax / 2.0).ceil() as usize).map(|i| i as f64 * 2.0).filter(|&x| x < ax).collect());
    let score = |&(bx, by): &(f64, f64)| place(m, ax, [bx, by], uw, uh).0;
    let mut top: Vec<(f64, f64)> = coarse.clone();
    top.sort_by(|p, q| score(q).cmp(&score(p)).then(p.1.partial_cmp(&q.1).unwrap()));
    let fine: Vec<f64> = top.iter().take(4).flat_map(|&(bx, _)| (-8..=8).map(move |k| bx + k as f64 * 0.25)).filter(|&x| x >= 0.0 && x < ax).collect();
    let mut all = coarse;
    all.extend(scan(fine));
    let &(bx, by) = all.iter().max_by(|p, q| score(p).cmp(&score(q)).then(q.1.partial_cmp(&p.1).unwrap())).unwrap();
    let (count, offsets) = place(m, ax, [bx, by], uw, uh);
    Layout { count, motif: m.clone(), offsets }
}

/// The outline plus a 180°-turned copy slid into it from below or from the side, best few by pair area.
fn pairs(base: &[Shape], gap: f64, label: &str) -> Vec<Motif> {
    let a = Motif::new(base.to_vec(), String::new());
    let (w, h) = (a.w(), a.h());
    let turned = normalized(base, |p| [-p[0], -p[1]]);
    let fits = |d: Pt| a.parts.iter().all(|p| turned.iter().all(|q| separated(p, q, d, gap)));
    let mut cands: Vec<(f64, Pt)> = vec![];
    let below: Vec<(f64, Pt)> = (0..=(2.0 * w) as usize).step_by(2).collect::<Vec<_>>().into_par_iter().map(|i| {
        let tx = i as f64 - w; let ty = smallest(0.0, h + gap, |y| fits([tx, y])); ((w.max(tx + w) - 0f64.min(tx)) * (h.max(ty + h)), [tx, ty])
    }).collect();
    let beside: Vec<(f64, Pt)> = (0..=(2.0 * h) as usize).step_by(2).collect::<Vec<_>>().into_par_iter().map(|i| {
        let ty = i as f64 - h; let tx = smallest(0.0, w + gap, |x| fits([x, ty])); ((w.max(tx + w)) * (h.max(ty + h) - 0f64.min(ty)), [tx, ty])
    }).collect();
    for set in [below, beside] {
        let mut s = set; s.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
        cands.extend(s.into_iter().take(3));
    }
    cands.into_iter().map(|(_, d)| {
        let mut parts = base.to_vec();
        parts.extend(turned.iter().map(|s| s.shifted(d).of_copy(1)));
        let m = Motif::new(parts.clone(), String::new());
        Motif::new(parts.iter().map(|s| s.shifted([-m.bb[0], -m.bb[1]])).collect(), format!("{label} + 180°"))
    }).collect()
}

fn check(l: &Layout, uw: f64, uh: f64, gap: f64) -> Result<(), String> {
    // (box number on the sheet, shape) for every shape placed
    let placed: Vec<(usize, Shape)> = l.offsets.iter().enumerate()
        .flat_map(|(k, &o)| l.motif.parts.iter().map(move |s| (k * l.motif.copies + s.copy, s.shifted(o)))).collect();
    for (i, (bi, s)) in placed.iter().enumerate() {
        if s.bb[0] < -1e-6 || s.bb[1] < -1e-6 || s.bb[2] > uw + 1e-6 || s.bb[3] > uh + 1e-6 { return Err(format!("box {bi} leaves the usable area")); }
        for (bj, t) in &placed[i + 1..] {
            if bi != bj && !separated(s, t, [0.0, 0.0], gap - 1e-6) { return Err(format!("box {bi} is closer than {gap} mm to box {bj}")); }
        }
    }
    Ok(())
}

fn svg(job: &Job, l: &Layout) -> String {
    let (sw, sh, m) = (job.sheet.w, job.sheet.h, job.sheet.margin);
    let mut s = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-10 -10 {} {}" width="{}mm" height="{}mm"><rect width="{sw}" height="{sh}" fill="#fff" stroke="#17251D" stroke-width="1.5"/><rect x="{m}" y="{m}" width="{}" height="{}" fill="none" stroke="#999" stroke-dasharray="8 6"/>"##,
        sw + 20.0, sh + 20.0, (sw + 20.0) / 4.0, (sh + 20.0) / 4.0, sw - 2.0 * m, sh - 2.0 * m);
    for &o in &l.offsets {
        for p in &l.motif.parts {
            let d: String = p.pts.iter().enumerate().map(|(i, q)| format!("{}{:.2} {:.2}", if i == 0 { 'M' } else { 'L' }, q[0] + o[0] + m, q[1] + o[1] + m)).collect();
            write!(s, r##"<path d="{d}Z" fill="#F3EBDD" stroke="#C9105A" stroke-width="1"/>"##).unwrap();
        }
    }
    s + "</svg>"
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().unwrap_or_else(|| "jobs.json".into());
    let out = PathBuf::from(args.next().unwrap_or_else(|| "out".into()));
    fs::create_dir_all(&out).unwrap();
    let jobs: Vec<Job> = serde_json::from_str(&fs::read_to_string(&input).expect("read jobs")).expect("parse jobs");

    println!("{:<18} {:>14} {:>9} {:>8} {:>7}  {:<14} {:>6} {}", "model", "size (mm)", "bbox now", "nested", "gain", "motif", "pieces", "time");
    for job in &jobs {
        let t0 = Instant::now();
        let (uw, uh, gap) = (job.sheet.w - 2.0 * job.sheet.margin, job.sheet.h - 2.0 * job.sheet.margin, job.sheet.gap);
        let base = outline(&job.polys);
        let pieces = base.len();
        let at0 = normalized(&base, |p| p);
        let at90 = normalized(&base, |p| [-p[1], p[0]]);
        let mut motifs = vec![Motif::new(at0.clone(), "0°".into()), Motif::new(at90.clone(), "90°".into())];
        motifs.extend(pairs(&at0, gap, "0°"));
        motifs.extend(pairs(&at90, gap, "90°"));
        let best = motifs.iter().filter(|m| m.w() <= uw && m.h() <= uh).map(|m| best_lattice(m, uw, uh, gap)).max_by_key(|l| l.count);
        let size = format!("{}×{}×{}", job.size[0], job.size[1], job.size[2]);
        match best {
            Some(l) if l.count > 0 => {
                let verdict = check(&l, uw, uh, gap);
                let gain = if job.bbox_count > 0 { format!("{:+.0}%", (l.count as f64 / job.bbox_count as f64 - 1.0) * 100.0) } else { "n/a".into() };
                println!("{:<18} {:>14} {:>9} {:>8} {:>7}  {:<14} {:>6} {:.1}s{}", job.name, size, job.bbox_count, l.count, gain, l.motif.label, pieces, t0.elapsed().as_secs_f64(),
                    verdict.err().map(|e| format!("  INVALID: {e}")).unwrap_or_default());
                fs::write(out.join(format!("{}-{}x{}x{}.svg", job.model, job.size[0], job.size[1], job.size[2])), svg(job, &l)).unwrap();
            }
            _ => println!("{:<18} {:>14} {:>9} {:>8}", job.name, size, job.bbox_count, 0),
        }
    }
}
