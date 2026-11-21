/* Terraflux website: theme, menu, search, copy, the table of contents, the lightbox (pan and zoom), sliders,
   before-and-after, charts and the downloads. No dependencies. */
(function () {
  "use strict";
  var $ = function (s, r) { return (r || document).querySelector(s); };
  var $$ = function (s, r) { return Array.prototype.slice.call((r || document).querySelectorAll(s)); };
  var ROOT = window.TF_ROOT || "./";
  var reduce = window.matchMedia && matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------------------------------------------------------------- theme and menus */
  var tt = $(".theme-toggle");
  if (tt) tt.addEventListener("click", function () {
    var t = document.documentElement.getAttribute("data-theme") === "dark" ? "light" : "dark";
    document.documentElement.setAttribute("data-theme", t);
    try {
      /* choosing the system's own theme is going back to following it */
      var sys = window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
      if (t === sys) localStorage.removeItem("tf-theme"); else localStorage.setItem("tf-theme", t);
    } catch (e) {}
    $$(".tf-chart").forEach(function (c) { if (c._draw) c._draw(); });
  });
  /* without a choice of the reader's (the button, ?theme=), the theme follows the system's, also when it changes */
  try {
    var mq = window.matchMedia("(prefers-color-scheme: dark)");
    var follow = function () {
      var chosen = null;
      try { chosen = localStorage.getItem("tf-theme"); } catch (e) {}
      if (chosen === "dark" || chosen === "light" || /[?&]theme=(light|dark)/.test(location.search)) return;
      document.documentElement.setAttribute("data-theme", mq.matches ? "dark" : "light");
      $$(".tf-chart").forEach(function (c) { if (c._draw) c._draw(); });
    };
    if (mq.addEventListener) mq.addEventListener("change", follow); else if (mq.addListener) mq.addListener(follow);
  } catch (e) {}
  var mt = $(".menu-toggle"), nav = $(".mainnav");
  if (mt && nav) mt.addEventListener("click", function () {
    var open = nav.classList.toggle("is-open");
    mt.setAttribute("aria-expanded", open ? "true" : "false");
  });
  var side = $(".doc-side");
  if (side) side.addEventListener("click", function (e) {
    if (e.target === side && window.innerWidth <= 900) side.classList.toggle("is-open");
  });

  /* ---------------------------------------------------------------- copy */
  document.addEventListener("click", function (e) {
    var b = e.target.closest && e.target.closest(".copy");
    if (!b) return;
    var block = b.parentElement.querySelector("pre");
    if (!block) return;
    var text = block.innerText.replace(/\n$/, "");
    var done = function () { b.textContent = "Copied"; b.classList.add("is-done"); setTimeout(function () { b.textContent = "Copy"; b.classList.remove("is-done"); }, 1600); };
    if (navigator.clipboard) navigator.clipboard.writeText(text).then(done, function () {});
    else { var t = document.createElement("textarea"); t.value = text; document.body.appendChild(t); t.select(); document.execCommand("copy"); t.remove(); done(); }
  });

  /* ---------------------------------------------------------------- table of contents follows the reading */
  var tocLinks = $$(".doc-toc a");
  if (tocLinks.length && "IntersectionObserver" in window) {
    var map = {};
    tocLinks.forEach(function (a) { map[decodeURIComponent(a.getAttribute("href").slice(1))] = a; });
    var heads = Object.keys(map).map(function (id) { return document.getElementById(id); }).filter(Boolean);
    var visible = {};
    var io = new IntersectionObserver(function (es) {
      es.forEach(function (en) { visible[en.target.id] = en.isIntersecting; });
      var first = heads.find(function (h) { return visible[h.id]; });
      if (!first) return;
      tocLinks.forEach(function (a) { a.classList.remove("is-on"); });
      map[first.id].classList.add("is-on");
    }, { rootMargin: "-70px 0px -65% 0px" });
    heads.forEach(function (h) { io.observe(h); });
  }

  /* ---------------------------------------------------------------- math */
  if ($(".arithmatex")) {
    var tryMath = function (n) {
      if (window.renderMathInElement) {
        $$(".arithmatex").forEach(function (el) {
          window.renderMathInElement(el, { delimiters: [{ left: "\\(", right: "\\)", display: false }, { left: "\\[", right: "\\]", display: true }], throwOnError: false });
        });
      } else if (n < 60) setTimeout(function () { tryMath(n + 1); }, 100);
    };
    tryMath(0);
  }

  /* ---------------------------------------------------------------- search */
  var dlg = $(".search-dialog"), sInput = dlg && $("input", dlg), sList = dlg && $(".search-results", dlg);
  var index = null, sel = 0, results = [];
  function loadIndex() {
    if (index) return Promise.resolve(index);
    // opened from the disk (the offline site), where fetch is refused: the index as a script (search.js)
    var got = location.protocol === "file:" ? new Promise(function (ok, fail) {
      if (window.TF_SEARCH) return ok(window.TF_SEARCH);
      var s = document.createElement("script");
      s.src = ROOT + "search.js";
      s.onload = function () { ok(window.TF_SEARCH || []); };
      s.onerror = fail;
      document.head.appendChild(s);
    }) : fetch(ROOT + "search.json").then(function (r) { return r.json(); });
    return got.then(function (d) {
      index = d.map(function (e) { e._t = e.t.toLowerCase(); e._h = (e.h || "").toLowerCase(); e._x = (e.x || "").toLowerCase(); return e; });
      return index;
    });
  }
  function openSearch() {
    if (!dlg) return;
    dlg.hidden = false; document.body.classList.add("lb-open");
    sInput.value = ""; sList.innerHTML = ""; sInput.focus(); loadIndex();
  }
  function closeSearch() { if (dlg) { dlg.hidden = true; document.body.classList.remove("lb-open"); } }
  function escapeHtml(s) { return s.replace(/[&<>"]/g, function (c) { return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]; }); }
  function snippet(e, words) {
    var x = e.x || "", lx = e._x, at = -1;
    for (var i = 0; i < words.length && at < 0; i++) at = lx.indexOf(words[i]);
    var s = at < 0 ? x.slice(0, 140) : x.slice(Math.max(0, at - 50), at + 110);
    var h = escapeHtml(s);
    words.forEach(function (w) { if (w.length > 1) h = h.replace(new RegExp("(" + w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + ")", "ig"), "<mark>$1</mark>"); });
    return (at > 50 ? "… " : "") + h;
  }
  function runSearch() {
    var q = sInput.value.trim().toLowerCase();
    if (!q || !index) { sList.innerHTML = ""; return; }
    var words = q.split(/\s+/).filter(Boolean);
    results = [];
    index.forEach(function (e) {
      var score = 0;
      for (var i = 0; i < words.length; i++) {
        var w = words[i], s = 0;
        if (e._t.indexOf(w) >= 0) s += e._t.indexOf(w) === 0 ? 30 : 18;
        if (e._h.indexOf(w) >= 0) s += 8;
        var c = e._x.split(w).length - 1;
        if (c) s += Math.min(c, 8);
        if (!s) return;
        score += s;
      }
      if (e._t === q) score += 40;
      if (e.s && e.s.indexOf("Operators") === 0) score *= 0.8;
      results.push([score, e]);
    });
    results.sort(function (a, b) { return b[0] - a[0]; });
    results = results.slice(0, 30).map(function (r) { return r[1]; });
    sel = 0;
    if (!results.length) { sList.innerHTML = '<li class="search-empty">Nothing found. Try fewer or other words.</li>'; return; }
    sList.innerHTML = results.map(function (e, i) {
      return '<li role="option"' + (i === 0 ? ' class="is-sel"' : "") + '><a href="' + ROOT + e.u + '"><span class="r-sec">' + escapeHtml(e.s || "") +
        '</span><span class="r-title">' + escapeHtml(e.t) + '</span><span class="r-text">' + snippet(e, words) + "</span></a></li>";
    }).join("");
  }
  if (dlg) {
    $$(".search-open").forEach(function (b) { b.addEventListener("click", openSearch); });
    sInput.addEventListener("input", function () { loadIndex().then(runSearch); });
    dlg.addEventListener("click", function (e) { if (e.target === dlg) closeSearch(); });
    sInput.addEventListener("keydown", function (e) {
      var items = $$("li[role=option]", sList);
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        if (!items.length) return;
        items[sel].classList.remove("is-sel");
        sel = (sel + (e.key === "ArrowDown" ? 1 : items.length - 1)) % items.length;
        items[sel].classList.add("is-sel"); items[sel].scrollIntoView({ block: "nearest" });
      } else if (e.key === "Enter" && items[sel]) { location.href = $("a", items[sel]).href; }
    });
  }
  document.addEventListener("keydown", function (e) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") { e.preventDefault(); openSearch(); }
    else if (e.key === "/" && !/input|textarea|select/i.test(document.activeElement.tagName) && dlg && dlg.hidden) { e.preventDefault(); openSearch(); }
    else if (e.key === "Escape") { closeSearch(); closeLightbox(); }
  });

  /* ---------------------------------------------------------------- lightbox: pan and zoom any figure */
  var lb = $(".lightbox"), stage = lb && $(".lb-stage", lb), content = lb && $(".lb-content", lb);
  var st = { s: 1, x: 0, y: 0, w: 0, h: 0, fit: 1 }, lastFocus = null;
  function apply() { content.style.transform = "translate(" + st.x + "px," + st.y + "px) scale(" + st.s + ")"; }
  function fit() {
    var r = stage.getBoundingClientRect();
    st.w = content.offsetWidth; st.h = content.offsetHeight;
    st.fit = Math.min((r.width - 32) / st.w, (r.height - 32) / st.h, 4);
    if (st.fit <= 0 || !isFinite(st.fit)) st.fit = 1;
    st.s = st.fit; st.x = (r.width - st.w * st.s) / 2; st.y = (r.height - st.h * st.s) / 2; apply();
  }
  function zoomAt(f, cx, cy) {
    var ns = Math.max(st.fit * 0.5, Math.min(st.s * f, Math.max(12, st.fit * 12)));
    f = ns / st.s; st.x = cx - (cx - st.x) * f; st.y = cy - (cy - st.y) * f; st.s = ns; apply();
  }
  function openLightbox(el) {
    if (!lb) return;
    lastFocus = document.activeElement;
    content.innerHTML = "";
    var img = el.querySelector(".sl-frame.is-on img") || el.querySelector("img");
    var svg = el.querySelector("svg.tfg");
    if (svg) {
      var wrap = document.createElement("div"); wrap.className = "tfg-canvas";
      wrap.appendChild(svg.cloneNode(true)); content.appendChild(wrap);
    } else if (img) {
      var big = new Image();
      big.alt = img.alt; big.src = img.currentSrc || img.src;
      var nw = img.naturalWidth || img.width;
      big.style.width = Math.max(nw, 200) + "px";
      content.appendChild(big);
      if (!big.complete) big.onload = fit;
    } else return;
    $(".lb-caption", lb).textContent = el.getAttribute("data-caption") || (img && img.alt) || "";
    lb.hidden = false; document.body.classList.add("lb-open");
    requestAnimationFrame(fit);
    $(".lb-close", lb).focus();
  }
  function closeLightbox() {
    if (!lb || lb.hidden) return;
    lb.hidden = true; document.body.classList.remove("lb-open"); content.innerHTML = "";
    if (lastFocus) lastFocus.focus();
  }
  if (lb) {
    document.addEventListener("click", function (e) {
      var z = e.target.closest && e.target.closest(".zoomable");
      if (!z || e.target.closest("input, .sl-controls, a")) return;
      e.preventDefault(); openLightbox(z);
    });
    document.addEventListener("keydown", function (e) {
      if (e.key === "Enter" && document.activeElement.classList && document.activeElement.classList.contains("zoomable")) openLightbox(document.activeElement);
    });
    $(".lb-close", lb).addEventListener("click", closeLightbox);
    $(".lb-fit", lb).addEventListener("click", fit);
    var center = function () { var r = stage.getBoundingClientRect(); return [r.width / 2, r.height / 2]; };
    $(".lb-in", lb).addEventListener("click", function () { var c = center(); zoomAt(1.4, c[0], c[1]); });
    $(".lb-out", lb).addEventListener("click", function () { var c = center(); zoomAt(1 / 1.4, c[0], c[1]); });
    stage.addEventListener("wheel", function (e) {
      e.preventDefault();
      var r = stage.getBoundingClientRect();
      zoomAt(Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.0018)), e.clientX - r.left, e.clientY - r.top);
    }, { passive: false });
    stage.addEventListener("dblclick", function (e) {
      var r = stage.getBoundingClientRect();
      if (st.s > st.fit * 1.05) fit(); else zoomAt(2.5, e.clientX - r.left, e.clientY - r.top);
    });
    var ptrs = {}, last = null, pinch = null;
    stage.addEventListener("pointerdown", function (e) {
      stage.setPointerCapture(e.pointerId); ptrs[e.pointerId] = [e.clientX, e.clientY];
      stage.classList.add("is-dragging");
      var ids = Object.keys(ptrs);
      if (ids.length === 2) {
        var a = ptrs[ids[0]], b = ptrs[ids[1]];
        pinch = { d: Math.hypot(a[0] - b[0], a[1] - b[1]), s: st.s };
      }
      last = [e.clientX, e.clientY];
    });
    stage.addEventListener("pointermove", function (e) {
      if (!ptrs[e.pointerId]) return;
      ptrs[e.pointerId] = [e.clientX, e.clientY];
      var ids = Object.keys(ptrs);
      if (ids.length === 2 && pinch) {
        var a = ptrs[ids[0]], b = ptrs[ids[1]], r = stage.getBoundingClientRect();
        var d = Math.hypot(a[0] - b[0], a[1] - b[1]);
        zoomAt((pinch.s * d / pinch.d) / st.s, (a[0] + b[0]) / 2 - r.left, (a[1] + b[1]) / 2 - r.top);
      } else if (ids.length === 1 && last) {
        st.x += e.clientX - last[0]; st.y += e.clientY - last[1]; apply();
        last = [e.clientX, e.clientY];
      }
    });
    var up = function (e) { delete ptrs[e.pointerId]; if (Object.keys(ptrs).length < 2) pinch = null; if (!Object.keys(ptrs).length) { stage.classList.remove("is-dragging"); last = null; } else { var k = Object.keys(ptrs)[0]; last = ptrs[k]; } };
    stage.addEventListener("pointerup", up); stage.addEventListener("pointercancel", up);
    window.addEventListener("resize", function () { if (!lb.hidden) fit(); });
    document.addEventListener("keydown", function (e) {
      if (lb.hidden) return;
      var c = center();
      if (e.key === "+" || e.key === "=") zoomAt(1.3, c[0], c[1]);
      else if (e.key === "-") zoomAt(1 / 1.3, c[0], c[1]);
      else if (e.key === "0") fit();
    });
  }

  /* ---------------------------------------------------------------- charts */
  var PALETTE = ["#3b8a5a", "#e8901f", "#2f6f9f", "#b4508a", "#8a6d2f", "#5e57b0", "#2f9f99", "#b4372f"];
  /* The palette of the benchmark charts (horizontal bars, dots, log axes, colour by a column): five slots in a fixed order,
     never cycled, the dark steps stepped for the dark surface; both validated with the dataviz validator. */
  var BENCH = { light: ["#3b8a5a", "#e8901f", "#2f6f9f", "#b4372f", "#5e57b0"], dark: ["#4fa874", "#cf7d17", "#4d8fc4", "#d9625a", "#9a82e0"] };
  function css(v) { return getComputedStyle(document.documentElement).getPropertyValue(v).trim(); }
  function isDark() { return document.documentElement.getAttribute("data-theme") === "dark"; }
  /* the decades of a log axis from lo to hi: the majors at 1, 10, 100... and the minors at 2 to 9 of each */
  function logTicks(lo, hi) {
    var major = [], minor = [], a = Math.floor(Math.log10(lo) + 1e-9), b = Math.ceil(Math.log10(hi) - 1e-9);
    for (var e = a; e <= b; e++) for (var k = 1; k <= 9; k++) {
      var v = +(k * Math.pow(10, e)).toPrecision(12);
      if (v < lo * (1 - 1e-9) || v > hi * (1 + 1e-9)) continue;
      (k === 1 ? major : minor).push(v);
    }
    if (!major.length) major = [lo, hi];
    return { major: major, minor: minor };
  }
  function logRange(vals, cfg) {
    var pos = vals.filter(function (v) { return v > 0; });
    var mn = pos.length ? Math.min.apply(null, pos) : 1, mx = pos.length ? Math.max.apply(null, pos) : 10;
    var lo = cfg.min != null ? cfg.min : (mn >= 1 ? 1 : Math.pow(10, Math.floor(Math.log10(mn) + 1e-9)));
    var hi = cfg.max != null ? cfg.max : Math.pow(10, Math.ceil(Math.log10(mx) - 1e-9));
    if (hi <= lo) hi = lo * 10;
    return [lo, hi];
  }
  function niceTicks(lo, hi, n) {
    if (lo === hi) { hi = lo + 1; }
    var span = hi - lo, step = Math.pow(10, Math.floor(Math.log10(span / n)));
    var err = (n * step) / span;
    if (err <= 0.15) step *= 10; else if (err <= 0.35) step *= 5; else if (err <= 0.75) step *= 2;
    var t = [], v = Math.ceil(lo / step) * step;
    for (; v <= hi + step * 1e-9; v += step) t.push(+v.toFixed(12));
    return t;
  }
  function fmt(v) {
    if (typeof v !== "number") return v;
    var a = Math.abs(v);
    if (a >= 1e6) return (v / 1e6).toFixed(a >= 1e7 ? 0 : 1) + "M";
    if (a >= 1e4) return (v / 1e3).toFixed(a >= 1e5 ? 0 : 1) + "k";
    if (a >= 100 || v === Math.round(v)) return String(Math.round(v * 100) / 100);
    return v.toPrecision(3).replace(/\.?0+$/, "");
  }
  function chart(el) {
    var cfg = JSON.parse(el.getAttribute("data-chart"));
    var plot = $(".ch-plot", el), leg = $(".ch-legend", el), off = {}, hover = -1, marker = -1;
    var rows = cfg.rows, xs = rows.map(function (r) { return r[cfg.x]; });
    var numericX = xs.every(function (v) { return typeof v === "number"; }) && cfg.type !== "bar";
    var bench = !!(cfg.horizontal || cfg.type === "dot" || cfg.log || cfg.color_by || cfg.rule || cfg.labels_at_end);
    var pal = function () { return bench ? BENCH[isDark() ? "dark" : "light"] : PALETTE; };
    var color = function (k, i) { return cfg.colors[k] || pal()[i % pal().length]; };
    var label = function (k) { return cfg.labels[k] || k; };
    // colour by a column: the groups in the order they first appear, one palette slot each (never cycled)
    var groups = [];
    if (cfg.color_by) rows.forEach(function (r) { if (groups.indexOf(r[cfg.color_by]) < 0) groups.push(r[cfg.color_by]); });
    var gcolor = function (g) { var i = groups.indexOf(g); return cfg.colors[g] || (i < pal().length ? pal()[i] : css("--ink-3")); };
    var legItems = cfg.color_by ? groups.map(function (g) { return [String(g), String(g), gcolor(g)]; })
      : cfg.y.map(function (k, i) { return [k, label(k), color(k, i)]; });
    leg.innerHTML = legItems.length > 1 ? legItems.map(function (it) {
      return '<button type="button" data-k="' + it[0] + '"><i style="background:' + it[2] + '"></i>' + it[1] + "</button>";
    }).join("") : "";
    $$("button", leg).forEach(function (b) {
      b.addEventListener("click", function () { var k = b.getAttribute("data-k"); off[k] = !off[k]; b.classList.toggle("is-off", off[k]); b.setAttribute("aria-pressed", off[k] ? "false" : "true"); draw(); });
      b.setAttribute("aria-pressed", "true");
    });
    var tip = document.createElement("div"); tip.className = "ch-tip"; tip.hidden = true; el.appendChild(tip);
    // numbers as the chart shows them: `digits` decimals, then the `format` ("{v}×") or the percent sign
    function num(v) { return typeof v !== "number" ? v : cfg.digits != null ? v.toFixed(cfg.digits) : fmt(v); }
    function fv(v) { if (typeof v !== "number") return v; var t = num(v); return cfg.format ? cfg.format.replace("{v}", t) : t + (cfg.percent ? "%" : ""); }
    function ft(v) { var t = fmt(v); return cfg.format ? cfg.format.replace("{v}", t) : t + (cfg.percent ? "%" : ""); }
    function paintLegend() {
      legItems.forEach(function (it, i) { var b = $$("button", leg)[i]; if (b) $("i", b).style.background = cfg.color_by ? gcolor(groups[i]) : color(it[0], i); });
    }
    function draw() { paintLegend(); if (cfg.horizontal || cfg.type === "dot") drawH(); else drawV(); }
    function drawV() {
      var W = plot.clientWidth || 600, H = cfg.height, m = { l: 54, r: 14, t: 10, b: cfg.xlabel ? 44 : 30 };
      var keys = cfg.y.filter(function (k) { return !off[k]; });
      var vals = [];
      rows.forEach(function (r) {
        if (cfg.stacked) { var s = 0; keys.forEach(function (k) { s += +r[k] || 0; }); vals.push(s); vals.push(0); }
        else keys.forEach(function (k) { if (typeof r[k] === "number") vals.push(r[k]); });
      });
      if (!vals.length) vals = [0, 1];
      var lo = Math.min.apply(null, vals), hi = Math.max.apply(null, vals);
      var lgY = !!cfg.log && !cfg.stacked && cfg.type !== "area", lt = null;
      if (lgY) { var rg = logRange(vals, cfg); lo = rg[0]; hi = rg[1]; lt = logTicks(lo, hi); }
      else if (cfg.type === "bar" || cfg.type === "area" || cfg.stacked) lo = Math.min(lo, 0);
      var yt = lgY ? lt.major : niceTicks(lo, hi, 5);
      if (!lgY) { lo = Math.min(lo, yt[0]); hi = Math.max(hi, yt[yt.length - 1]); }
      var xlo, xhi, xpos, bw = 0;
      var lg = numericX && cfg.xlog && xs.every(function (v) { return v > 0; });
      var tx = function (v) { return lg ? Math.log10(v) : v; };
      if (numericX) { xlo = tx(Math.min.apply(null, xs)); xhi = tx(Math.max.apply(null, xs)); if (xlo === xhi) xhi = xlo + 1;
        xpos = function (i) { return m.l + (tx(xs[i]) - xlo) / (xhi - xlo) * (W - m.l - m.r); }; }
      else { bw = (W - m.l - m.r) / rows.length; xpos = function (i) { return m.l + bw * (i + 0.5); }; }
      var ypos = lgY
        ? function (v) { return m.t + (1 - (Math.log10(Math.max(v, lo)) - Math.log10(lo)) / (Math.log10(hi) - Math.log10(lo))) * (H - m.t - m.b); }
        : function (v) { return m.t + (1 - (v - lo) / (hi - lo)) * (H - m.t - m.b); };
      var s = '<svg viewBox="0 0 ' + W + " " + H + '" height="' + H + '">';
      if (lgY) lt.minor.forEach(function (t) { var y = ypos(t); s += '<line class="grid minor" x1="' + m.l + '" x2="' + (W - m.r) + '" y1="' + y + '" y2="' + y + '"/>'; });
      yt.forEach(function (t) { var y = ypos(t); s += '<line class="grid" x1="' + m.l + '" x2="' + (W - m.r) + '" y1="' + y + '" y2="' + y + '"/><text x="' + (m.l - 8) + '" y="' + (y + 4) + '" text-anchor="end">' + ft(t) + "</text>"; });
      s += '<line class="ax" x1="' + m.l + '" x2="' + (W - m.r) + '" y1="' + ypos(Math.max(lo, 0)) + '" y2="' + ypos(Math.max(lo, 0)) + '"/>';
      var xticks = null;
      if (numericX && lg) {
        xticks = [];
        for (var e = Math.floor(xlo); e <= Math.ceil(xhi); e++) [1, 2, 5].forEach(function (k) { var v = k * Math.pow(10, e); if (Math.log10(v) >= xlo - 1e-9 && Math.log10(v) <= xhi + 1e-9) xticks.push(v); });
        if (xticks.length > Math.floor((W - 80) / 50)) xticks = xticks.filter(function (v) { return Math.abs(Math.log10(v) % 1) < 1e-9; });
      } else if (numericX) xticks = niceTicks(xlo, xhi, Math.max(2, Math.floor((W - 80) / 80)));
      if (numericX) xticks.forEach(function (t) { var x = m.l + (tx(t) - xlo) / (xhi - xlo) * (W - m.l - m.r); s += '<text x="' + x + '" y="' + (H - m.b + 18) + '" text-anchor="middle">' + fmt(t) + "</text>"; });
      else { var every = Math.ceil(rows.length / Math.max(1, Math.floor((W - 60) / 70))); xs.forEach(function (v, i) { if (i % every === 0) s += '<text x="' + xpos(i) + '" y="' + (H - m.b + 18) + '" text-anchor="middle">' + fmt(v) + "</text>"; }); }
      if (cfg.xlabel) s += '<text x="' + ((W + m.l) / 2) + '" y="' + (H - 6) + '" text-anchor="middle">' + cfg.xlabel + "</text>";
      if (cfg.ylabel) s += '<text transform="translate(12,' + ((H - m.b + m.t) / 2) + ') rotate(-90)" text-anchor="middle">' + cfg.ylabel + "</text>";
      if (cfg.type === "bar") {
        var n = cfg.stacked ? 1 : keys.length, inner = bw * 0.78 / Math.max(n, 1);
        rows.forEach(function (r, i) {
          var base = 0;
          if (cfg.color_by && off[r[cfg.color_by]]) return;
          keys.forEach(function (k, j) {
            var ci = cfg.y.indexOf(k), v = +r[k] || 0, x, y0, y1;
            if (cfg.stacked) { x = xpos(i) - bw * 0.39; y0 = ypos(base); y1 = ypos(base + v); base += v; }
            else { x = xpos(i) - bw * 0.39 + j * inner; y0 = ypos(Math.max(lo, 0)); y1 = ypos(v); }
            s += '<rect x="' + x + '" y="' + Math.min(y0, y1) + '" width="' + Math.max(1, inner - 2) + '" height="' + Math.abs(y0 - y1) + '" rx="2" fill="' + (cfg.color_by ? gcolor(r[cfg.color_by]) : color(k, ci)) + '"' + (hover >= 0 && hover !== i ? ' opacity=".55"' : "") + "/>";
          });
        });
      } else {
        var acc = rows.map(function () { return 0; });
        keys.forEach(function (k) {
          var ci = cfg.y.indexOf(k), pts = [], basePts = [];
          rows.forEach(function (r, i) {
            if (typeof r[k] !== "number") return;
            var v = r[k];
            if (cfg.stacked) { basePts.push([xpos(i), ypos(acc[i])]); acc[i] += v; v = acc[i]; }
            pts.push([xpos(i), ypos(v)]);
          });
          if (!pts.length) return;
          var d = "M" + pts.map(function (p) { return p[0].toFixed(1) + "," + p[1].toFixed(1); }).join("L");
          if (cfg.type === "area" || cfg.stacked) {
            var b = cfg.stacked ? basePts.slice().reverse() : [[pts[pts.length - 1][0], ypos(Math.max(lo, 0))], [pts[0][0], ypos(Math.max(lo, 0))]];
            s += '<path class="ar" d="' + d + "L" + b.map(function (p) { return p[0].toFixed(1) + "," + p[1].toFixed(1); }).join("L") + 'Z" fill="' + color(k, ci) + '" fill-opacity="' + (cfg.stacked ? 0.85 : 0.18) + '"/>';
          }
          if (cfg.type === "scatter") pts.forEach(function (p) { s += '<circle cx="' + p[0] + '" cy="' + p[1] + '" r="3.5" fill="' + color(k, ci) + '" fill-opacity=".8"/>'; });
          else s += '<path class="ln" d="' + d + '" stroke="' + color(k, ci) + '"/>';
        });
      }
      if (cfg.rule) { var ry = ypos(cfg.rule.value); s += '<line class="rule" x1="' + m.l + '" x2="' + (W - m.r) + '" y1="' + ry + '" y2="' + ry + '"/>' + (cfg.rule.label ? '<text class="rulelbl" x="' + (W - m.r) + '" y="' + (ry - 5) + '" text-anchor="end">' + cfg.rule.label + "</text>" : ""); }
      if (marker >= 0 && marker < rows.length) s += '<line class="mark" x1="' + xpos(marker) + '" x2="' + xpos(marker) + '" y1="' + m.t + '" y2="' + (H - m.b) + '"/>';
      if (hover >= 0) {
        s += '<line class="cross" x1="' + xpos(hover) + '" x2="' + xpos(hover) + '" y1="' + m.t + '" y2="' + (H - m.b) + '"/>';
        if (cfg.type !== "bar") { var acc2 = 0; keys.forEach(function (k) { var v = rows[hover][k]; if (typeof v !== "number") return; if (cfg.stacked) { acc2 += v; v = acc2; } s += '<circle class="dot" cx="' + xpos(hover) + '" cy="' + ypos(v) + '" r="4.5" fill="' + color(k, cfg.y.indexOf(k)) + '"/>'; }); }
      }
      s += '<rect x="' + m.l + '" y="' + m.t + '" width="' + (W - m.l - m.r) + '" height="' + (H - m.t - m.b) + '" fill="transparent" class="hit"/></svg>';
      plot.innerHTML = s;
      var hit = $(".hit", plot);
      hit.addEventListener("pointermove", function (e) {
        var r = plot.getBoundingClientRect(), x = (e.clientX - r.left) * (W / r.width), best = 0, bd = 1e9;
        rows.forEach(function (_, i) { var d = Math.abs(xpos(i) - x); if (d < bd) { bd = d; best = i; } });
        if (best !== hover) { hover = best; draw(); }
        tip.hidden = false;
        tip.innerHTML = "<b>" + (cfg.xlabel ? cfg.xlabel + " " : "") + fmt(rows[best][cfg.x]) + "</b><br>" + keys.map(function (k) {
          var v = rows[best][k]; return '<i style="background:' + color(k, cfg.y.indexOf(k)) + '"></i>' + label(k) + ": " + fv(v);
        }).join("<br>");
        tip.style.left = (xpos(best) / W * r.width) + "px"; tip.style.top = (plot.offsetTop + 20) + "px";
      });
      hit.addEventListener("pointerleave", function () { hover = -1; tip.hidden = true; draw(); });
    }
    /* Horizontal bars and dot plots: the categories down the left (above each bar on a narrow screen), the values
       along x, linear or log. The rows are not redrawn on hover: the row under the pointer or the focus is marked. */
    var mctx = document.createElement("canvas").getContext("2d");
    function tw(t, font) { mctx.font = font + " " + getComputedStyle(plot).fontFamily; return mctx.measureText(t).width; }
    function trunc(t, maxw, font) {
      if (tw(t, font) <= maxw) return t;
      var a = 0, b = t.length;
      while (a < b) { var mid = (a + b + 1) >> 1; if (tw(t.slice(0, mid) + "…", font) <= maxw) a = mid; else b = mid - 1; }
      return t.slice(0, a).replace(/\s+$/, "") + "…";
    }
    function esc(t) { return String(t).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;"); }
    function drawH() {
      var dot = cfg.type === "dot", stk = !!cfg.stacked && !dot, W = plot.clientWidth || 600, narrow = W < 520, GAP = 2;
      var keys = cfg.y.filter(function (k) { return !off[k] && rows.some(function (r) { return typeof r[k] === "number"; }); });
      if (cfg.color_by) keys = keys.slice(0, 1);
      var vis = [];
      rows.forEach(function (r, i) { if (!(cfg.color_by && off[r[cfg.color_by]])) vis.push(i); });
      var vals = [];
      var total = function (r) { var t = 0; keys.forEach(function (k) { t += +r[k] || 0; }); return t; };
      if (stk) vis.forEach(function (i) { vals.push(total(rows[i])); });
      else vis.forEach(function (i) { keys.forEach(function (k) { if (typeof rows[i][k] === "number") vals.push(rows[i][k]); }); });
      if (!vals.length) vals = [1, 10];
      var n = dot || stk ? 1 : Math.max(keys.length, 1), bh = n === 1 ? 20 : n === 2 ? 16 : 14;
      var stack = dot ? 14 : n * bh + (n - 1) * GAP, labH = narrow ? 19 : 0, pitch = labH + stack + (narrow ? 16 : 18);
      var catFont = (narrow ? "13px" : "13.5px"), valFont = "600 12.5px";
      var cats = vis.map(function (i) { return String(rows[i][cfg.x]); });
      var lw = 0;
      if (!narrow) { cats.forEach(function (t) { lw = Math.max(lw, tw(t, catFont)); }); lw = Math.min(Math.ceil(lw), Math.floor(W * 0.36), 240); }
      var ends = vis.map(function (i) { var out = []; if (stk) return [fv(total(rows[i]))]; (dot ? keys.slice(-1) : keys).forEach(function (k) { if (typeof rows[i][k] === "number") out.push(fv(rows[i][k])); }); return out; });
      var ew = 0;
      if (cfg.labels_at_end) ends.forEach(function (o) { o.forEach(function (t) { ew = Math.max(ew, tw(t, valFont)); }); });
      var m = { l: narrow ? 14 : lw + 16, r: Math.max(22, cfg.labels_at_end ? Math.ceil(ew) + 14 : 0), t: cfg.rule ? 26 : 10, b: cfg.xlabel ? 52 : 30 };
      var ph = vis.length * pitch, H = m.t + ph + m.b, pw = W - m.l - m.r;
      var lg = !!cfg.log, lo, hi, major, minor = [];
      if (lg) { var rg = logRange(vals, cfg); lo = rg[0]; hi = rg[1]; var lt = logTicks(lo, hi); major = lt.major; minor = lt.minor; }
      else {
        lo = Math.min.apply(null, vals); hi = Math.max.apply(null, vals);
        if (cfg.min != null) lo = cfg.min; else if (!dot) lo = Math.min(lo, 0);
        if (cfg.max != null) hi = cfg.max;
        major = niceTicks(lo, hi, Math.max(2, Math.floor(pw / 90)));
        if (cfg.min == null) lo = Math.min(lo, major[0]);
        if (cfg.max == null) hi = Math.max(hi, major[major.length - 1]);
      }
      var L = lg ? Math.log10 : function (v) { return v; };
      var xp = function (v) { return m.l + (L(Math.min(Math.max(v, lo), hi)) - L(lo)) / (L(hi) - L(lo)) * pw; };
      var colorOf = function (r, k) { return cfg.color_by ? gcolor(r[cfg.color_by]) : color(k, cfg.y.indexOf(k)); };
      var s = '<svg viewBox="0 0 ' + W + " " + H + '" height="' + H + '">', yb = m.t + ph;
      minor.forEach(function (t) { var x = xp(t); s += '<line class="grid minor" x1="' + x + '" x2="' + x + '" y1="' + m.t + '" y2="' + yb + '"/>'; });
      var every = Math.max(1, Math.ceil(major.length / Math.max(1, Math.floor(pw / 62))));
      major.forEach(function (t, i) {
        var x = xp(t); s += '<line class="grid" x1="' + x + '" x2="' + x + '" y1="' + m.t + '" y2="' + yb + '"/>';
        if (i % every === 0) s += '<text x="' + x + '" y="' + (yb + 18) + '" text-anchor="middle">' + esc(ft(t)) + "</text>";
      });
      if (!dot) s += '<line class="ax" x1="' + xp(lo) + '" x2="' + xp(lo) + '" y1="' + m.t + '" y2="' + yb + '"/>';
      if (cfg.xlabel) s += '<text x="' + (m.l + pw / 2) + '" y="' + (H - 8) + '" text-anchor="middle">' + esc(cfg.xlabel) + "</text>";
      vis.forEach(function (ri, vi) {
        var r = rows[ri], top = m.t + vi * pitch, mt = top + (narrow ? 8 : 9) + labH, full = cats[vi];
        var shown = trunc(full, narrow ? W - 24 : lw, catFont);
        var aria = full + ": " + keys.map(function (k) { return label(k) + " " + fv(r[k]); }).join(", ") + (stk ? ", total " + fv(total(r)) : "");
        s += '<g class="row" data-i="' + vi + '">';
        s += '<text class="cat" x="' + (narrow ? m.l : 0) + '" y="' + (narrow ? top + 15 : mt + stack / 2 + 4.5) + '">' + esc(shown) + "</text>";
        if (dot) {
          var vs = keys.filter(function (k) { return typeof r[k] === "number"; }), cy = mt + 7;
          if (vs.length > 1) {
            var xs2 = vs.map(function (k) { return xp(r[k]); });
            s += '<line class="seg" x1="' + Math.min.apply(null, xs2) + '" x2="' + Math.max.apply(null, xs2) + '" y1="' + cy + '" y2="' + cy + '"/>';
          }
          vs.forEach(function (k) { s += '<circle class="pt" cx="' + xp(r[k]) + '" cy="' + cy + '" r="6" fill="' + colorOf(r, k) + '"/>'; });
          if (cfg.labels_at_end && vs.length) {
            var lk = vs[vs.length - 1], rx = Math.max.apply(null, vs.map(function (k) { return xp(r[k]); }));
            s += '<text class="val" x="' + (rx + 12) + '" y="' + (cy + 4.2) + '">' + esc(fv(r[lk])) + "</text>";
          }
        } else if (stk) {
          var base = 0, lastK = null, y = mt, SG = 2;
          keys.forEach(function (k) { if ((+r[k] || 0) > 0) lastK = k; });
          keys.forEach(function (k) {
            var v = +r[k] || 0; if (v <= 0) return;
            var a = xp(base) + (base > 0 ? SG / 2 : 0), b = xp(base + v) - (k === lastK ? 0 : SG / 2); base += v;
            b = Math.max(b, a + 1);
            if (k === lastK) {
              var rr = Math.min(4, bh / 2, (b - a) / 2);
              s += '<path class="bar" fill="' + color(k, cfg.y.indexOf(k)) + '" d="M' + a + "," + y + "H" + (b - rr) + "A" + rr + "," + rr + " 0 0 1 " + b + "," + (y + rr) + "V" + (y + bh - rr) + "A" + rr + "," + rr + " 0 0 1 " + (b - rr) + "," + (y + bh) + "H" + a + 'Z"/>';
            } else s += '<rect class="bar" fill="' + color(k, cfg.y.indexOf(k)) + '" x="' + a + '" y="' + y + '" width="' + (b - a) + '" height="' + bh + '"/>';
          });
          if (cfg.labels_at_end) s += '<text class="val" x="' + (xp(base) + 7) + '" y="' + (y + bh / 2 + 4.2) + '">' + esc(fv(base)) + "</text>";
        } else keys.forEach(function (k, j) {
          var v = r[k]; if (typeof v !== "number") return;
          var y = mt + j * (bh + GAP), x0 = xp(lo), x1 = Math.max(xp(v), x0 + 2), rr = Math.min(4, bh / 2, (x1 - x0) / 2);
          s += '<path class="bar" fill="' + colorOf(r, k) + '" d="M' + x0 + "," + y + "H" + (x1 - rr) + "A" + rr + "," + rr + " 0 0 1 " + x1 + "," + (y + rr) + "V" + (y + bh - rr) + "A" + rr + "," + rr + " 0 0 1 " + (x1 - rr) + "," + (y + bh) + "H" + x0 + 'Z"/>';
          if (cfg.labels_at_end) s += '<text class="val" x="' + (x1 + 7) + '" y="' + (y + bh / 2 + 4.2) + '">' + esc(fv(v)) + "</text>";
        });
        s += '<rect class="rowhit" x="0" y="' + top + '" width="' + W + '" height="' + pitch + '" tabindex="0" role="img" aria-label="' + esc(aria) + '"/></g>';
      });
      if (cfg.rule) {
        var rx2 = xp(cfg.rule.value), rl = cfg.rule.label || "", rw = rl ? tw(rl, "12px") : 0, endA = rx2 + 6 + rw > W - 4;
        s += '<line class="rule" x1="' + rx2 + '" x2="' + rx2 + '" y1="' + (m.t - 8) + '" y2="' + yb + '"/>';
        if (rl) s += '<text class="rulelbl" x="' + (endA ? rx2 - 6 : rx2 + 6) + '" y="' + (m.t - 6) + '" text-anchor="' + (endA ? "end" : "start") + '">' + esc(rl) + "</text>";
      }
      s += "</svg>";
      plot.innerHTML = s;
      var svg = $("svg", plot);
      function show(vi, cx) {
        var r = rows[vis[vi]];
        if (stk) {
          var cum = 0, hk = null;
          if (cx != null) keys.forEach(function (k) { var v = +r[k] || 0; if (v > 0 && hk === null && cx <= xp(cum + v) + 1) hk = k; if (v > 0) cum += v; });
          svg.classList.add("has-hover");
          $$(".row", svg).forEach(function (g, i) { g.classList.toggle("is-hover", i === vi); });
          tip.innerHTML = "<b>" + esc(cats[vi]) + "</b>" + keys.map(function (k) {
            var t = '<i style="background:' + color(k, cfg.y.indexOf(k)) + '"></i>' + esc(label(k)) + ": " + esc(fv(+r[k] || 0));
            return "<br>" + (k === hk ? "<b>" + t + "</b>" : t);
          }).join("") + "<br>Total: " + esc(fv(total(r)));
          tip.hidden = false;
          var hf = tip.offsetWidth / 2, xx = cx == null ? m.l + pw / 2 : cx;
          tip.style.left = Math.min(Math.max(xx, hf + 2), Math.max(hf + 2, W - hf - 2)) + "px";
          tip.style.top = (plot.offsetTop + m.t + vi * pitch + 4) + "px";
          return;
        }
        svg.classList.add("has-hover");
        $$(".row", svg).forEach(function (g, i) { g.classList.toggle("is-hover", i === vi); });
        var head = "<b>" + esc(cats[vi]) + "</b>";
        if (cfg.color_by) head += '<br><i style="background:' + gcolor(r[cfg.color_by]) + '"></i>' + esc(r[cfg.color_by]);
        tip.innerHTML = head + keys.map(function (k) {
          return "<br>" + (cfg.color_by ? "" : '<i style="background:' + color(k, cfg.y.indexOf(k)) + '"></i>') + esc(label(k)) + ": " + esc(fv(r[k]));
        }).join("");
        tip.hidden = false;
        var half = tip.offsetWidth / 2, x = cx == null ? m.l + pw / 2 : cx;
        tip.style.left = Math.min(Math.max(x, half + 2), Math.max(half + 2, W - half - 2)) + "px";
        tip.style.top = (plot.offsetTop + m.t + vi * pitch + 4) + "px";
      }
      function hide() { svg.classList.remove("has-hover"); $$(".row", svg).forEach(function (g) { g.classList.remove("is-hover"); }); tip.hidden = true; }
      $$(".rowhit", svg).forEach(function (h, vi) {
        var move = function (e) { show(vi, e.clientX - plot.getBoundingClientRect().left); };
        h.addEventListener("pointerenter", move); h.addEventListener("pointermove", move);
        h.addEventListener("pointerleave", hide);
        h.addEventListener("focus", function () { show(vi, null); }); h.addEventListener("blur", hide);
      });
    }
    el._draw = draw;
    el._mark = function (i) { marker = i; draw(); };
    draw();
    if ("ResizeObserver" in window) { var w0 = plot.clientWidth; new ResizeObserver(function () { if (Math.abs(plot.clientWidth - w0) > 4) { w0 = plot.clientWidth; draw(); } }).observe(plot); }
  }
  $$(".tf-chart").forEach(function (el) { try { chart(el); } catch (e) { console.error(e); } });

  /* ---------------------------------------------------------------- sortable tables (tftable with sortable: true) */
  $$(".tf-table.is-sortable").forEach(function (fig) {
    var table = $("table", fig), body = $("tbody", table), ths = $$("th", table);
    ths.forEach(function (th, c) {
      var b = $(".th-sort", th); if (!b) return;
      b.addEventListener("click", function () {
        var cur = th.getAttribute("aria-sort"), next = cur === "ascending" ? "descending" : cur === "descending" ? "none" : "ascending";
        ths.forEach(function (o) { if (o.hasAttribute("aria-sort")) o.setAttribute("aria-sort", "none"); });
        th.setAttribute("aria-sort", next);
        var trs = $$("tr", body), val = function (tr) { return tr.children[c].getAttribute("data-v") || ""; };
        trs.sort(function (a, z) {
          if (next === "none") return +a.getAttribute("data-i") - +z.getAttribute("data-i");
          var x = val(a), y = val(z), nx = parseFloat(x), ny = parseFloat(y), d;
          if (!isNaN(nx) && !isNaN(ny) && /^-?[\d.]/.test(x) && /^-?[\d.]/.test(y)) d = nx - ny; else d = x.localeCompare(y, undefined, { numeric: true });
          return next === "ascending" ? d : -d;
        });
        trs.forEach(function (tr) { body.appendChild(tr); });
      });
    });
  });

  /* ---------------------------------------------------------------- sliders (frames, with a chart and numbers) */
  $$(".tf-slider").forEach(function (fig) {
    var frames = JSON.parse(fig.getAttribute("data-frames"));
    var range = $("input[type=range]", fig), out = $(".sl-value", fig), cap = $(".sl-caption", fig), stats = $(".sl-stats", fig);
    var els = $$(".sl-frame", fig), ch = $(".tf-chart", fig), play = $(".sl-play", fig), timer = null;
    function show(i) {
      els.forEach(function (e, j) {
        e.classList.toggle("is-on", j === i);
        if (Math.abs(j - i) <= 2) { var im = $("img", e); if (im && im.loading === "lazy") im.loading = "eager"; }
      });
      var f = frames[i];
      out.textContent = f.value;
      cap.innerHTML = f.caption || "";
      if (stats) stats.innerHTML = Object.keys(f.stats || {}).map(function (k) { return "<div><dt>" + k + "</dt><dd>" + f.stats[k] + "</dd></div>"; }).join("");
      if (ch && ch._mark) ch._mark(i);
    }
    range.addEventListener("input", function () { show(+range.value); });
    if (play) play.addEventListener("click", function () {
      if (timer) { clearInterval(timer); timer = null; play.classList.remove("is-playing"); return; }
      play.classList.add("is-playing");
      if (+range.value >= frames.length - 1) range.value = 0;
      timer = setInterval(function () {
        var v = +range.value + 1;
        if (v >= frames.length) { clearInterval(timer); timer = null; play.classList.remove("is-playing"); return; }
        range.value = v; show(v);
      }, 700);
    });
    show(+range.value);
  });

  /* ---------------------------------------------------------------- before and after */
  $$(".tf-compare").forEach(function (f) {
    var r = $(".cmp-range", f), v = $(".cmp-view", f);
    r.addEventListener("input", function () { v.style.setProperty("--pos", r.value + "%"); });
  });

  /* ---------------------------------------------------------------- filters */
  var tf = $(".tut-filter");
  if (tf) $$("button", tf).forEach(function (b) {
    b.addEventListener("click", function () {
      $$("button", tf).forEach(function (x) { x.classList.toggle("is-on", x === b); });
      var lv = b.getAttribute("data-level");
      $$(".tut-card, .tut-series li").forEach(function (c) {
        var l = c.getAttribute("data-level") || "";
        if (lv === "all" || l === lv) c.removeAttribute("data-hide"); else c.setAttribute("data-hide", "");
      });
      $$(".tut-series").forEach(function (s) {
        if ($$("li:not([data-hide])", s).length) s.removeAttribute("data-hide"); else s.setAttribute("data-hide", "");
      });
    });
  });
  var of = $(".op-filter input");
  if (of) of.addEventListener("input", function () {
    var q = of.value.trim().toLowerCase();
    $$(".op-group li").forEach(function (li) { if (!q || li.getAttribute("data-q").toLowerCase().indexOf(q) >= 0) li.removeAttribute("data-hide"); else li.setAttribute("data-hide", ""); });
    $$(".op-group").forEach(function (g) { if ($$("li:not([data-hide])", g).length) g.removeAttribute("data-hide"); else g.setAttribute("data-hide", ""); });
  });

  /* ---------------------------------------------------------------- the visitor's system */
  var ua = navigator.userAgent, plat = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || "";
  var os = /win/i.test(plat) || /Windows/.test(ua) ? "windows" : /mac/i.test(plat) || /Mac OS X/.test(ua) ? "macos" : /linux/i.test(plat) && !/Android/.test(ua) ? "linux" : null;
  var names = { windows: "Windows", macos: "macOS", linux: "Linux" };
  if (os) {
    $$('.os-card[data-os="' + os + '"], .dl-os[data-os="' + os + '"]').forEach(function (c) { c.classList.add("is-mine"); });
    $$(".dl-auto").forEach(function (a) { a.textContent = a.getAttribute("data-os-label").replace("{os}", names[os]); a.href = a.href.replace(/#.*$/, "") + "#" + os; });
    var det = $(".dl-detected");
    if (det) { det.hidden = false; $(".dl-os-name", det).textContent = names[os]; }
  }
  $$(".dl-file.is-pending").forEach(function (a) { a.addEventListener("click", function (e) { e.preventDefault(); }); });
})();
