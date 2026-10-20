// Writes the panels of each carton model as JSON for the Rust nesting prototype (nest/).
//   node tests/export-outline.mjs 90 60 30 > nest/jobs.json
// Each job carries the bounding-box count from impose(), so the prototype can compare against today's plano.
import { loadEngine } from './engine.mjs';

const [L = 90, W = 60, D = 30] = process.argv.slice(2).map(Number);
const SHEET = { w: 650, h: 1000, margin: 10, gap: 6 };
const E = loadEngine();

const jobs = E.ORDER_A.filter(k => k !== 'import').map(model => {
  const M = E.MODELS[model];
  const s = { model, L, W: M.hex ? L : W, D, Xs: {}, mat: 'iv310', T: 0.42, glue: 15, TI: 15, HBM: 'v' };
  s.X = M.defX ? M.defX(s) : 30;
  const g = E.buildModel(s).parts[0].items[0].g;
  return {
    model, name: M.name, size: [L, s.W, D], w: g.w, h: g.h, sheet: SHEET,
    bboxCount: E.impose(g.w, g.h, SHEET.w, SHEET.h, SHEET.margin, SHEET.gap).n,
    polys: g.polys.map(p => p.pts),
  };
});
process.stdout.write(JSON.stringify(jobs));
