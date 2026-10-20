// Runs the Rust nester (nest/, compiled to lipatin_nest_bg.wasm) off the main thread for the plano tab.
import init, { nest_sheet } from './lipatin_nest.js';

const ready = init({ module_or_path: new URL('./lipatin_nest_bg.wasm', import.meta.url) });
ready.catch(() => {}); // reported on the first message instead

self.onmessage = async ({ data }) => {
  try {
    await ready;
  } catch (e) {
    // the module did not load: tell the page to stop asking and keep the bounding-box count
    return self.postMessage({ error: String((e && e.message) || e), fatal: true });
  }
  try {
    self.postMessage(JSON.parse(nest_sheet(data.polys, data.w, data.h, data.margin, data.gap)));
  } catch (e) {
    self.postMessage({ error: String((e && e.message) || e) });
  }
};
