// Golden test for the pattern engine: every model at three sizes, compared against tests/golden/engine.json.
//   node tests/golden.mjs           compare, exit 1 on any difference
//   node tests/golden.mjs --update  rewrite the golden file after an intended change
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { loadEngine } from './engine.mjs';

const GOLDEN = new URL('./golden/engine.json', import.meta.url);
const SIZES = [
  { L: 40, W: 30, D: 15 },   // small: lip balm, sample
  { L: 90, W: 60, D: 30 },   // the studio default
  { L: 300, W: 220, D: 90 }, // large: hampers, shoe box
];
const MATS = { carton: { mat: 'iv310', T: 0.42, glue: 15 }, hardbox: { mat: 'gb20', T: 2, glue: 20 } };
const SHEET = { W: 650, H: 1000, margin: 10, gap: 6 };

const r2 = v => Math.round(v * 100) / 100;
const segKey = s => s.map(r2).join(',');
const polyKey = P => P.map(p => p.map(r2).join(' ')).join(';');

function snapshot(E) {
  const out = {};
  for (const key of [...E.ORDER_A, ...E.ORDER_B]) {
    if (key === 'import') continue;
    const M = E.MODELS[key], mat = M.hb ? MATS.hardbox : MATS.carton;
    if (!E.MAT[mat.mat]) throw new Error(`unknown material ${mat.mat}`);
    for (const size of SIZES) {
      const s = { model: key, ...size, Xs: {}, ...mat, TI: 15, HBM: 'v' };
      if (M.hex) s.W = s.L;
      s.X = M.defX ? M.defX(s) : 30;
      const m = E.buildModel(s);
      out[`${key} ${size.L}x${size.W}x${size.D}`] = m.parts.map(p => ({
        part: p.key,
        items: p.items.map(it => {
          const g = it.g;
          return {
            name: it.name, count: it.count, w: r2(g.w), h: r2(g.h),
            cutLen: r2(g.cutLen), foldLen: r2(g.foldLen), area: r2(g.area),
            perPlano: E.impose(g.w, g.h, SHEET.W, SHEET.H, SHEET.margin, SHEET.gap).n,
            cuts: g.cuts.map(segKey).sort(), folds: g.folds.map(segKey).sort(),
            polys: g.polys.map(p => polyKey(p.pts)), holes: g.holes.map(polyKey),
          };
        }),
      }));
    }
  }
  return out;
}

function diff(a, b, path = '', acc = []) {
  if (acc.length >= 20) return acc;
  if (typeof a !== typeof b || Array.isArray(a) !== Array.isArray(b)) acc.push(`${path}: type ${JSON.stringify(a)?.slice(0, 60)} vs ${JSON.stringify(b)?.slice(0, 60)}`);
  else if (a && typeof a === 'object') {
    for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) diff(a[k], b[k], path ? `${path}.${k}` : k, acc);
  } else if (a !== b) acc.push(`${path}: expected ${JSON.stringify(a)}, got ${JSON.stringify(b)}`);
  return acc;
}

const now = snapshot(loadEngine());
if (process.argv.includes('--update') || !existsSync(GOLDEN)) {
  writeFileSync(GOLDEN, JSON.stringify(now, null, 1) + '\n');
  console.log(`golden written: ${Object.keys(now).length} cases`);
} else {
  const want = JSON.parse(readFileSync(GOLDEN, 'utf8'));
  const d = diff(want, now);
  if (d.length) { console.error(`engine output changed (${d.length}${d.length >= 20 ? '+' : ''} differences):\n  ` + d.join('\n  ')); process.exit(1); }
  console.log(`golden ok: ${Object.keys(now).length} cases match`);
}
