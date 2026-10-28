// The calculators' playgrounds' engine (static/js/calc-engine.js) run outside a browser, on the playgrounds' data
// (static/data/calculators/*.js): answers the requests of a JSON file, as calc.js asks them, on stdout.
//
//     node website/tools/check_calc.mjs requests.json > answers.json
//
// Each request is calc.js's ({fn, text, ...}) with `data: "raster"` or `data: "vector"`: the inputs (and, for
// features, the features) of that playground added, and `inputs` the node's other inputs (numbers, texts).
// figures/calculators/playground.py uses it to check the engine against `terraflux run`.

import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sandbox = { window: {} };
vm.createContext(sandbox);
for (const f of ["js/calc-engine.js", "data/calculators/raster.js", "data/calculators/vector.js"]) {
  vm.runInContext(fs.readFileSync(path.join(web, "static", f), "utf8"), sandbox);
}
const w = sandbox.window;
const bytes = Buffer.from(w.TFCalcEngine.wasm, "base64");
const { instance } = await WebAssembly.instantiate(bytes, {});
const e = instance.exports;
const enc = new TextEncoder(), dec = new TextDecoder();

function call(req) {
  const b = enc.encode(JSON.stringify(req));
  const p = e.tf_alloc(b.length);
  new Uint8Array(e.memory.buffer, p, b.length).set(b);
  const n = e.tf_call(p, b.length);
  return JSON.parse(dec.decode(new Uint8Array(e.memory.buffer, e.tf_answer(), n)));
}

const requests = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const answers = requests.map((r) => {
  const q = { ...r };
  const data = w.TFCalcData[r.data];
  delete q.data;
  if (r.data === "raster") {
    q.width = data.width;
    q.height = data.height;
    q.inputs = { ...data.inputs };
    for (const [k, v] of Object.entries(r.inputs || {})) q.inputs[k] = typeof v === "string" ? { text: v } : { number: v };
  } else {
    q.features = data.features;
    q.inputs = r.inputs || {};
  }
  return call(q);
});
process.stdout.write(JSON.stringify(answers));
