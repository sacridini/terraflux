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
    try { localStorage.setItem("tf-theme", t); } catch (e) {}
    $$(".tf-chart").forEach(function (c) { if (c._draw) c._draw(); });
  });
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
    return fetch(ROOT + "search.json").then(function (r) { return r.json(); }).then(function (d) {
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
  function css(v) { return getComputedStyle(document.documentElement).getPropertyValue(v).trim(); }
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
    var color = function (k, i) { return cfg.colors[k] || PALETTE[i % PALETTE.length]; };
    var label = function (k) { return cfg.labels[k] || k; };
    leg.innerHTML = cfg.y.length > 1 ? cfg.y.map(function (k, i) {
      return '<button type="button" data-k="' + k + '"><i style="background:' + color(k, i) + '"></i>' + label(k) + "</button>";
    }).join("") : "";
    $$("button", leg).forEach(function (b) {
      b.addEventListener("click", function () { var k = b.getAttribute("data-k"); off[k] = !off[k]; b.classList.toggle("is-off", off[k]); draw(); });
    });
    var tip = document.createElement("div"); tip.className = "ch-tip"; tip.hidden = true; el.appendChild(tip);
    function draw() {
      var W = plot.clientWidth || 600, H = cfg.height, m = { l: 54, r: 14, t: 10, b: cfg.xlabel ? 44 : 30 };
      var keys = cfg.y.filter(function (k) { return !off[k]; });
      var vals = [];
      rows.forEach(function (r) {
        if (cfg.stacked) { var s = 0; keys.forEach(function (k) { s += +r[k] || 0; }); vals.push(s); vals.push(0); }
        else keys.forEach(function (k) { if (typeof r[k] === "number") vals.push(r[k]); });
      });
      if (!vals.length) vals = [0, 1];
      var lo = Math.min.apply(null, vals), hi = Math.max.apply(null, vals);
      if (cfg.type === "bar" || cfg.type === "area" || cfg.stacked) lo = Math.min(lo, 0);
      var yt = niceTicks(lo, hi, 5); lo = Math.min(lo, yt[0]); hi = Math.max(hi, yt[yt.length - 1]);
      var xlo, xhi, xpos, bw = 0;
      var lg = numericX && cfg.xlog && xs.every(function (v) { return v > 0; });
      var tx = function (v) { return lg ? Math.log10(v) : v; };
      if (numericX) { xlo = tx(Math.min.apply(null, xs)); xhi = tx(Math.max.apply(null, xs)); if (xlo === xhi) xhi = xlo + 1;
        xpos = function (i) { return m.l + (tx(xs[i]) - xlo) / (xhi - xlo) * (W - m.l - m.r); }; }
      else { bw = (W - m.l - m.r) / rows.length; xpos = function (i) { return m.l + bw * (i + 0.5); }; }
      var ypos = function (v) { return m.t + (1 - (v - lo) / (hi - lo)) * (H - m.t - m.b); };
      var s = '<svg viewBox="0 0 ' + W + " " + H + '" height="' + H + '">';
      yt.forEach(function (t) { var y = ypos(t); s += '<line class="grid" x1="' + m.l + '" x2="' + (W - m.r) + '" y1="' + y + '" y2="' + y + '"/><text x="' + (m.l - 8) + '" y="' + (y + 4) + '" text-anchor="end">' + fmt(t) + (cfg.percent ? "%" : "") + "</text>"; });
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
          keys.forEach(function (k, j) {
            var ci = cfg.y.indexOf(k), v = +r[k] || 0, x, y0, y1;
            if (cfg.stacked) { x = xpos(i) - bw * 0.39; y0 = ypos(base); y1 = ypos(base + v); base += v; }
            else { x = xpos(i) - bw * 0.39 + j * inner; y0 = ypos(Math.max(lo, 0)); y1 = ypos(v); }
            s += '<rect x="' + x + '" y="' + Math.min(y0, y1) + '" width="' + Math.max(1, inner - 2) + '" height="' + Math.abs(y0 - y1) + '" rx="2" fill="' + color(k, ci) + '"' + (hover >= 0 && hover !== i ? ' opacity=".55"' : "") + "/>";
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
          var v = rows[best][k]; return '<i style="background:' + color(k, cfg.y.indexOf(k)) + '"></i>' + label(k) + ": " + fmt(v) + (cfg.percent ? "%" : "");
        }).join("<br>");
        tip.style.left = (xpos(best) / W * r.width) + "px"; tip.style.top = (plot.offsetTop + 20) + "px";
      });
      hit.addEventListener("pointerleave", function () { hover = -1; tip.hidden = true; draw(); });
    }
    el._draw = draw;
    el._mark = function (i) { marker = i; draw(); };
    draw();
    if ("ResizeObserver" in window) { var w0 = plot.clientWidth; new ResizeObserver(function () { if (Math.abs(plot.clientWidth - w0) > 4) { w0 = plot.clientWidth; draw(); } }).observe(plot); }
  }
  $$(".tf-chart").forEach(function (el) { try { chart(el); } catch (e) { console.error(e); } });

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
