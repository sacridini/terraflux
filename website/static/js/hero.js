/* The home page's simulation: a small cellular automaton in the spirit of Dinamica EGO's Expander and Patcher.
   Forest next to farming is converted where a suitability is high (Expander); now and then a new patch opens
   away from the old ones (Patcher). Cells drawn as the logo's. */
(function () {
  "use strict";
  var cv = document.querySelector(".sim-canvas");
  if (!cv) return;
  var ctx = cv.getContext("2d");
  var COLS = 70, ROWS = 50, STEPS = 30;
  var stepEl = document.querySelector(".sim-step");
  var reduce = window.matchMedia && matchMedia("(prefers-reduced-motion: reduce)").matches;
  var FOREST = 1, FARM = 2, NEW = 3, WATER = 4;
  var COLORS = { 1: "#4c9f6a", 2: "#c9a35b", 3: "#e8901f", 4: "#3f7fae" };
  var grid, suit, step, timer, seed = 20260;

  function rng() { seed = (seed * 1664525 + 1013904223) >>> 0; return seed / 4294967296; }

  function field(blobs, scale) {
    var f = new Float32Array(COLS * ROWS), b = [];
    for (var i = 0; i < blobs; i++) b.push([rng() * COLS, rng() * ROWS, (0.4 + rng()) * scale, rng() * 2 - 0.6]);
    for (var y = 0; y < ROWS; y++) for (var x = 0; x < COLS; x++) {
      var v = 0;
      for (var k = 0; k < b.length; k++) { var dx = x - b[k][0], dy = y - b[k][1]; v += b[k][3] * Math.exp(-(dx * dx + dy * dy) / (2 * b[k][2] * b[k][2])); }
      f[y * COLS + x] = v;
    }
    var lo = Infinity, hi = -Infinity;
    for (i = 0; i < f.length; i++) { lo = Math.min(lo, f[i]); hi = Math.max(hi, f[i]); }
    for (i = 0; i < f.length; i++) f[i] = (f[i] - lo) / (hi - lo || 1);
    return f;
  }

  function init() {
    suit = field(14, 9);
    var start = field(5, 7);
    grid = new Uint8Array(COLS * ROWS);
    // a river that wanders across, widening into a lake
    var ry = ROWS * (0.3 + rng() * 0.4), drift = 0;
    for (var x = 0; x < COLS; x++) {
      drift += (rng() - 0.5) * 0.9; drift *= 0.92; ry += drift;
      ry = Math.max(4, Math.min(ROWS - 5, ry));
      var w = 1 + (x > COLS * 0.55 && x < COLS * 0.7 ? 3 : 0.6);
      for (var y = Math.floor(ry - w); y <= Math.ceil(ry + w); y++) if (y >= 0 && y < ROWS) grid[y * COLS + x] = WATER;
    }
    for (var i = 0; i < grid.length; i++) {
      if (grid[i]) continue;
      grid[i] = start[i] > 0.78 ? FARM : FOREST;
    }
    step = 0;
  }

  function neighbours(i) {
    var x = i % COLS, y = (i / COLS) | 0, n = 0;
    for (var dy = -1; dy <= 1; dy++) for (var dx = -1; dx <= 1; dx++) {
      if (!dx && !dy) continue;
      var xx = x + dx, yy = y + dy;
      if (xx < 0 || yy < 0 || xx >= COLS || yy >= ROWS) continue;
      var g = grid[yy * COLS + xx];
      if (g === FARM || g === NEW) n++;
    }
    return n;
  }

  function advance() {
    var cand = [], total = 0, forest = 0;
    for (var i = 0; i < grid.length; i++) {
      if (grid[i] !== FOREST) continue;
      forest++;
      var n = neighbours(i);
      if (!n) continue;
      var w = Math.pow(suit[i], 3) * Math.pow(n, 1.6);
      cand.push([i, w]); total += w;
    }
    var quota = Math.round(forest * 0.012) + 4;
    for (var q = 0; q < quota && cand.length; q++) {
      var r = rng() * total, k = 0;
      while (k < cand.length - 1 && r > cand[k][1]) { r -= cand[k][1]; k++; }
      grid[cand[k][0]] = NEW; total -= cand[k][1]; cand.splice(k, 1);
    }
    // a new patch now and then, where it suits (Patcher)
    if (rng() < 0.45) {
      for (var t = 0; t < 40; t++) {
        var c = (rng() * grid.length) | 0;
        if (grid[c] === FOREST && suit[c] > 0.7 && neighbours(c) === 0) {
          var cx = c % COLS, cy = (c / COLS) | 0;
          for (var dy = -1; dy <= 1; dy++) for (var dx = -1; dx <= 1; dx++) {
            var j = (cy + dy) * COLS + cx + dx;
            if (cy + dy >= 0 && cy + dy < ROWS && cx + dx >= 0 && cx + dx < COLS && grid[j] === FOREST && rng() < 0.75) grid[j] = NEW;
          }
          break;
        }
      }
    }
    step++;
  }

  function draw() {
    var W = cv.width, H = cv.height, cw = W / COLS, ch = H / ROWS, g = Math.max(1, cw * 0.12), rr = cw * 0.2;
    ctx.fillStyle = "#262827"; ctx.fillRect(0, 0, W, H);
    for (var i = 0; i < grid.length; i++) {
      var x = (i % COLS) * cw, y = ((i / COLS) | 0) * ch;
      ctx.fillStyle = COLORS[grid[i]];
      if (grid[i] === FOREST) ctx.globalAlpha = 0.72 + suit[i] * 0.28; else ctx.globalAlpha = 1;
      if (ctx.roundRect) { ctx.beginPath(); ctx.roundRect(x + g / 2, y + g / 2, cw - g, ch - g, rr); ctx.fill(); }
      else ctx.fillRect(x + g / 2, y + g / 2, cw - g, ch - g);
    }
    ctx.globalAlpha = 1;
    if (stepEl) stepEl.textContent = "step " + step + " of " + STEPS;
  }

  function sizeCanvas() {
    var r = cv.getBoundingClientRect(), dpr = Math.min(window.devicePixelRatio || 1, 2);
    var w = Math.round(r.width * dpr);
    if (w && Math.abs(cv.width - w) > 2) { cv.width = w; cv.height = Math.round(w * 5 / 7); }
  }

  function run() {
    clearInterval(timer);
    init(); sizeCanvas();
    if (reduce) { while (step < STEPS) advance(); draw(); return; }
    draw();
    timer = setInterval(function () {
      if (step >= STEPS) { clearInterval(timer); return; }
      advance(); draw();
    }, 260);
  }

  var started = false;
  if ("IntersectionObserver" in window) {
    new IntersectionObserver(function (es) { if (es[0].isIntersecting && !started) { started = true; run(); } }).observe(cv);
  } else run();
  window.addEventListener("resize", function () { sizeCanvas(); if (grid) draw(); });
  var replay = document.querySelector(".sim-replay");
  if (replay) replay.addEventListener("click", function () { seed = (Date.now() & 0xffffff) >>> 0; run(); });
})();
