//! Compares the nested count from the library with today's bounding-box count from `impose()`, for jobs written
//! by `tests/export-outline.mjs`, and draws each layout as SVG.
//!
//!   cargo run --release -- jobs.json [out-dir]

use lipatin_nest::{Layout, Pt, Sheet, check, nest};
use serde::Deserialize;
use std::{fmt::Write as _, fs, path::PathBuf, time::Instant};

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

fn svg(job: &Job, l: &Layout) -> String {
    let (sw, sh, m) = (job.sheet.w, job.sheet.h, job.sheet.margin);
    let mut s = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-10 -10 {} {}" width="{}mm" height="{}mm"><rect width="{sw}" height="{sh}" fill="#fff" stroke="#17251D" stroke-width="1.5"/><rect x="{m}" y="{m}" width="{}" height="{}" fill="none" stroke="#999" stroke-dasharray="8 6"/>"##,
        sw + 20.0, sh + 20.0, (sw + 20.0) / 4.0, (sh + 20.0) / 4.0, sw - 2.0 * m, sh - 2.0 * m);
    for (_, p) in l.shapes() {
        let d: String = p.pts.iter().enumerate().map(|(i, q)| format!("{}{:.2} {:.2}", if i == 0 { 'M' } else { 'L' }, q[0] + m, q[1] + m)).collect();
        write!(s, r##"<path d="{d}Z" fill="#F3EBDD" stroke="#C9105A" stroke-width="1"/>"##).unwrap();
    }
    s + "</svg>"
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().unwrap_or_else(|| "jobs.json".into());
    let out = PathBuf::from(args.next().unwrap_or_else(|| "out".into()));
    fs::create_dir_all(&out).unwrap();
    let jobs: Vec<Job> = serde_json::from_str(&fs::read_to_string(&input).expect("read jobs")).expect("parse jobs");

    println!("{:<18} {:>14} {:>9} {:>8} {:>7}  {:<22} {:>6} {}", "model", "size (mm)", "bbox now", "nested", "gain", "motif", "pieces", "time");
    for job in &jobs {
        let t0 = Instant::now();
        let size = format!("{}×{}×{}", job.size[0], job.size[1], job.size[2]);
        match nest(&job.polys, &job.sheet) {
            Some(l) => {
                let (uw, uh) = job.sheet.usable();
                let verdict = check(&l, uw, uh, job.sheet.gap);
                let gain = if job.bbox_count > 0 { format!("{:+.0}%", (l.count as f64 / job.bbox_count as f64 - 1.0) * 100.0) } else { "n/a".into() };
                println!("{:<18} {:>14} {:>9} {:>8} {:>7}  {:<22} {:>6} {:.1}s{}", job.name, size, job.bbox_count, l.count, gain, l.label, l.pieces, t0.elapsed().as_secs_f64(),
                    verdict.err().map(|e| format!("  INVALID: {e}")).unwrap_or_default());
                fs::write(out.join(format!("{}-{}x{}x{}.svg", job.model, job.size[0], job.size[1], job.size[2])), svg(job, &l)).unwrap();
            }
            None => println!("{:<18} {:>14} {:>9} {:>8}", job.name, size, job.bbox_count, 0),
        }
    }
}
