// Writes nest/tests/fixtures.json: real patterns from the engine that `cargo test` nests, so the Rust tests cover
// the cases where a single lattice used to lose to impose() and the hardbox pieces (board, wrap, liner).
//   node tests/export-nest-fixtures.mjs
import { writeFileSync } from 'node:fs';
import { loadEngine } from './engine.mjs';

const SHEET = { w: 650, h: 1000, margin: 10, gap: 6 };
const E = loadEngine();
const build = (model, [L, W, D]) => {
  const M = E.MODELS[model];
  const s = { model, L, W: M.hex ? L : W, D, Xs: {}, mat: 'iv310', T: 0.42, glue: 15, TI: 15, HBM: 'v' };
  s.X = M.defX ? M.defX(s) : 30;
  return E.buildModel(s);
};
const job = (model, size, name, g) => ({
  model, name, size, sheet: SHEET, polys: g.polys.map(p => p.pts),
  bboxCount: E.impose(g.w, g.h, SHEET.w, SHEET.h, SHEET.margin, SHEET.gap).n,
});

const jobs = [];
for (const [model, size] of [['rte', [90, 60, 30]], ['tray', [150, 100, 50]], ['ste', [40, 30, 15]], ['tray', [40, 30, 15]]]) {
  jobs.push(job(model, size, `${model} ${size.join('x')}`, build(model, size).parts[0].items[0].g));
}
for (const [model, size] of [['hb_lid', [150, 100, 50]], ['hb_lid', [60, 60, 60]], ['hb_shoulder', [60, 60, 60]]]) {
  for (const p of build(model, size).parts) for (const it of p.items) {
    jobs.push(job(model, size, `${model} ${size.join('x')} ${p.key}: ${it.name}`, it.g));
  }
}
writeFileSync(new URL('../nest/tests/fixtures.json', import.meta.url), JSON.stringify(jobs));
console.log(jobs.map(j => `${j.name} (impose ${j.bboxCount})`).join('\n'));
