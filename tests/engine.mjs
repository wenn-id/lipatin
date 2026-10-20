// Loads Lipatin's pattern engine straight out of index.html into a sandbox, without a browser.
// The engine is the code between the "Materials" and "WebGL folding preview" markers, plus impose().
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const between = (src, start, end) => {
  const a = src.indexOf(start), b = src.indexOf(end, a);
  if (a < 0 || b < 0) throw new Error(`marker not found: ${a < 0 ? start : end}`);
  return src.slice(a, b);
};
const line = (src, head) => {
  const found = src.split('\n').find(l => l.startsWith(head));
  if (!found) throw new Error(`line not found: ${head}`);
  return found;
};

export function loadEngine(htmlPath = new URL('../index.html', import.meta.url)) {
  const html = readFileSync(htmlPath, 'utf8');
  const code = [
    line(html, 'const clamp='), line(html, 'const nf='), line(html, 'const f2='), line(html, 'const esc='),
    between(html, '/* ---------- Materials', '/* ---------- WebGL folding preview'),
    between(html, '/* ---------- Imposition on a plano sheet', '/* ---------- UI ---------- */'),
    ';({MODELS,MAT,ORDER_A,ORDER_B,buildModel,impose});',
  ].join('\n');
  return vm.runInNewContext(code, { window: {}, console }, { filename: 'lipatin-engine.js' });
}
