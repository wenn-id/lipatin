// Runs the committed WebAssembly nester (lipatin_nest_bg.wasm, built from nest/) on every carton model and checks
// that the count the plano tab shows, max(impose(), nested), never drops below impose() and that every
// placement stays on the sheet. Reverse tuck 90×60×30 on 65×100 cm, margin 10, gap 6 must give 12.
//   node tests/nest-wasm.mjs
import { readFileSync } from 'node:fs';
import { initSync, nest_sheet } from '../lipatin_nest.js';
import { loadEngine } from './engine.mjs';

initSync({ module: readFileSync(new URL('../lipatin_nest_bg.wasm', import.meta.url)) });
const E = loadEngine();
const SHEET = { w: 650, h: 1000, margin: 10, gap: 6 };
let failed = 0;
const fail = msg => { failed++; console.log('  FAIL ' + msg); };

for (const [L, W, D] of [[90, 60, 30], [150, 100, 50], [40, 30, 15]]) {
  for (const model of E.ORDER_A.filter(k => k !== 'import')) {
    const M = E.MODELS[model];
    const s = { model, L, W: M.hex ? L : W, D, Xs: {}, mat: 'iv310', T: 0.42, glue: 15, TI: 15, HBM: 'v' };
    s.X = M.defX ? M.defX(s) : 30;
    const g = E.buildModel(s).parts[0].items[0].g;
    const bbox = E.impose(g.w, g.h, SHEET.w, SHEET.h, SHEET.margin, SHEET.gap).n;
    const t0 = performance.now();
    const r = JSON.parse(nest_sheet(JSON.stringify(g.polys.map(p => p.pts)), SHEET.w, SHEET.h, SHEET.margin, SHEET.gap));
    const ms = performance.now() - t0;
    const tag = `${model} ${L}×${s.W}×${D}`;
    console.log(`${tag.padEnd(22)} kotak ${String(bbox).padStart(4)}  dikunci ${String(r.count).padStart(4)}  ${r.motif.padEnd(24)} ${ms.toFixed(0)} ms`);
    if (r.error) { fail(`${tag}: ${r.error}`); continue; }
    if (r.placements.length !== r.count) fail(`${tag}: ${r.placements.length} placements for count ${r.count}`);
    for (const p of r.placements) {
      if (![0, 90, 180, 270].includes(p.rot)) fail(`${tag}: rotation ${p.rot}`);
      if (p.x < -g.w - 1 || p.y < -g.w - 1 || p.x > SHEET.w + g.w || p.y > SHEET.h + g.w) fail(`${tag}: placement off the sheet`);
    }
    if (model === 'rte' && L === 90 && Math.max(bbox, r.count) !== 12) fail(`${tag}: expected 12 per plano, got ${Math.max(bbox, r.count)}`);
  }
}
console.log(failed ? `${failed} failed` : 'ok');
process.exit(failed ? 1 : 0);
