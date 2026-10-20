//! Nesting core for Lipatin: how many copies of a real dieline outline fit on a plano sheet.
//!
//! The outline is the union of the pattern's panels. Copies are placed on a lattice (one motif repeated along
//! two vectors). A motif is the outline at 0° or 90°, or a pair of the outline with a copy turned 180° and slid
//! into it, which is how tuck-end boxes interlock on a press sheet. The sheet can also be split into two blocks
//! side by side or one above the other, each with its own motif, the way `impose()` in index.html mixes a
//! rotated and an unrotated block. Every layout is checked again at the end: all boxes at least `gap` apart
//! and inside the usable area.

use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use serde::{Deserialize, Serialize};

pub type Pt = [f64; 2];

/// With no gap at all, two copies laid exactly on top of each other look "separated" (every edge only touches),
/// so the search and the check always keep at least this much between boxes.
const MIN_GAP: f64 = 0.01;

#[derive(Deserialize, Clone, Copy)]
pub struct Sheet { pub w: f64, pub h: f64, pub margin: f64, pub gap: f64 }

impl Sheet {
    pub fn usable(&self) -> (f64, f64) { (self.w - 2.0 * self.margin, self.h - 2.0 * self.margin) }
}

fn bbox<'a>(pts: impl Iterator<Item = &'a Pt>) -> [f64; 4] {
    let mut bb = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for p in pts { bb[0] = bb[0].min(p[0]); bb[1] = bb[1].min(p[1]); bb[2] = bb[2].max(p[0]); bb[3] = bb[3].max(p[1]); }
    bb
}

/// Turns a point by a multiple of 90°, the same way SVG `rotate()` does with y pointing down.
fn turn(p: Pt, deg: u32) -> Pt {
    match deg % 360 { 90 => [-p[1], p[0]], 180 => [-p[0], -p[1]], 270 => [p[1], -p[0]], _ => p }
}

/// A closed polygon with its bounding box, in mm. `rot` and `t` say where it sits relative to the pattern as
/// drawn in index.html: `pts = turn(original, rot) + t`.
#[derive(Clone)]
pub struct Shape { pub pts: Vec<Pt>, pub bb: [f64; 4], pub copy: usize, pub rot: u32, pub t: Pt }

impl Shape {
    pub fn new(pts: Vec<Pt>) -> Self { Shape { bb: bbox(pts.iter()), pts, copy: 0, rot: 0, t: [0.0, 0.0] } }
    fn of_copy(mut self, copy: usize) -> Self { self.copy = copy; self }
    pub fn shifted(&self, d: Pt) -> Self {
        let pts = self.pts.iter().map(|p| [p[0] + d[0], p[1] + d[1]]).collect();
        Shape { t: [self.t[0] + d[0], self.t[1] + d[1]], ..Shape { copy: self.copy, rot: self.rot, ..Shape::new(pts) } }
    }
    fn turned(&self, deg: u32) -> Self {
        let pts = self.pts.iter().map(|&p| turn(p, deg)).collect();
        Shape { rot: (self.rot + deg) % 360, t: turn(self.t, deg), copy: self.copy, ..Shape::new(pts) }
    }
}

/// One or more boxes placed together; the unit that a lattice repeats. A box can be several shapes when its
/// panels only touch at a corner, so each shape carries the index of the box it belongs to.
#[derive(Clone)]
pub struct Motif { pub parts: Vec<Shape>, pub bb: [f64; 4], pub label: String, pub copies: usize }

impl Motif {
    pub fn new(parts: Vec<Shape>, label: String) -> Self {
        let bb = bbox(parts.iter().flat_map(|s| [[s.bb[0], s.bb[1]], [s.bb[2], s.bb[3]]]).collect::<Vec<_>>().iter());
        let copies = parts.iter().map(|s| s.copy).max().map_or(0, |c| c + 1);
        Motif { parts, bb, label, copies }
    }
    pub fn w(&self) -> f64 { self.bb[2] - self.bb[0] }
    pub fn h(&self) -> f64 { self.bb[3] - self.bb[1] }
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
pub fn separated(a: &Shape, b: &Shape, d: Pt, gap: f64) -> bool {
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
pub fn outline(polys: &[Vec<Pt>]) -> Vec<Shape> {
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

/// The parts turned by `deg` and moved so their common bounding box starts at the origin.
fn normalized(parts: &[Shape], deg: u32) -> Vec<Shape> {
    let turned: Vec<Shape> = parts.iter().map(|s| s.turned(deg)).collect();
    let m = Motif::new(turned.clone(), String::new());
    turned.iter().map(|s| s.shifted([-m.bb[0], -m.bb[1]])).collect()
}

// ---------- search ----------

#[cfg(not(target_arch = "wasm32"))]
fn par_map<T: Send, R: Send>(v: Vec<T>, f: impl Fn(T) -> R + Sync + Send) -> Vec<R> {
    use rayon::prelude::*;
    v.into_par_iter().map(f).collect()
}

// wasm32-unknown-unknown has no threads; the browser runs this inside a Web Worker instead.
#[cfg(target_arch = "wasm32")]
fn par_map<T, R>(v: Vec<T>, f: impl Fn(T) -> R) -> Vec<R> { v.into_iter().map(f).collect() }

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

/// A motif repeated every `ax` along x, and every `b` from one row to the next.
#[derive(Clone, Copy)]
struct Lattice<'a> { m: &'a Motif, ax: f64, b: Pt }

impl Lattice<'_> {
    /// Offsets of the motif copies inside a `uw` × `uh` area, for the horizontal phase that fits the most.
    fn place(&self, uw: f64, uh: f64) -> Vec<Pt> {
        let (m, ax, b) = (self.m, self.ax, self.b);
        let mut best: Vec<Pt> = vec![];
        if m.w() > uw + 1e-9 || m.h() > uh + 1e-9 { return best; }
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
            if offs.len() > best.len() { best = offs; }
        }
        best
    }
    fn count(&self, uw: f64, uh: f64) -> usize { self.place(uw, uh).len() * self.m.copies }
}

/// The densest lattice for the whole usable area, plus the plain grid (rows straight above each other),
/// which is what a narrow strip next to another block usually wants.
fn lattices(m: &Motif, uw: f64, uh: f64, gap: f64) -> Vec<Lattice<'_>> {
    let ax = smallest(0.0, m.w() + gap, |x| x > 1e-6 && (1..=((m.w() + gap) / x).ceil() as i64).all(|i| motif_separated(m, [i as f64 * x, 0.0], gap)));
    let scan = |xs: Vec<f64>| -> Vec<(f64, f64)> {
        par_map(xs, |bx| (bx, smallest(0.0, m.h() + gap, |by| lattice_ok(m, ax, [bx, by], gap))))
    };
    let coarse = scan((0..(ax / 2.0).ceil() as usize).map(|i| i as f64 * 2.0).filter(|&x| x < ax).collect());
    let grid = Lattice { m, ax, b: [0.0, coarse[0].1] };
    let score = |&(bx, by): &(f64, f64)| Lattice { m, ax, b: [bx, by] }.count(uw, uh);
    let mut top: Vec<(f64, f64)> = coarse.clone();
    top.sort_by(|p, q| score(q).cmp(&score(p)).then(p.1.total_cmp(&q.1)));
    let fine: Vec<f64> = top.iter().take(4).flat_map(|&(bx, _)| (-8..=8).map(move |k| bx + k as f64 * 0.25)).filter(|&x| x >= 0.0 && x < ax).collect();
    let mut all = coarse;
    all.extend(scan(fine));
    let &(bx, by) = all.iter().max_by(|p, q| score(p).cmp(&score(q)).then(q.1.total_cmp(&p.1))).unwrap();
    vec![Lattice { m, ax, b: [bx, by] }, grid]
}

/// The outline plus a 180°-turned copy slid into it from below or from the side, best few by pair area.
fn pairs(base: &[Shape], gap: f64, label: &str) -> Vec<Motif> {
    let a = Motif::new(base.to_vec(), String::new());
    let (w, h) = (a.w(), a.h());
    let turned = normalized(base, 180);
    let fits = |d: Pt| a.parts.iter().all(|p| turned.iter().all(|q| separated(p, q, d, gap)));
    let mut cands: Vec<(f64, Pt)> = vec![];
    let below = par_map((0..=(2.0 * w) as usize).step_by(2).collect(), |i| {
        let tx = i as f64 - w; let ty = smallest(0.0, h + gap, |y| fits([tx, y])); ((w.max(tx + w) - 0f64.min(tx)) * (h.max(ty + h)), [tx, ty])
    });
    let beside = par_map((0..=(2.0 * h) as usize).step_by(2).collect(), |i| {
        let ty = i as f64 - h; let tx = smallest(0.0, w + gap, |x| fits([x, ty])); ((w.max(tx + w)) * (h.max(ty + h) - 0f64.min(ty)), [tx, ty])
    });
    for set in [below, beside] {
        let mut s = set; s.sort_by(|p, q| p.0.total_cmp(&q.0));
        cands.extend(s.into_iter().take(3));
    }
    cands.into_iter().map(|(_, d)| {
        let mut parts = base.to_vec();
        parts.extend(turned.iter().map(|s| s.shifted(d).of_copy(1)));
        let m = Motif::new(parts.clone(), String::new());
        Motif::new(parts.iter().map(|s| s.shifted([-m.bb[0], -m.bb[1]])).collect(), format!("{label} + 180°"))
    }).collect()
}

/// Motif copies placed at `offsets`, in usable-area coordinates.
#[derive(Clone)]
pub struct Block { pub motif: Motif, pub offsets: Vec<Pt> }

#[derive(Clone)]
pub struct Layout { pub count: usize, pub label: String, pub pieces: usize, pub blocks: Vec<Block> }

/// Where one box goes on the sheet: draw the pattern with `translate(x y) rotate(rot)`.
#[derive(Serialize, Clone, Copy, Debug, PartialEq)]
pub struct Placement { pub x: f64, pub y: f64, pub rot: u32 }

impl Layout {
    /// (box number on the sheet, shape) for every shape placed, in usable-area coordinates.
    pub fn shapes(&self) -> Vec<(usize, Shape)> {
        let mut out = vec![];
        let mut first = 0;
        for b in &self.blocks {
            for (k, &o) in b.offsets.iter().enumerate() {
                out.extend(b.motif.parts.iter().map(|s| (first + k * b.motif.copies + s.copy, s.shifted(o))));
            }
            first += b.offsets.len() * b.motif.copies;
        }
        out
    }

    /// One placement per box, in sheet coordinates (the margin added back).
    pub fn placements(&self, margin: f64) -> Vec<Placement> {
        let mut seen = vec![false; self.count];
        let mut out = vec![Placement { x: 0.0, y: 0.0, rot: 0 }; self.count];
        for (i, s) in self.shapes() {
            if !seen[i] { seen[i] = true; out[i] = Placement { x: s.t[0] + margin, y: s.t[1] + margin, rot: s.rot }; }
        }
        out
    }
}

/// Final validation: every box inside the usable area and at least `gap` from every other box.
pub fn check(l: &Layout, uw: f64, uh: f64, gap: f64) -> Result<(), String> {
    let gap = gap.max(MIN_GAP);
    let placed = l.shapes();
    for (i, (bi, s)) in placed.iter().enumerate() {
        if s.bb[0] < -1e-6 || s.bb[1] < -1e-6 || s.bb[2] > uw + 1e-6 || s.bb[3] > uh + 1e-6 { return Err(format!("box {bi} leaves the usable area")); }
        for (bj, t) in &placed[i + 1..] {
            if bi != bj && !separated(s, t, [0.0, 0.0], gap - 1e-6) { return Err(format!("box {bi} is closer than {gap} mm to box {bj}")); }
        }
    }
    Ok(())
}

fn span(m: &Motif, offs: &[Pt], axis: usize) -> (f64, f64) {
    offs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), o| (lo.min(o[axis] + m.bb[axis]), hi.max(o[axis] + m.bb[axis + 2])))
}

/// Two blocks split along x (axis 0) or y (axis 1): `a` packed against the start, `b` in what is left after
/// one gap. Only the narrowest strip for each count of `a` is tried, found on a 1 mm scan.
fn best_split(lats: &[Lattice], uw: f64, uh: f64, gap: f64, axis: usize) -> Option<Layout> {
    let full = if axis == 0 { uw } else { uh };
    let area = |len: f64| if axis == 0 { (len, uh) } else { (uw, len) };
    let strips: Vec<Vec<(usize, f64, Vec<Pt>)>> = par_map(lats.to_vec(), |a| {
        let mut levels: Vec<(usize, f64, Vec<Pt>)> = vec![];
        let mut len = if axis == 0 { a.m.w() } else { a.m.h() };
        while len < full - gap {
            let (w, h) = area(len);
            let offs = a.place(w, h);
            if !offs.is_empty() && levels.last().is_none_or(|l| offs.len() > l.0) {
                let (lo, hi) = span(a.m, &offs, axis);
                levels.push((offs.len(), hi - lo, offs.into_iter().map(|mut o| { o[axis] -= lo; o }).collect()));
            }
            len += 1.0;
        }
        levels
    });
    let mut best: Option<Layout> = None;
    for (ia, a) in lats.iter().enumerate() {
        for (n, used, offs_a) in &strips[ia] {
            for b in lats {
                let (w, h) = area(full - used - gap);
                let offs_b: Vec<Pt> = b.place(w, h).into_iter().map(|mut o| { o[axis] += used + gap; o }).collect();
                let count = n * a.m.copies + offs_b.len() * b.m.copies;
                if offs_b.is_empty() || best.as_ref().is_some_and(|l| l.count >= count) { continue; }
                best = Some(Layout {
                    count, label: format!("{} | {}", a.m.label, b.m.label), pieces: 0,
                    blocks: vec![Block { motif: a.m.clone(), offsets: offs_a.clone() }, Block { motif: b.m.clone(), offsets: offs_b }],
                });
            }
        }
    }
    best
}

/// Best validated layout of the pattern's panels on the sheet, or None when not even one box fits.
pub fn nest(polys: &[Vec<Pt>], sheet: &Sheet) -> Option<Layout> {
    let (uw, uh) = sheet.usable();
    let gap = sheet.gap.max(MIN_GAP);
    if uw <= 0.0 || uh <= 0.0 || polys.is_empty() { return None; }
    let base = outline(polys);
    let pieces = base.len();
    let at0 = normalized(&base, 0);
    let at90 = normalized(&base, 90);
    let mut motifs = vec![Motif::new(at0.clone(), "0°".into()), Motif::new(at90.clone(), "90°".into())];
    motifs.extend(pairs(&at0, gap, "0°"));
    motifs.extend(pairs(&at90, gap, "90°"));
    motifs.retain(|m| m.w() <= uw + 1e-9 && m.h() <= uh + 1e-9);
    let lats: Vec<Lattice> = motifs.iter().flat_map(|m| lattices(m, uw, uh, gap)).collect();
    let mut cands: Vec<Layout> = lats.iter().map(|l| {
        let offsets = l.place(uw, uh);
        Layout { count: offsets.len() * l.m.copies, label: l.m.label.clone(), pieces, blocks: vec![Block { motif: l.m.clone(), offsets }] }
    }).collect();
    cands.extend([0, 1].into_iter().filter_map(|axis| best_split(&lats, uw, uh, gap, axis)));
    cands.sort_by(|p, q| q.count.cmp(&p.count));
    cands.into_iter().filter(|l| l.count > 0).find(|l| check(l, uw, uh, gap).is_ok()).map(|l| Layout { pieces, ..l })
}

#[derive(Serialize)]
struct Answer { count: usize, motif: String, pieces: usize, placements: Vec<Placement> }

/// JSON in, JSON out, so the same entry point serves the CLI tests and the browser.
pub fn nest_to_json(polys: &[Vec<Pt>], sheet: &Sheet) -> String {
    let a = match nest(polys, sheet) {
        Some(l) => Answer { count: l.count, motif: l.label.clone(), pieces: l.pieces, placements: l.placements(sheet.margin) },
        None => Answer { count: 0, motif: String::new(), pieces: 0, placements: vec![] },
    };
    serde_json::to_string(&a).unwrap()
}

/// Browser entry point. `polys` is the JSON of `g.polys.map(p => p.pts)`; sizes in mm. Returns JSON
/// `{count, motif, pieces, placements: [{x, y, rot}]}`, or `{error}` when `polys` does not parse.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn nest_sheet(polys: &str, sheet_w: f64, sheet_h: f64, margin: f64, gap: f64) -> String {
    match serde_json::from_str::<Vec<Vec<Pt>>>(polys) {
        Ok(p) => nest_to_json(&p, &Sheet { w: sheet_w, h: sheet_h, margin, gap }),
        Err(e) => serde_json::json!({ "error": e.to_string() }).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<Pt> { vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]] }

    #[test]
    fn union_merges_panels_that_share_an_edge() {
        // a cross: four flaps around a centre panel, every flap sharing one full edge with it
        let polys = vec![rect(10.0, 10.0, 10.0, 10.0), rect(0.0, 10.0, 10.0, 10.0), rect(20.0, 10.0, 10.0, 10.0), rect(10.0, 0.0, 10.0, 10.0), rect(10.0, 20.0, 10.0, 10.0)];
        let o = outline(&polys);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].pts.len(), 12, "collinear points on shared edges are dropped");
        assert_eq!(o[0].bb, [0.0, 0.0, 30.0, 30.0]);
    }

    #[test]
    fn union_keeps_panels_that_only_touch_at_a_corner_apart() {
        let o = outline(&[rect(0.0, 0.0, 10.0, 10.0), rect(10.0, 10.0, 10.0, 10.0)]);
        assert_eq!(o.len(), 2);
    }

    #[test]
    fn union_snaps_edges_that_differ_by_rounding() {
        let o = outline(&[rect(0.0, 0.0, 10.0, 10.0), rect(10.0000002, 0.0, 10.0, 10.0)]);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].pts.len(), 4);
    }

    #[test]
    fn turn_matches_svg_rotate() {
        let s = Shape::new(rect(0.0, 0.0, 20.0, 10.0));
        let t = s.turned(90);
        assert_eq!(t.bb, [-10.0, 0.0, 0.0, 20.0]);
        // pts = turn(original, rot) + t holds after turning and shifting
        let moved = t.turned(180).shifted([5.0, 7.0]);
        for (p, q) in s.pts.iter().zip(&moved.pts) {
            let r = turn(*p, moved.rot);
            assert!((r[0] + moved.t[0] - q[0]).abs() < 1e-12 && (r[1] + moved.t[1] - q[1]).abs() < 1e-12);
        }
    }

    fn layout(offsets: Vec<Pt>, w: f64, h: f64) -> Layout {
        let m = Motif::new(vec![Shape::new(rect(0.0, 0.0, w, h))], "0°".into());
        Layout { count: offsets.len(), label: "0°".into(), pieces: 1, blocks: vec![Block { motif: m, offsets }] }
    }

    #[test]
    fn check_accepts_boxes_exactly_one_gap_apart() {
        assert!(check(&layout(vec![[0.0, 0.0], [16.0, 0.0], [0.0, 16.0]], 10.0, 10.0), 26.0, 26.0, 6.0).is_ok());
    }

    #[test]
    fn check_rejects_boxes_closer_than_the_gap() {
        let e = check(&layout(vec![[0.0, 0.0], [15.0, 0.0]], 10.0, 10.0), 100.0, 100.0, 6.0).unwrap_err();
        assert!(e.contains("closer"), "{e}");
    }

    #[test]
    fn check_rejects_a_box_inside_another() {
        let mut l = layout(vec![[0.0, 0.0]], 50.0, 50.0);
        let small = Motif::new(vec![Shape::new(rect(0.0, 0.0, 5.0, 5.0))], "0°".into());
        l.blocks.push(Block { motif: small, offsets: vec![[20.0, 20.0]] });
        l.count = 2;
        assert!(check(&l, 100.0, 100.0, 6.0).is_err());
    }

    #[test]
    fn check_rejects_boxes_outside_the_usable_area() {
        let e = check(&layout(vec![[0.0, 0.0], [95.0, 0.0]], 10.0, 10.0), 100.0, 100.0, 6.0).unwrap_err();
        assert!(e.contains("usable area"), "{e}");
    }

    #[test]
    fn rectangles_nest_like_a_grid_and_mix_rotations() {
        // 3 columns at 0° (90 mm) × 3 rows (60 mm) = 9, plus one 90° column (20 mm) × 2 rows = 11;
        // all at 90° gives 5 × 2 = 10, all at 0° gives 3 × 3 = 9
        let sheet = Sheet { w: 111.0, h: 61.0, margin: 0.0, gap: 0.0 };
        let l = nest(&[rect(0.0, 0.0, 30.0, 20.0)], &sheet).unwrap();
        assert_eq!(l.count, 11, "{}", l.label);
        assert_eq!(l.blocks.len(), 2);
        assert!(check(&l, 111.0, 61.0, 0.0).is_ok());
    }

    #[test]
    fn placements_reproduce_the_shapes() {
        let polys = vec![rect(0.0, 0.0, 30.0, 10.0), rect(0.0, 10.0, 10.0, 15.0)];
        let sheet = Sheet { w: 120.0, h: 90.0, margin: 5.0, gap: 3.0 };
        let l = nest(&polys, &sheet).unwrap();
        let base = outline(&polys);
        let pl = l.placements(sheet.margin);
        assert_eq!(pl.len(), l.count);
        for (i, s) in l.shapes() {
            let p = pl[i];
            let mut want = base[0].turned(p.rot).shifted([p.x - sheet.margin, p.y - sheet.margin]).bb;
            let got = s.bb;
            for k in 0..4 { want[k] -= got[k]; assert!(want[k].abs() < 1e-9, "box {i}: {want:?}"); }
        }
    }
}
