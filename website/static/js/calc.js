/* The calculators' playgrounds (```tfcalc blocks): a script of map algebra over Sibuyan's maps (raster.calc) or
   its patches (vector.calc, vector.filter), computed as it is typed by Terraflux's own engine built to WebAssembly
   (calc-engine.js, from crates/calc-web): its words coloured, its first error where it is, the names completed,
   every output drawn (the values under the pointer told), a console, and the features' table. */
(function () {
  "use strict";

  var ROOT = window.TF_ROOT || "./";
  var loading = {};

  function script(src) {
    if (!loading[src]) {
      loading[src] = new Promise(function (ok, fail) {
        var s = document.createElement("script");
        s.src = src;
        s.onload = ok;
        s.onerror = function () { fail(new Error("could not load " + src)); };
        document.head.appendChild(s);
      });
    }
    return loading[src];
  }

  // ------------------------------------------------------------------------------------------- the engine

  var engine = null;

  /* The engine's infinite values ("inf", "-inf": 1 / 0, log(0)) as numbers, in its maps, layers, attributes of
     numbers and numbers (not in texts). */
  function infinite(v) {
    var num = function (x) { return x === "inf" ? Infinity : x === "-inf" ? -Infinity : x; };
    (function walk(o) {
      if (!o || typeof o !== "object") return;
      if (Array.isArray(o)) { o.forEach(walk); return; }
      ["map", "cells", "numbers"].forEach(function (k) { if (Array.isArray(o[k])) o[k] = o[k].map(num); });
      if (o.number !== undefined) o.number = num(o.number);
      Object.keys(o).forEach(function (k) { if (k !== "texts") walk(o[k]); });
    })(v);
    return v;
  }

  function startEngine(src) {
    if (!engine) {
      engine = script(src).then(function () {
        var b64 = window.TFCalcEngine.wasm;
        var bin = atob(b64);
        var bytes = new Uint8Array(bin.length);
        for (var i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        return WebAssembly.instantiate(bytes, {});
      }).then(function (r) {
        var e = r.instance.exports;
        var enc = new TextEncoder(), dec = new TextDecoder();
        return function call(req) {
          var b = enc.encode(typeof req === "string" ? req : JSON.stringify(req));
          var p = e.tf_alloc(b.length);
          new Uint8Array(e.memory.buffer, p, b.length).set(b);
          var n = e.tf_call(p, b.length);
          var a = e.tf_answer();
          return infinite(JSON.parse(dec.decode(new Uint8Array(e.memory.buffer, a, n))));
        };
      });
    }
    return engine;
  }

  // ------------------------------------------------------------------------------------------- helpers

  function h(tag, cls, text) {
    var el = document.createElement(tag);
    if (cls) el.className = cls;
    if (text != null) el.textContent = text;
    return el;
  }

  function css(v) { return getComputedStyle(document.documentElement).getPropertyValue(v).trim(); }

  function fmt(x) {
    if (x === null || x === undefined || (typeof x === "number" && isNaN(x))) return "None";
    if (typeof x !== "number") return String(x);
    if (!isFinite(x)) return x > 0 ? "inf" : "-inf";
    if (x === 0) return "0";
    var a = Math.abs(x);
    if (Number.isInteger(x) && a < 1e9) return x.toLocaleString("en-US").replace(/,/g, " ");
    if (a >= 1e6 || a < 1e-3) return x.toExponential(3).replace("e+", "e");
    var d = a >= 1000 ? 1 : a >= 10 ? 2 : a >= 1 ? 3 : 4;
    return x.toFixed(d).replace(/\.?0+$/, "");
  }

  function esc(s) { return String(s).replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); }

  // viridis, sampled
  var VIRIDIS = ["#440154", "#482475", "#414487", "#355f8d", "#2a788e", "#21918c", "#22a884", "#44bf70", "#7ad151", "#bddf26", "#fde725"];
  var CLASSES = ["#0072b2", "#e69f00", "#009e73", "#cc79a7", "#56b4e9", "#d55e00", "#f0e442", "#785ef0", "#8c564b", "#7f7f7f", "#bcbd22", "#17becf"];

  function rgb(hex) { return [parseInt(hex.slice(1, 3), 16), parseInt(hex.slice(3, 5), 16), parseInt(hex.slice(5, 7), 16)]; }
  var VIR = VIRIDIS.map(rgb);

  function ramp(t) {
    t = Math.max(0, Math.min(1, t)) * (VIR.length - 1);
    var i = Math.min(VIR.length - 2, Math.floor(t)), f = t - i;
    return [0, 1, 2].map(function (k) { return Math.round(VIR[i][k] + (VIR[i + 1][k] - VIR[i][k]) * f); });
  }

  function summary(cells) {
    var lo = Infinity, hi = -Infinity, sum = 0, n = 0, inf = 0, missing = 0, distinct = new Map(), whole = true;
    for (var i = 0; i < cells.length; i++) {
      var x = cells[i];
      if (x === null || isNaN(x)) { missing++; continue; }
      n++;
      // (an infinite value: counted, out of the range drawn)
      if (!isFinite(x)) { whole = false; inf++; continue; }
      if (x < lo) lo = x;
      if (x > hi) hi = x;
      sum += x;
      if (whole && !Number.isInteger(x)) whole = false;
      if (distinct.size <= 12) distinct.set(x, (distinct.get(x) || 0) + 1);
    }
    return { lo: lo, hi: hi, mean: n > inf ? sum / (n - inf) : NaN, n: n, inf: inf, missing: missing, classes: whole && distinct.size <= 12 ? distinct : null };
  }

  /* How the values of a map or an attribute are coloured: classes (a few whole numbers: 0 and 1, the land use's
     classes) or a ramp from the least to the largest. */
  function scheme(s, known) {
    if (s.classes && s.classes.size) {
      var keys = Array.from(s.classes.keys()).sort(function (a, b) { return a - b; });
      var colors = {}, labels = {};
      var land = known && keys.every(function (k) { return known[String(k)]; }) && keys.length > 2;
      keys.forEach(function (k, i) {
        if (land) {
          colors[k] = known[String(k)][1];
          labels[k] = k + " " + known[String(k)][0];
        } else if (keys.length <= 2 && keys.every(function (k) { return k === 0 || k === 1; })) {
          colors[k] = k === 1 ? css("--ember") || "#e8901f" : css("--rule-2") || "#c5cec6";
          labels[k] = k === 1 ? "1 (True)" : "0 (False)";
        } else {
          colors[k] = CLASSES[i % CLASSES.length];
          labels[k] = fmt(k);
        }
      });
      return { kind: "classes", keys: keys, colors: colors, labels: labels, counts: s.classes };
    }
    return { kind: "ramp", lo: s.lo, hi: s.hi };
  }

  function colorOf(sc, x) {
    if (x === null || isNaN(x)) return null;
    if (sc.kind === "classes") return rgb(sc.colors[x] || "#888888");
    return ramp(sc.hi > sc.lo ? (x - sc.lo) / (sc.hi - sc.lo) : 0.5);
  }

  function legend(sc, total) {
    var el = h("div", "cx-legend");
    if (sc.kind === "classes") {
      sc.keys.forEach(function (k) {
        var it = h("span", "cx-key");
        var sw = h("i");
        sw.style.background = sc.colors[k];
        it.appendChild(sw);
        var n = sc.counts.get(k) || 0;
        it.appendChild(document.createTextNode(sc.labels[k] + (total ? " · " + (100 * n / total).toFixed(1) + " %" : "")));
        el.appendChild(it);
      });
    } else if (isFinite(sc.lo)) {
      var bar = h("span", "cx-ramp");
      bar.style.background = "linear-gradient(90deg," + VIRIDIS.join(",") + ")";
      el.appendChild(h("span", "cx-lo", fmt(sc.lo)));
      el.appendChild(bar);
      el.appendChild(h("span", "cx-hi", fmt(sc.hi)));
    }
    return el;
  }

  /* A script's line of an error: "line 3: ..." (the engine's messages). */
  function lineOf(msg) {
    var m = /(?:^|: )line (\d+): /.exec(msg);
    return m ? +m[1] : 0;
  }

  // ------------------------------------------------------------------------------------------- the editor

  function Editor(host, onChange, engineCall) {
    var self = this;
    this.wrap = h("div", "cx-editor");
    this.gutter = h("div", "cx-gutter");
    this.area = h("div", "cx-area");
    this.pre = h("pre", "cx-pre");
    this.pre.setAttribute("aria-hidden", "true");
    this.ta = h("textarea", "cx-ta");
    this.ta.spellcheck = false;
    this.ta.setAttribute("autocapitalize", "off");
    this.ta.setAttribute("autocomplete", "off");
    this.ta.setAttribute("aria-label", "The script");
    this.pop = h("ul", "cx-pop");
    this.pop.hidden = true;
    this.pop.setAttribute("role", "listbox");
    this.area.appendChild(this.pre);
    this.area.appendChild(this.ta);
    this.area.appendChild(this.pop);
    this.wrap.appendChild(this.gutter);
    this.wrap.appendChild(this.area);
    host.appendChild(this.wrap);
    this.parts = [];
    this.error = null;
    this.items = [];
    this.sel = 0;
    this.range = null;
    this.complete = null;

    this.ta.addEventListener("input", function (e) {
      self.paint();
      onChange();
      var t = e.data || "";
      if (/^[A-Za-z_.0-9]$/.test(t) && self.complete) self.suggest(false);
      else self.close();
    });
    this.ta.addEventListener("scroll", function () { self.sync(); });
    this.ta.addEventListener("keydown", function (e) {
      if (!self.pop.hidden) {
        if (e.key === "ArrowDown" || e.key === "ArrowUp") {
          e.preventDefault();
          self.sel = (self.sel + (e.key === "ArrowDown" ? 1 : self.items.length - 1)) % self.items.length;
          self.drawPop();
          return;
        }
        if (e.key === "Enter" || e.key === "Tab") { e.preventDefault(); self.accept(self.sel); return; }
        if (e.key === "Escape") { e.preventDefault(); self.close(); return; }
      }
      if (e.key === " " && (e.ctrlKey || e.metaKey) && self.complete) { e.preventDefault(); self.suggest(true); }
    });
    this.ta.addEventListener("blur", function () { setTimeout(function () { self.close(); }, 150); });
    this.ta.addEventListener("click", function () { self.close(); });
  }

  Editor.prototype.value = function () { return this.ta.value; };
  Editor.prototype.set = function (t) { this.ta.value = t; this.parts = []; this.error = null; this.paint(); };

  Editor.prototype.sync = function () {
    this.pre.scrollTop = this.ta.scrollTop;
    this.pre.scrollLeft = this.ta.scrollLeft;
    this.gutter.scrollTop = this.ta.scrollTop;
  };

  /* The script drawn under the text box: its parts coloured, its error's range underlined. */
  Editor.prototype.paint = function () {
    var t = this.ta.value;
    var parts = this.parts;
    var err = this.error;
    var out = "";
    var cls = new Array(t.length);
    parts.forEach(function (p) { for (var i = p[0]; i < p[1] && i < t.length; i++) cls[i] = "p-" + p[2]; });
    var bad = new Array(t.length);
    if (err && err.start != null) {
      var a = err.start, b = Math.max(err.end, err.start + 1);
      for (var i = a; i < b && i < t.length; i++) bad[i] = true;
    }
    var cur = null, curBad = false, buf = "";
    function flush() {
      if (!buf) return;
      var c = (cur || "") + (curBad ? " p-err" : "");
      out += c ? '<span class="' + c.trim() + '">' + esc(buf) + "</span>" : esc(buf);
      buf = "";
    }
    for (var k = 0; k < t.length; k++) {
      var c = cls[k] || null, bb = !!bad[k];
      if (t[k] === "\n") { flush(); cur = null; curBad = false; out += "\n"; continue; }
      if (c !== cur || bb !== curBad) { flush(); cur = c; curBad = bb; }
      buf += t[k];
    }
    flush();
    // an error at the end of the text: a mark after it
    if (err && err.start != null && err.start >= t.length) out += '<span class="p-err p-end"> </span>';
    this.pre.innerHTML = out + "\n";
    var lines = t.split("\n").length;
    var g = "";
    var errLine = err ? err.line : 0;
    for (var l = 1; l <= lines; l++) g += '<span' + (l === errLine ? ' class="is-err"' : "") + ">" + l + "</span>";
    this.gutter.innerHTML = g;
    // as tall as its lines (between 6 and 18 of them)
    var lh = parseFloat(getComputedStyle(this.ta).lineHeight) || 21;
    var px = Math.round(Math.max(6, Math.min(18, lines + 1)) * lh + 20) + "px";
    if (this.area.style.height !== px) { this.area.style.height = px; this.gutter.style.height = px; }
    this.sync();
  };

  /* Where the cursor is drawn, in the text box's pixels. */
  Editor.prototype.caret = function (pos) {
    var t = this.ta.value.slice(0, pos);
    var line = t.split("\n").length - 1;
    var col = t.length - t.lastIndexOf("\n") - 1;
    var cs = getComputedStyle(this.ta);
    var lh = parseFloat(cs.lineHeight) || 20;
    if (!this.cw) {
      var probe = h("span", null, "0000000000");
      probe.style.font = cs.font;
      probe.style.position = "absolute";
      probe.style.visibility = "hidden";
      document.body.appendChild(probe);
      this.cw = probe.getBoundingClientRect().width / 10;
      probe.remove();
    }
    return { x: parseFloat(cs.paddingLeft) + col * this.cw - this.ta.scrollLeft, y: parseFloat(cs.paddingTop) + (line + 1) * lh - this.ta.scrollTop };
  };

  Editor.prototype.suggest = function (asked) {
    var pos = this.ta.selectionStart;
    var r = this.complete(this.ta.value, pos);
    if (!r || !r.items.length) { this.close(); return; }
    var word = this.ta.value.slice(r.start, r.end);
    var items = r.items;
    // only the one typed whole: nothing to complete
    if (!asked && items.length === 1 && items[0].text === word) { this.close(); return; }
    if (!asked && !word && this.ta.value[pos - 1] !== ".") { this.close(); return; }
    this.items = items.slice(0, 40);
    this.range = [r.start, r.end];
    this.sel = 0;
    this.drawPop();
    var c = this.caret(r.start);
    this.pop.style.left = Math.max(0, c.x) + "px";
    this.pop.style.top = c.y + 2 + "px";
    this.pop.hidden = false;
  };

  Editor.prototype.drawPop = function () {
    var self = this;
    this.pop.innerHTML = "";
    this.items.forEach(function (it, i) {
      var li = h("li", "p-" + it.part + (i === self.sel ? " is-on" : ""));
      li.setAttribute("role", "option");
      li.appendChild(h("b", null, it.label));
      if (it.detail) li.appendChild(h("span", null, it.detail));
      li.addEventListener("mousedown", function (e) { e.preventDefault(); self.accept(i); });
      self.pop.appendChild(li);
    });
    var on = this.pop.children[this.sel];
    if (on && on.scrollIntoView) on.scrollIntoView({ block: "nearest" });
  };

  Editor.prototype.accept = function (i) {
    var it = this.items[i];
    if (!it || !this.range) return this.close();
    var t = this.ta.value;
    this.ta.value = t.slice(0, this.range[0]) + it.text + t.slice(this.range[1]);
    var p = this.range[0] + it.text.length;
    // a function's parenthesis: the cursor inside
    if (/\(\)$/.test(it.text)) p -= 1;
    this.ta.setSelectionRange(p, p);
    this.close();
    this.ta.dispatchEvent(new Event("input"));
    this.ta.focus();
  };

  Editor.prototype.close = function () { this.pop.hidden = true; this.items = []; };

  // ------------------------------------------------------------------------------------------- a raster's card

  function mapCard(name, cells, w, h_, opts) {
    var card = h("figure", "cx-card");
    var head = h("figcaption", "cx-card-head");
    head.appendChild(h("code", "cx-name", name));
    head.appendChild(h("span", "cx-kind", opts.kind || "a map"));
    card.appendChild(head);
    var s = summary(cells);
    var sc = scheme(s, opts.classes);
    var view = h("div", "cx-map");
    var cv = h("canvas");
    cv.width = w;
    cv.height = h_;
    var ctx = cv.getContext("2d");
    var img = ctx.createImageData(w, h_);
    for (var i = 0; i < cells.length; i++) {
      var c = colorOf(sc, cells[i]);
      if (c) { img.data[4 * i] = c[0]; img.data[4 * i + 1] = c[1]; img.data[4 * i + 2] = c[2]; img.data[4 * i + 3] = 255; }
    }
    ctx.putImageData(img, 0, 0);
    view.appendChild(cv);
    var tip = h("div", "cx-tip");
    tip.hidden = true;
    view.appendChild(tip);
    card.appendChild(view);
    cv.addEventListener("mousemove", function (e) {
      var r = cv.getBoundingClientRect();
      var x = Math.floor((e.clientX - r.left) / r.width * w), y = Math.floor((e.clientY - r.top) / r.height * h_);
      if (x < 0 || y < 0 || x >= w || y >= h_) { tip.hidden = true; return; }
      var i = y * w + x;
      tip.innerHTML = "<b>" + esc(name) + " = " + esc(fmt(cells[i])) + "</b>" + (opts.at ? opts.at(i) : "") +
        '<span class="cx-rc">row ' + (y + 1) + ", column " + (x + 1) + "</span>";
      tip.hidden = false;
      var left = e.clientX - r.left + 14, top = e.clientY - r.top + 14;
      if (left + 220 > r.width) left = Math.max(0, e.clientX - r.left - 230);
      tip.style.left = left + "px";
      tip.style.top = top + "px";
      if (opts.hover) opts.hover(i);
    });
    cv.addEventListener("mouseleave", function () { tip.hidden = true; if (opts.hover) opts.hover(-1); });
    card.appendChild(legend(sc, s.n));
    var st = h("p", "cx-stats");
    st.innerHTML = s.n ? "least <b>" + fmt(s.lo) + "</b> · mean <b>" + fmt(s.mean) + "</b> · largest <b>" + fmt(s.hi) +
      "</b>" + (s.missing ? " · " + fmt(s.missing) + " cells missing" : "") : "every cell missing";
    card.appendChild(st);
    return card;
  }

  function numberCard(name, x, what) {
    var card = h("figure", "cx-card cx-number");
    var head = h("figcaption", "cx-card-head");
    head.appendChild(h("code", "cx-name", name));
    head.appendChild(h("span", "cx-kind", what || "a number"));
    card.appendChild(head);
    card.appendChild(h("p", "cx-big", fmt(x)));
    return card;
  }

  /* A stack: its layers side by side (a few), or one at a time. */
  function stackCard(name, layers, w, h_, opts) {
    var card = h("div", "cx-stack");
    var head = h("div", "cx-stack-head");
    head.appendChild(h("code", "cx-name", name));
    head.appendChild(h("span", "cx-kind", "a stack of " + layers.length + " layers"));
    card.appendChild(head);
    var grid = h("div", "cx-stack-grid");
    layers.forEach(function (l) {
      grid.appendChild(mapCard(name + "." + layerName(l.key), l.cells, w, h_, opts));
    });
    card.appendChild(grid);
    return card;
  }

  /* A layer's name as a script writes it (a name that is not one, `1997`, after an underscore). */
  function layerName(k) {
    var n = String(k).replace(/[^A-Za-z0-9_]/g, "_");
    return /^[A-Za-z_]/.test(n) ? n : "_" + n;
  }

  // ------------------------------------------------------------------------------------------- the features' map

  function rings(g) {
    if (g.type === "Polygon") return g.coordinates;
    if (g.type === "MultiPolygon") return [].concat.apply([], g.coordinates);
    return [];
  }

  function featureMap(data, onHover) {
    var b = data.bounds;
    var W = 1000, H = Math.round(1000 * (b[3] - b[1]) / (b[2] - b[0]));
    var NS = "http://www.w3.org/2000/svg";
    var svg = document.createElementNS(NS, "svg");
    svg.setAttribute("viewBox", "0 0 " + W + " " + H);
    svg.setAttribute("class", "cx-svg");
    svg.setAttribute("role", "img");
    var k = W / (b[2] - b[0]);
    var paths = data.features.geometries.map(function (g, i) {
      var d = rings(g).map(function (r) {
        return "M" + r.map(function (p) { return ((p[0] - b[0]) * k).toFixed(1) + " " + ((b[3] - p[1]) * k).toFixed(1); }).join("L") + "Z";
      }).join("");
      var p = document.createElementNS(NS, "path");
      p.setAttribute("d", d);
      p.setAttribute("fill-rule", "evenodd");
      p.dataset.i = i;
      svg.appendChild(p);
      return p;
    });
    svg.addEventListener("mousemove", function (e) {
      var t = e.target;
      onHover(t && t.dataset && t.dataset.i != null ? +t.dataset.i : -1, e);
    });
    svg.addEventListener("mouseleave", function () { onHover(-1); });
    return { svg: svg, paths: paths };
  }

  // ------------------------------------------------------------------------------------------- a playground

  function Playground(el) {
    var self = this;
    this.el = el;
    this.spec = JSON.parse(el.getAttribute("data-spec"));
    this.kind = this.spec.kind;
    this.names = false;
    el.innerHTML = "";
    el.classList.add("is-loading");
    var bar = h("div", "cx-bar");
    var pl = h("label", "cx-field");
    pl.appendChild(h("span", null, "Example"));
    this.presetSel = h("select");
    this.spec.presets.forEach(function (p, i) {
      var o = h("option", null, p.name);
      o.value = i;
      self.presetSel.appendChild(o);
    });
    pl.appendChild(this.presetSel);
    bar.appendChild(pl);
    if (this.kind === "vector") {
      var ops = h("div", "cx-seg");
      ops.setAttribute("role", "group");
      ops.setAttribute("aria-label", "The operator");
      this.opBtns = {};
      ["calc", "filter"].forEach(function (o) {
        var b = h("button", null, "vector." + o);
        b.type = "button";
        b.addEventListener("click", function () { self.setOp(o); self.schedule(0); });
        ops.appendChild(b);
        self.opBtns[o] = b;
      });
      bar.appendChild(ops);
      var cl = h("label", "cx-field cx-column");
      cl.appendChild(h("span", null, "column"));
      this.column = h("input");
      this.column.placeholder = "(a script with out: empty)";
      this.column.addEventListener("input", function () { self.schedule(150); });
      cl.appendChild(this.column);
      bar.appendChild(cl);
      this.columnField = cl;
    }
    this.inputsBox = h("div", "cx-inputs");
    bar.appendChild(this.inputsBox);
    var ev = h("label", "cx-check");
    this.everyBox = h("input");
    this.everyBox.type = "checkbox";
    this.everyBox.addEventListener("change", function () { self.names = self.everyBox.checked; self.schedule(0); });
    ev.appendChild(this.everyBox);
    ev.appendChild(document.createTextNode(" every name"));
    ev.title = "Show what every name of the script holds, not only its outputs";
    bar.appendChild(ev);
    el.appendChild(bar);

    var body = h("div", "cx-body");
    var left = h("div", "cx-left");
    this.editor = new Editor(left, function () { self.schedule(220); });
    this.status = h("p", "cx-status");
    this.status.setAttribute("aria-live", "polite");
    left.appendChild(this.status);
    var con = h("form", "cx-console");
    con.appendChild(h("span", "cx-prompt", ">"));
    this.conIn = h("input");
    this.conIn.placeholder = this.kind === "vector" ? "a line in the script's scope: geometry.area.sum(), cover.count()" : "a line in the script's scope: lu._2011.count(), d.elevation.max()";
    this.conIn.setAttribute("aria-label", "Console");
    this.conIn.spellcheck = false;
    con.appendChild(this.conIn);
    this.conOut = h("div", "cx-con-out");
    con.addEventListener("submit", function (e) { e.preventDefault(); self.consoleLine(); });
    var tools = h("div", "cx-tools");
    var cbox = h("div");
    cbox.appendChild(con);
    cbox.appendChild(this.conOut);
    tools.appendChild(cbox);
    this.about = h("p", "cx-about");
    tools.appendChild(this.about);
    left.appendChild(tools);
    body.appendChild(left);
    this.results = h("div", "cx-results");
    body.appendChild(this.results);
    el.appendChild(body);
    if (this.kind === "vector") {
      this.tableBox = h("div", "cx-table-box");
      el.appendChild(this.tableBox);
    }

    this.presetSel.addEventListener("change", function () { self.preset(+self.presetSel.value); });
    this.status.textContent = "Loading Terraflux's engine…";

    Promise.all([startEngine(ROOT + "js/calc-engine.js?v=" + (el.dataset.v || "")), script(el.getAttribute("data-src"))]).then(function (r) {
      self.call = r[0];
      self.data = window.TFCalcData[self.kind];
      el.classList.remove("is-loading");
      self.about.textContent = self.data.about + " The script runs on every cell (feature) in your browser, computed by Terraflux " + window.TFCalcEngine.version + "'s own engine.";
      self.editor.complete = function (t, pos) { var a = self.ask(self.kind + ".complete", { text: t, cursor: pos }); return a.ok; };
      if (self.kind === "vector") self.setupFeatures();
      self.preset(self.spec.start || 0);
    }).catch(function (e) {
      el.classList.remove("is-loading");
      self.status.className = "cx-status is-err";
      self.status.textContent = "The playground could not start: " + e.message + ".";
    });
  }

  Playground.prototype.setOp = function (o) {
    this.op = o;
    for (var k in this.opBtns) this.opBtns[k].classList.toggle("is-on", k === o);
    this.columnField.hidden = o !== "calc";
  };

  Playground.prototype.preset = function (i) {
    var p = this.spec.presets[i];
    this.presetSel.value = i;
    if (this.kind === "vector") {
      this.setOp(p.op || "calc");
      this.column.value = p.column || "";
    }
    this.extra = {};
    var self = this;
    Object.keys(p.inputs || {}).forEach(function (k) { self.extra[k] = p.inputs[k]; });
    this.drawInputs();
    this.editor.set(p.script.replace(/\n$/, ""));
    this.conOut.innerHTML = "";
    this.compute();
  };

  /* The node's other inputs (numbers, texts), each in a box of its own. */
  Playground.prototype.drawInputs = function () {
    var self = this;
    this.inputsBox.innerHTML = "";
    Object.keys(this.extra).forEach(function (k) {
      var l = h("label", "cx-field cx-input");
      l.appendChild(h("code", null, k + " ="));
      var inp = h("input");
      var v = self.extra[k];
      inp.value = typeof v === "string" ? '"' + v + '"' : String(v);
      inp.size = Math.max(4, inp.value.length + 1);
      inp.title = "An input of the node: a number, or a text in quotes";
      inp.addEventListener("input", function () {
        var t = inp.value.trim();
        var m = /^["'](.*)["']$/.exec(t);
        self.extra[k] = m ? m[1] : t === "" || isNaN(+t) ? t : +t;
        inp.size = Math.max(4, inp.value.length + 1);
        self.schedule(200);
      });
      l.appendChild(inp);
      self.inputsBox.appendChild(l);
    });
  };

  /* The data of a request in JSON, made once: whole (to compute), or light (the names and kinds only: to colour,
     check and complete a script as it is typed). */
  Playground.prototype.dataJson = function (light) {
    var key = light ? "lightJson" : "fullJson";
    if (!this[key]) {
      var d = this.data;
      if (this.kind === "raster") {
        var inputs = {};
        Object.keys(d.inputs).forEach(function (k) {
          var v = d.inputs[k];
          inputs[k] = !light ? v : v.stack ? { stack: { layers: v.stack.layers, cells: [] } } : { map: [] };
        });
        this[key] = JSON.stringify(inputs);
      } else {
        var f = d.features;
        this[key] = JSON.stringify(light
          ? { geometries: [], attributes: f.attributes.map(function (a) { return a.texts ? { name: a.name, texts: [] } : { name: a.name, numbers: [] }; }) }
          : f);
      }
    }
    return this[key];
  };

  Playground.prototype.ask = function (fn, args) {
    var light = !/\.(run|line)$/.test(fn);
    var r = { fn: fn };
    for (var k in args) r[k] = args[k];
    var extra = {};
    for (var e in this.extra) {
      var v = this.extra[e];
      extra[e] = this.kind === "raster" ? (typeof v === "string" ? { text: v } : { number: v }) : v;
    }
    var s = JSON.stringify(r).slice(0, -1);
    if (this.kind === "raster") {
      var inputs = this.dataJson(light);
      var more = JSON.stringify(extra).slice(1, -1);
      s += ',"width":' + this.data.width + ',"height":' + this.data.height + ',"inputs":' + inputs.slice(0, -1) + (more ? "," + more : "") + "}}";
    } else {
      s += ',"features":' + this.dataJson(light) + ',"inputs":' + JSON.stringify(extra) + "}";
    }
    return this.call(s);
  };

  Playground.prototype.schedule = function (ms) {
    var self = this;
    clearTimeout(this.timer);
    this.timer = setTimeout(function () { self.compute(); }, ms);
  };

  Playground.prototype.compute = function () {
    if (!this.call) return;
    var text = this.editor.value();
    var parts = this.ask(this.kind + ".parts", { text: text });
    this.editor.parts = parts.ok || [];
    var err = null;
    if (this.kind === "raster") {
      var p = this.ask("raster.problem", { text: text });
      if (p.ok) err = p.ok;
    }
    var t0 = performance.now();
    var r = null;
    if (!err) {
      r = this.kind === "raster"
        ? this.ask("raster.run", { text: text, names: this.names })
        : this.ask("vector.run", { text: text, op: this.op, column: this.column.value.trim(), names: this.names });
      if (r.error) err = { message: r.error, line: lineOf(r.error) };
    }
    var ms = performance.now() - t0;
    if (err && err.start == null && err.line) {
      // the error's line underlined
      var lines = text.split("\n"), at = 0;
      for (var i = 0; i < err.line - 1 && i < lines.length; i++) at += lines[i].length + 1;
      var line = lines[err.line - 1] || "";
      var lead = line.length - line.replace(/^\s+/, "").length;
      err.start = at + lead;
      err.end = at + line.replace(/\s+$/, "").length;
    }
    this.editor.error = err;
    this.editor.paint();
    if (err) {
      this.status.className = "cx-status is-err";
      this.status.innerHTML = "";
      this.status.appendChild(h("b", null, err.line ? "Line " + err.line + ": " : ""));
      this.status.appendChild(document.createTextNode(err.message.replace(/^(.*?: )?line \d+: /, "$1")));
      this.el.classList.add("is-stale");
      return;
    }
    this.el.classList.remove("is-stale");
    this.status.className = "cx-status is-ok";
    if (this.kind === "raster") this.drawRaster(r.ok, ms);
    else this.drawVector(r.ok, ms);
  };

  /* The values of every input at a cell, for the pointer's tip. */
  Playground.prototype.cellInputs = function (i) {
    var d = this.data.inputs, out = "";
    var read = (this.readNames || []);
    for (var k in d) {
      if (d[k].stack) {
        d[k].stack.layers.forEach(function (l, j) {
          var n = k + "." + layerName(l);
          if (read.indexOf(n) >= 0 || read.indexOf(k) >= 0) out += "<span>" + esc(n) + " = " + esc(fmt(d[k].stack.cells[j][i])) + "</span>";
        });
      } else if (d[k].map && read.indexOf(k) >= 0) {
        out += "<span>" + esc(k) + " = " + esc(fmt(d[k].map[i])) + "</span>";
      }
    }
    return out;
  };

  Playground.prototype.drawRaster = function (r, ms) {
    var self = this;
    var w = this.data.width, hh = this.data.height;
    var text = this.editor.value();
    // the inputs the script reads, told under the pointer
    this.readNames = [];
    var m, re = /\b([A-Za-z_][A-Za-z0-9_]*)(?:\.([A-Za-z_][A-Za-z0-9_]*))?/g;
    var noComments = text.replace(/#.*$/gm, "");
    while ((m = re.exec(noComments))) {
      if (this.data.inputs[m[1]]) this.readNames.push(m[2] ? m[1] + "." + m[2] : m[1]);
    }
    var opts = { classes: this.data.classes.lu, at: function (i) { var t = self.cellInputs(i); return t ? '<span class="cx-in">' + t + "</span>" : ""; } };
    this.results.innerHTML = "";
    var shown = r.outputs;
    var given = {};
    r.outputs.forEach(function (o) { given[o.name] = true; });
    var maps = 0, nums = 0, stacks = 0;
    function draw(list, into, note) {
      list.forEach(function (o) {
        if (o.map) { maps++; into.appendChild(mapCard(o.name, o.map, w, hh, Object.assign({ kind: note(o) || "a map" }, opts))); }
        else if (o.stack) { stacks++; into.appendChild(stackCard(o.name, o.stack, w, hh, opts)); }
        else { nums++; into.appendChild(numberCard(o.name, o.number, note(o))); }
      });
    }
    var grid = h("div", "cx-grid");
    if (this.names && r.names) {
      draw(r.names, grid, function (o) { return given[o.name] ? (o.map ? "a map, out" : "a number, out") : (o.map ? "a map" : "a number"); });
    } else {
      draw(shown, grid, function () { return ""; });
    }
    this.results.appendChild(grid);
    var bits = [];
    if (maps) bits.push(maps + (maps > 1 ? " maps" : " map"));
    if (stacks) bits.push(stacks + (stacks > 1 ? " stacks" : " stack"));
    if (nums) bits.push(nums + (nums > 1 ? " numbers" : " number"));
    this.status.innerHTML = "";
    this.status.appendChild(h("b", null, "✓ "));
    this.status.appendChild(document.createTextNode((this.names ? "Every name: " : "Its outputs: ") + bits.join(", ") + ", on " + fmt(w * hh) + " cells in " + Math.max(1, Math.round(ms)) + " ms."));
  };

  // ------------------------------------------------------------------------------------------- features

  Playground.prototype.setupFeatures = function () {
    var self = this;
    var box = h("div", "cx-fmap");
    var fm = featureMap(this.data, function (i, e) { self.hoverFeature(i, e); });
    this.fmap = fm;
    box.appendChild(fm.svg);
    this.ftip = h("div", "cx-tip");
    this.ftip.hidden = true;
    box.appendChild(this.ftip);
    this.fbox = box;
    this.colorSel = h("select");
    this.colorSel.addEventListener("change", function () { self.colorBy = self.colorSel.value; self.paintFeatures(); });
    this.sortBy = null;
  };

  function columnValues(c) { return c.texts || c.numbers; }

  Playground.prototype.hoverFeature = function (i, e) {
    var tip = this.ftip;
    if (i < 0 || !this.attrs) { tip.hidden = true; this.markRow(-1); return; }
    var self = this;
    var out = "<b>feature " + (i + 1) + (this.kept ? (this.keptSet.has(i) ? " · kept" : " · left out") : "") + "</b>";
    this.attrs.forEach(function (a) {
      var v = columnValues(a)[i];
      out += '<span class="' + (self.given[a.name] ? "is-given" : "") + '">' + esc(a.name) + " = " + esc(a.texts ? (v ? '"' + v + '"' : "None") : fmt(v)) + "</span>";
    });
    tip.innerHTML = out;
    tip.hidden = false;
    var r = this.fbox.getBoundingClientRect();
    var left = e.clientX - r.left + 14, top = e.clientY - r.top + 14;
    if (left + 230 > r.width) left = Math.max(0, e.clientX - r.left - 240);
    if (top + 220 > r.height) top = Math.max(0, e.clientY - r.top - 200);
    tip.style.left = left + "px";
    tip.style.top = top + "px";
    this.markRow(i);
  };

  Playground.prototype.markRow = function (i) {
    if (!this.rows) return;
    if (this.onRow != null && this.rows[this.onRow]) this.rows[this.onRow].classList.remove("is-hover");
    this.onRow = i;
    if (i >= 0 && this.rows[i]) this.rows[i].classList.add("is-hover");
    this.fmap.paths.forEach(function (p, k) { p.classList.toggle("is-hover", k === i); });
  };

  Playground.prototype.paintFeatures = function () {
    var a = (this.attrs || []).find(function (x) { return x.name === this.colorBy; }, this) || (this.attrs || [])[0];
    if (!a) return;
    var vals = columnValues(a);
    var paths = this.fmap.paths;
    var kept = this.kept ? this.keptSet : null;
    var leg;
    if (a.texts) {
      var known = this.data.classes.cover || {};
      var keys = Array.from(new Set(vals.filter(function (v) { return v; }))).sort();
      var colors = {};
      keys.forEach(function (k, i) { colors[k] = known[k] || CLASSES[i % CLASSES.length]; });
      paths.forEach(function (p, i) {
        var v = vals[i];
        p.style.fill = v ? colors[v] : "transparent";
      });
      leg = h("div", "cx-legend");
      var counts = {};
      vals.forEach(function (v) { counts[v || ""] = (counts[v || ""] || 0) + 1; });
      keys.forEach(function (k) {
        var it = h("span", "cx-key");
        var sw = h("i");
        sw.style.background = colors[k];
        it.appendChild(sw);
        it.appendChild(document.createTextNode('"' + k + '" · ' + counts[k]));
        leg.appendChild(it);
      });
      if (counts[""]) leg.appendChild(h("span", "cx-key cx-none", "None · " + counts[""]));
    } else {
      var s = summary(vals);
      var sc = scheme(s, null);
      if (sc.kind === "ramp") {
        // by rank: a few large features do not take the whole ramp
        var sorted = vals.filter(function (x) { return x !== null && !isNaN(x); }).sort(function (a, b) { return a - b; });
        var rank = function (x) {
          var lo = 0, hi = sorted.length;
          while (lo < hi) { var m = (lo + hi) >> 1; if (sorted[m] < x) lo = m + 1; else hi = m; }
          return sorted.length > 1 ? lo / (sorted.length - 1) : 0.5;
        };
        paths.forEach(function (p, i) {
          var x = vals[i];
          p.style.fill = x === null || isNaN(x) ? "transparent" : "rgb(" + ramp(rank(x)).join(",") + ")";
        });
        leg = legend(sc, vals.length);
        var med = sorted.length ? sorted[Math.floor((sorted.length - 1) / 2)] : NaN;
        leg.appendChild(h("span", "cx-none", "coloured by rank: the median, " + fmt(med) + ", in the middle"));
      } else {
        paths.forEach(function (p, i) {
          var c = colorOf(sc, vals[i]);
          p.style.fill = c ? "rgb(" + c.join(",") + ")" : "transparent";
        });
        leg = legend(sc, vals.length);
      }
    }
    paths.forEach(function (p, i) { p.classList.toggle("is-out", !!kept && !kept.has(i)); });
    if (this.legendBox) this.legendBox.replaceWith(leg);
    this.legendBox = leg;
    if (this.fcard && !leg.parentNode) this.fcard.appendChild(leg);
  };

  Playground.prototype.drawVector = function (r, ms) {
    var self = this;
    var base = this.data.features.attributes;
    var n = this.data.features.geometries.length;
    this.given = {};
    if (r.kept) {
      this.kept = r.kept;
      this.keptSet = new Set(r.kept);
      this.attrs = base.slice();
    } else {
      this.kept = null;
      this.keptSet = null;
      this.attrs = r.attributes;
      (r.given || []).forEach(function (g) { self.given[g] = true; });
    }
    if (this.names && r.names) {
      r.names.forEach(function (c) {
        if (!self.given[c.name] && !self.attrs.some(function (a) { return a.name === c.name; })) {
          self.attrs = self.attrs.concat([Object.assign({ name: c.name, inner: true }, c)]);
        }
      });
    }
    // colour by: the first attribute given, else what it was, else the land cover
    var names = this.attrs.map(function (a) { return a.name; });
    var first = Object.keys(this.given)[0];
    if (!this.colorBy || names.indexOf(this.colorBy) < 0 || (first && this.lastFirst !== first)) this.colorBy = first || "cover";
    this.lastFirst = first;
    this.colorSel.innerHTML = "";
    names.forEach(function (nm) {
      var o = h("option", null, nm + (self.given[nm] ? " (given)" : ""));
      o.value = nm;
      self.colorSel.appendChild(o);
    });
    this.colorSel.value = this.colorBy;

    this.results.innerHTML = "";
    var card = h("figure", "cx-card cx-fcard");
    var head = h("figcaption", "cx-card-head");
    head.appendChild(h("span", "cx-kind", r.kept ? "the features kept, coloured by" : "the features, coloured by"));
    head.appendChild(this.colorSel);
    card.appendChild(head);
    card.appendChild(this.fbox);
    this.fcard = card;
    this.legendBox = null;
    this.results.appendChild(card);
    this.paintFeatures();
    this.drawTable();
    this.status.innerHTML = "";
    this.status.appendChild(h("b", null, "✓ "));
    var given = Object.keys(this.given);
    this.status.appendChild(document.createTextNode(r.kept
      ? "vector.filter keeps " + r.kept.length + " of " + n + " features (" + (100 * r.kept.length / n).toFixed(1) + " %), in " + Math.max(1, Math.round(ms)) + " ms."
      : "vector.calc gives " + given.join(", ") + " to " + n + " features, in " + Math.max(1, Math.round(ms)) + " ms."));
  };

  Playground.prototype.drawTable = function () {
    var self = this;
    var attrs = this.attrs;
    var n = this.data.features.geometries.length;
    var order = [];
    for (var i = 0; i < n; i++) if (!this.keptSet || this.keptSet.has(i)) order.push(i);
    if (this.sortBy) {
      var c = attrs.find(function (a) { return a.name === self.sortBy.name; });
      if (c) {
        var v = columnValues(c), dir = this.sortBy.dir;
        order.sort(function (a, b) {
          var x = v[a], y = v[b];
          var xm = x === null || x === "" || (typeof x === "number" && isNaN(x)), ym = y === null || y === "" || (typeof y === "number" && isNaN(y));
          if (xm || ym) return xm - ym;
          return (x < y ? -1 : x > y ? 1 : 0) * dir;
        });
      }
    }
    var box = this.tableBox;
    box.innerHTML = "";
    var cap = h("p", "cx-table-cap");
    cap.textContent = (this.keptSet ? "The features kept (" + order.length + " of " + n + ")" : "The features' attributes (" + n + ")") + ": click a column to sort, point at a row to find its feature on the map.";
    box.appendChild(cap);
    var wrap = h("div", "cx-table-wrap");
    var t = h("table", "cx-table");
    var thead = h("thead"), tr = h("tr");
    var th0 = h("th", null, "feature");
    tr.appendChild(th0);
    attrs.forEach(function (a) {
      var th = h("th", (self.given[a.name] ? "is-given" : "") + (a.inner ? " is-inner" : ""));
      th.textContent = a.name;
      th.title = self.given[a.name] ? "given by the script" : a.inner ? "a name of the script (not an attribute)" : a.texts ? "texts" : "numbers";
      if (self.sortBy && self.sortBy.name === a.name) th.textContent += self.sortBy.dir > 0 ? " ▲" : " ▼";
      th.addEventListener("click", function () {
        self.sortBy = self.sortBy && self.sortBy.name === a.name ? { name: a.name, dir: -self.sortBy.dir } : { name: a.name, dir: 1 };
        self.drawTable();
      });
      tr.appendChild(th);
    });
    thead.appendChild(tr);
    t.appendChild(thead);
    var tb = h("tbody");
    this.rows = {};
    order.forEach(function (i) {
      var row = h("tr");
      row.appendChild(h("td", "cx-num", String(i + 1)));
      attrs.forEach(function (a) {
        var v = columnValues(a)[i];
        var td = h("td", (a.texts ? "cx-txt" : "cx-num") + (self.given[a.name] ? " is-given" : ""));
        td.textContent = a.texts ? (v ? v : "None") : fmt(v);
        if ((a.texts && !v) || (!a.texts && (v === null || isNaN(v)))) td.classList.add("is-none");
        row.appendChild(td);
      });
      row.addEventListener("mouseenter", function () { self.markRow(i); });
      row.addEventListener("mouseleave", function () { self.markRow(-1); });
      self.rows[i] = row;
      tb.appendChild(row);
    });
    t.appendChild(tb);
    wrap.appendChild(t);
    box.appendChild(wrap);
  };

  // ------------------------------------------------------------------------------------------- the console

  Playground.prototype.consoleLine = function () {
    var line = this.conIn.value.trim();
    if (!line || !this.call) return;
    var a = this.ask(this.kind + ".line", { text: this.editor.value(), line: line });
    var row = h("div", "cx-con-row");
    row.appendChild(h("code", "cx-con-in", "> " + line));
    if (a.error) {
      row.appendChild(h("span", "cx-con-err", a.error));
    } else if (this.kind === "raster") {
      var v = a.ok.value;
      if (v.number !== undefined) row.appendChild(h("span", "cx-con-val", fmt(v.number)));
      else if (v.map) {
        var s = summary(v.map);
        row.appendChild(h("span", "cx-con-val", "a map: least " + fmt(s.lo) + ", mean " + fmt(s.mean) + ", largest " + fmt(s.hi) + (s.missing ? ", " + fmt(s.missing) + " missing" : "")));
      } else if (v.stack) {
        row.appendChild(h("span", "cx-con-val", "a stack of " + v.stack.length + " layers: " + v.stack.map(function (l) { return layerName(l.key); }).join(", ")));
      }
    } else {
      var vals = columnValues(a.ok);
      var same = vals.every(function (x) { return x === vals[0] || (x !== x && vals[0] !== vals[0]); });
      var show = function (x) { return a.ok.texts ? (x ? '"' + x + '"' : "None") : fmt(x); };
      if (same) row.appendChild(h("span", "cx-con-val", show(vals[0]) + (vals.length > 1 ? " (every feature)" : "")));
      else row.appendChild(h("span", "cx-con-val", "a value per feature: " + vals.slice(0, 6).map(show).join(", ") + (vals.length > 6 ? ", … (" + vals.length + ")" : "")));
    }
    this.conOut.insertBefore(row, this.conOut.firstChild);
    while (this.conOut.children.length > 6) this.conOut.lastChild.remove();
    this.conIn.select();
  };

  function start() {
    var els = document.querySelectorAll(".tf-calc");
    if (!els.length) return;
    var go = function (el) { if (!el.dataset.started) { el.dataset.started = "1"; new Playground(el); } };
    if ("IntersectionObserver" in window) {
      var io = new IntersectionObserver(function (es) {
        es.forEach(function (e) { if (e.isIntersecting) { go(e.target); io.unobserve(e.target); } });
      }, { rootMargin: "400px" });
      els.forEach(function (el) { io.observe(el); });
    } else {
      els.forEach(go);
    }
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
