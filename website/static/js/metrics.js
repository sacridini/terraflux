/* The metrics index: filters the table of every metric by words, family, theme and level (the controls start hidden: without
   JavaScript the page is the plain table). The state is kept in the address (?q=&family=&theme=&level=), so a filtered list
   can be linked. */
(function () {
  "use strict";
  var root = document.querySelector(".mc-index");
  if (!root) return;
  var bar = root.querySelector(".mc-filters");
  var rows = Array.prototype.slice.call(root.querySelectorAll(".mc-table tbody tr"));
  var count = root.querySelector(".mc-count");
  var none = root.querySelector(".mc-none");
  var controls = {};
  Array.prototype.forEach.call(bar.querySelectorAll("[data-filter]"), function (el) { controls[el.getAttribute("data-filter")] = el; });
  var keys = ["q", "family", "theme", "level"];

  function fromAddress() {
    var p = new URLSearchParams(location.search);
    keys.forEach(function (k) { if (controls[k] && p.has(k)) controls[k].value = p.get(k); });
  }
  function toAddress() {
    var p = new URLSearchParams();
    keys.forEach(function (k) { if (controls[k] && controls[k].value) p.set(k, controls[k].value); });
    var q = p.toString();
    history.replaceState(null, "", location.pathname + (q ? "?" + q : "") + location.hash);
  }
  function apply() {
    var words = controls.q.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
    var shown = 0;
    rows.forEach(function (tr) {
      var ok = (!controls.family.value || tr.dataset.family === controls.family.value) &&
        (!controls.theme.value || tr.dataset.theme === controls.theme.value) &&
        (!controls.level.value || tr.dataset.level === controls.level.value);
      if (ok) {
        var hay = tr.dataset.s;
        for (var i = 0; i < words.length && ok; i++) ok = hay.indexOf(words[i]) >= 0;
      }
      tr.hidden = !ok;
      if (ok) shown++;
    });
    count.textContent = shown === rows.length ? rows.length + " metrics" : shown + " of " + rows.length + " metrics";
    none.hidden = shown > 0;
    toAddress();
  }

  fromAddress();
  bar.hidden = false;
  keys.forEach(function (k) {
    if (!controls[k]) return;
    controls[k].addEventListener(k === "q" ? "input" : "change", apply);
  });
  apply();
})();
