---
title: Format comparison
description: The same photos in JPEG, WebP, AVIF, JPEG XL, HEIC and PNG, lossless and at several quality levels, shown by your own browser and zoomed in, with real file sizes and scores.
permalink: /compare/
wide: true
---
{%- assign demo = site.data.format_demo -%}
{%- assign strip_photo = demo.photos | where: "id", "portrait" | first | default: demo.photos[0] -%}

# Format comparison

<p class="applies">Applies to {% include pills.html on="both" %}</p>

<p class="lead">The same photos, compressed by ImageCompress into each format at four quality levels, and losslessly where the format can. Every picture on this page is the real compressed file, drawn by your own browser.</p>

Each example is a {{ demo.crop.w }}×{{ demo.crop.h }} detail cut from a larger photo, so you can look at single pixels. The sizes are for that cut; the chart at the end also gives the size of the whole photo. The score is [SSIMULACRA2]({{ site.baseurl }}/formats/#how-visually-lossless-and-smaller-work), a measure of how close a result looks to the original: 90 and up is very hard to tell apart, 70 is a small loss that is hard to spot, and below 50 the damage is easy to see.

<p class="support" id="support">Checking which formats this browser can show…</p>

## Compare side by side

Drag the line across the picture to compare the two sides. Zoom in to see single pixels, and drag the picture to move around. It starts at **Quality 25**, where the damage is easy to see. Switch to **Visually lossless** and the two sides become very hard to tell apart, even zoomed in: that is the point of that setting.

<div class="cmp" id="cmp">
  <div class="cmp-controls">
    <label>Photo
      <select id="cmp-photo">
        {%- for p in demo.photos %}<option value="{{ forloop.index0 }}"{% if p.id == strip_photo.id %} selected{% endif %}>{{ p.title }}</option>{% endfor %}
      </select></label>
    <label>Quality
      <select id="cmp-level">
        {%- for l in demo.levels %}<option value="{{ l.id }}"{% if l.id == "q25" %} selected{% endif %}>{{ l.name }}</option>{% endfor %}
      </select></label>
    <label>Left
      <select id="cmp-a"><option value="original" selected>Original</option>
        {%- for f in demo.formats %}<option value="{{ f.id }}">{{ f.name }}</option>{% endfor %}
      </select></label>
    <label>Right
      <select id="cmp-b"><option value="original">Original</option>
        {%- for f in demo.formats %}<option value="{{ f.id }}"{% if f.id == "jpeg" %} selected{% endif %}>{{ f.name }}</option>{% endfor %}
      </select></label>
    <div class="zoom" role="group" aria-label="Zoom">
      <span>Zoom</span>
      <button type="button" data-zoom="fit" aria-pressed="true">Fit</button>
      <button type="button" data-zoom="4" aria-pressed="false">4×</button>
      <button type="button" data-zoom="8" aria-pressed="false">8×</button>
    </div>
  </div>
  <div class="viewer" id="viewer">
    <div class="pane pane-a"><img id="img-a" alt="" draggable="false"><div class="missing" hidden></div></div>
    <div class="pane pane-b"><img id="img-b" alt="" draggable="false"><div class="missing" hidden></div></div>
    <div class="tag tag-a" id="tag-a"></div>
    <div class="tag tag-b" id="tag-b"></div>
    <div class="divider" id="divider" role="slider" tabindex="0" aria-label="Comparison divider" aria-valuemin="0" aria-valuemax="100" aria-valuenow="50"><span class="knob"></span></div>
  </div>
  <p class="cmp-note" id="cmp-note"></p>
</div>
<noscript><p>The comparison needs JavaScript. Every result is also listed in the tables below, linked to its file.</p></noscript>

## Where each format breaks down

The same small patch of *{{ strip_photo.title }}* at **Quality 25**, enlarged 3× so each pixel is easy to see. At this setting every format has to throw away a lot, and each fails in its own way.

<div class="strip">
  <figure class="strip-cell">
    <div class="zoomtile"><img src="{{ site.baseurl }}/assets/demo/{{ strip_photo.original.src }}" alt="Original patch" data-format="Original" loading="lazy" decoding="async"></div>
    <figcaption><b>Original</b> · {{ strip_photo.original.kb }} KB</figcaption>
  </figure>
  {%- for f in demo.formats %}
    {%- assign r = strip_photo.results | where: "format", f.id | where: "level", "q25" | first %}
    {%- unless r %}{% continue %}{% endunless %}
  <figure class="strip-cell">
    <div class="zoomtile"><img src="{{ site.baseurl }}/assets/demo/{{ r.src }}" alt="{{ f.name }} at Quality 25" data-format="{{ f.name }}" loading="lazy" decoding="async"></div>
    <figcaption><b>{{ f.name }}</b> · {{ r.kb }} KB · score {{ r.score }}<br><span class="muted">{{ f.breaks }}</span></figcaption>
  </figure>
  {%- endfor %}
</div>

## The formats

<div class="table-scroll">
<table>
  <thead><tr><th>Format</th><th>Good at</th><th>Shows in</th></tr></thead>
  <tbody>
  {%- for f in demo.formats %}
    <tr><th scope="row">{{ f.name }}</th><td>{{ f.good_at }}</td><td>{{ f.shows_in }}</td></tr>
  {%- endfor %}
  </tbody>
</table>
</div>

If a format does not show on this page, your browser cannot open it yet. The file is still fine: [Which apps open which format]({{ site.baseurl }}/formats/#which-apps-open-which-format) lists where it does work.

## Size against quality

Each point is one result for the photo you pick: further right looks closer to the original, higher up is a bigger file. A line joins each format's results, from Quality 25 at the left to Lossless at the top right, so you can see how fast each one gives up quality as it saves space. The size scale is logarithmic: each step up the side is a few times larger. Point at a result to read it, and click it to open it in the comparison above.

<div class="chart" id="chart">
  <div class="chart-controls">
    <div class="seg" role="group" aria-label="Photo" id="chart-photos">
      {%- for p in demo.photos %}<button type="button" data-photo="{{ forloop.index0 }}" aria-pressed="{% if p.id == strip_photo.id %}true{% else %}false{% endif %}">{{ p.title }}</button>{% endfor %}
    </div>
    <div class="legend" role="group" aria-label="Formats shown" id="chart-legend">
      {%- for f in demo.formats %}<button type="button" data-format="{{ f.id }}" aria-pressed="true" class="fmt-{{ f.id }}"><svg aria-hidden="true" width="14" height="14" viewBox="-7 -7 14 14"><path class="mark"/></svg>{{ f.name }}</button>{% endfor %}
    </div>
  </div>
  <div class="chart-plot" id="chart-plot">
    <svg id="chart-svg" role="group" aria-label="File size against quality score"></svg>
    <div class="chart-tip" id="chart-tip" hidden></div>
  </div>
  <p class="chart-note">Click a format above to hide or show it. The dashed lines mark the two automatic settings (Smaller at 70, Visually lossless at 88) and the size of the cut as saved.</p>
</div>
<noscript><p>The chart needs JavaScript. Every result is in the table below.</p></noscript>

<details class="numbers">
<summary>All results as a table</summary>

<p>Sizes are for the {{ demo.crop.w }}×{{ demo.crop.h }} cut, followed by the score. Point at a size for the whole-photo size. <b>View</b> opens a result in the comparison, next to the original.</p>

{% for p in demo.photos %}
<h3 id="table-{{ p.id }}">{{ p.title }}</h3>

<p class="credit">{{ p.credit }}. Whole photo {{ p.full.w }}×{{ p.full.h }}, {{ p.full.kb }} KB. The cut as saved: {{ p.original.kb }} KB.</p>

<div class="table-scroll">
<table class="demo-table">
  <thead><tr><th>Format</th>{% for l in demo.levels %}<th>{{ l.name }}</th>{% endfor %}</tr></thead>
  <tbody>
  {%- assign pi = forloop.index0 %}
  {%- for f in demo.formats %}
    {%- assign has = p.results | where: "format", f.id %}{% if has.size == 0 %}{% continue %}{% endif %}
    <tr><th scope="row">{{ f.name }}</th>
    {%- for l in demo.levels %}
      {%- assign r = has | where: "level", l.id | first %}
      <td>{% if r %}<a href="{{ site.baseurl }}/assets/demo/{{ r.src }}" title="{{ r.saved }}% smaller than the cut · whole photo {{ r.full_kb }} KB">{{ r.kb }} KB</a> <span class="muted">· {{ r.score }}</span> <button type="button" class="view" data-photo="{{ pi }}" data-format="{{ f.id }}" data-level="{{ l.id }}" aria-label="View {{ f.name }}, {{ l.name }}">View</button>{% else %}<span class="muted">–</span>{% endif %}</td>
    {%- endfor %}
    </tr>
  {%- endfor %}
  </tbody>
</table>
</div>
{% endfor %}

<p>A dash means there is no such result: JPEG, AVIF and HEIC have no lossless mode for new pictures in ImageCompress, and PNG is always lossless.</p>
</details>

The cuts and results were made with `scripts/docs_format_demo.sh` in the [source repository]({{ site.repo }}), using imgcompress {{ demo.version }}. All JPEG results use the default encoder, [jpegli]({{ site.baseurl }}/formats/#jpeg-encoders), at the same quality settings; mozjpeg would give somewhat different sizes. "Lossless" keeps every pixel exactly, so it scores 100; PNG, WebP and JPEG XL can do it. The whole photos start as JPEGs, and lossless JPEG XL repacks the JPEG itself, so the original file can be rebuilt bit for bit. "Visually lossless" and "Smaller" pick the quality for each image by searching for a score of 88 and 70. "Quality 50" and "Quality 25" are fixed settings, there to show where each format starts to break down. Photo credits are on the [Acknowledgements]({{ site.baseurl }}/acknowledgements/#sample-photos) page.

<script>
(function () {
  document.documentElement.classList.add("js");
  var demo = {{ demo | jsonify }};
  var base = "{{ site.baseurl }}/assets/demo/";
  var CW = demo.crop.w, CH = demo.crop.h;
  var levelName = {}; demo.levels.forEach(function (l) { levelName[l.id] = l.name; });
  var fmtName = {}; demo.formats.forEach(function (f) { fmtName[f.id] = f.name; });
  var el = function (id) { return document.getElementById(id); };

  // Static images (the strip): swap any image the browser can't decode for a note naming the format.
  function unsupported(img) {
    var box = document.createElement("div");
    box.className = "demo-missing";
    box.textContent = "Your browser can't display " + (img.dataset.format || "this format");
    img.replaceWith(box);
  }
  document.querySelectorAll(".zoomtile img").forEach(function (img) {
    if (img.complete && img.naturalWidth === 0 && img.getAttribute("src")) unsupported(img);
    else img.addEventListener("error", function () { unsupported(img); }, { once: true });
  });

  // Which formats this browser draws, tested with each format's smallest file.
  var support = el("support"), pending = demo.formats.length, yes = [], no = [];
  demo.formats.forEach(function (f) {
    var smallest = null;
    demo.photos.forEach(function (p) { p.results.forEach(function (r) {
      if (r.format === f.id && (!smallest || r.bytes < smallest.bytes)) smallest = r;
    }); });
    var probe = new Image();
    probe.onload = function () { (probe.naturalWidth > 0 ? yes : no).push(f.name); done(); };
    probe.onerror = function () { no.push(f.name); done(); };
    probe.src = base + smallest.src;
  });
  function done() {
    if (--pending) return;
    support.textContent = "This browser shows " + (yes.join(", ") || "none of them") +
      (no.length ? ". It can't show " + no.join(" or ") + ", so those appear as a note instead of a picture." : ". That's all of them.");
  }

  // The comparison viewer: two stacked copies of the cut, the right one clipped at the divider.
  var viewer = el("viewer"), divider = el("divider");
  var panes = { a: viewer.querySelector(".pane-a"), b: viewer.querySelector(".pane-b") };
  var imgs = { a: el("img-a"), b: el("img-b") };
  var st = { zoom: "fit", split: 0.5, cx: CW / 2, cy: CH / 2 };

  function pick(photo, which, level) {
    if (which === "original") return { src: photo.original.src, name: "Original", tag: "Original · " + photo.original.kb + " KB" };
    var rs = photo.results.filter(function (x) { return x.format === which; });
    var r = rs.filter(function (x) { return x.level === level; })[0];
    if (!r && which === "png") r = rs[0];  // PNG is always lossless: show that result at every level
    if (!r) return { none: (fmtName[which] || which) + (level === "lossless" ? " has no lossless mode here" : ": no result") };
    return { src: r.src, name: r.format_name,
      tag: r.format_name + " · " + levelName[r.level] + " · " + r.kb + " KB · " + r.saved + "% smaller" + (r.score != null ? " · score " + r.score : "") };
  }
  function load(side, choice) {
    var img = imgs[side], miss = panes[side].querySelector(".missing");
    miss.hidden = true; img.hidden = false;
    el("tag-" + side).textContent = choice.tag || "";
    if (choice.none) { img.hidden = true; img.removeAttribute("src"); miss.textContent = choice.none; miss.hidden = false; return; }
    img.onerror = function () { img.hidden = true; miss.textContent = "Your browser can't display " + choice.name; miss.hidden = false; };
    img.alt = choice.tag;
    img.src = base + choice.src;
  }
  function scale(W) { var fit = W / CW; return st.zoom === "fit" ? fit : Math.max(fit, +st.zoom); }
  function layout() {
    var W = viewer.clientWidth, H = W * CH / CW, s = scale(W);
    viewer.style.height = H + "px";
    // Keep the viewing centre inside the picture, then place both images the same way.
    var halfW = W / 2 / s, halfH = H / 2 / s;
    st.cx = Math.min(Math.max(st.cx, halfW), CW - halfW);
    st.cy = Math.min(Math.max(st.cy, halfH), CH - halfH);
    var ox = W / 2 - st.cx * s, oy = H / 2 - st.cy * s;
    ["a", "b"].forEach(function (k) {
      var img = imgs[k];
      img.style.width = CW * s + "px"; img.style.height = CH * s + "px";
      img.style.transform = "translate(" + ox + "px," + oy + "px)";
    });
    var x = st.split * W;
    panes.b.style.clipPath = "inset(0 0 0 " + x + "px)";
    divider.style.left = x + "px";
    divider.setAttribute("aria-valuenow", Math.round(st.split * 100));
    viewer.classList.toggle("zoomed", st.zoom !== "fit");
  }
  function render() {
    var photo = demo.photos[+el("cmp-photo").value], level = el("cmp-level").value;
    load("a", pick(photo, el("cmp-a").value, level));
    load("b", pick(photo, el("cmp-b").value, level));
    var lv = level === "vl" || level === "smaller";
    el("cmp-note").textContent = level === "lossless"
      ? "Lossless keeps every pixel exactly, so the two sides are identical and only the sizes differ. JPEG, AVIF and HEIC have no lossless mode here."
      : lv
      ? levelName[level] + " searches each format for its own quality setting, so the sizes differ while the look stays close."
      : levelName[level] + " is the same setting number for every format. The numbers mean different things in each encoder, so the sizes and the damage differ.";
    layout();
  }
  ["cmp-photo", "cmp-level", "cmp-a", "cmp-b"].forEach(function (id) { el(id).addEventListener("change", render); });

  document.querySelectorAll(".zoom button").forEach(function (b) {
    b.addEventListener("click", function () {
      st.zoom = b.dataset.zoom;
      document.querySelectorAll(".zoom button").forEach(function (o) { o.setAttribute("aria-pressed", String(o === b)); });
      layout();
    });
  });

  // Drag the divider (or, at Fit, anywhere); when zoomed, drag elsewhere to move around.
  // A press within 28px of the line always moves the line, even when zoomed. Dragging never selects anything.
  var drag = null;
  viewer.addEventListener("pointerdown", function (e) {
    if (e.button > 0) return;
    var r = viewer.getBoundingClientRect();
    var near = Math.abs(e.clientX - r.left - st.split * r.width) <= 28;
    drag = { mode: near || st.zoom === "fit" ? "split" : "pan", x: e.clientX, y: e.clientY, cx: st.cx, cy: st.cy };
    viewer.setPointerCapture(e.pointerId);
    document.body.classList.add("dragging");
    var sel = window.getSelection && window.getSelection(); if (sel && sel.removeAllRanges) sel.removeAllRanges();
    if (drag.mode === "split") move(e);
    divider.focus({ preventScroll: true });
    e.preventDefault();
  });
  function stop() { drag = null; document.body.classList.remove("dragging"); }
  viewer.addEventListener("pointermove", move);
  viewer.addEventListener("pointerup", stop);
  viewer.addEventListener("pointercancel", stop);
  viewer.addEventListener("lostpointercapture", stop);
  ["selectstart", "dragstart", "contextmenu"].forEach(function (t) { viewer.addEventListener(t, function (e) { e.preventDefault(); }); });
  function move(e) {
    if (!drag) return;
    var r = viewer.getBoundingClientRect();
    if (drag.mode === "split") {
      st.split = Math.min(Math.max((e.clientX - r.left) / r.width, 0), 1);
    } else {
      var s = scale(r.width);
      st.cx = drag.cx - (e.clientX - drag.x) / s;
      st.cy = drag.cy - (e.clientY - drag.y) / s;
    }
    layout();
  }
  divider.addEventListener("keydown", function (e) {
    var d = e.key === "ArrowLeft" ? -0.05 : e.key === "ArrowRight" ? 0.05 : 0;
    if (!d) return;
    st.split = Math.min(Math.max(st.split + d, 0), 1); layout(); e.preventDefault();
  });
  window.addEventListener("resize", layout);

  // Open one result in the viewer, against the original.
  function show(photo, format, level) {
    el("cmp-photo").value = photo;
    el("cmp-level").value = level;
    el("cmp-a").value = "original";
    el("cmp-b").value = format;
    render();
    el("cmp").scrollIntoView({ behavior: "smooth", block: "start" });
  }
  document.querySelectorAll("button.view").forEach(function (b) {
    b.addEventListener("click", function () { show(b.dataset.photo, b.dataset.format, b.dataset.level); });
  });

  // The size-against-quality chart: one SVG, redrawn on resize, photo change and legend toggles.
  var NS = "http://www.w3.org/2000/svg";
  var shapes = {  // each format has its own marker, so colour is never the only key
    jpeg: "M0,-5A5,5 0 1,1 0,5A5,5 0 1,1 0,-5Z",
    webp: "M-4.5,-4.5H4.5V4.5H-4.5Z",
    avif: "M0,-5.8L5.4,4H-5.4Z",
    jxl: "M0,-6L6,0L0,6L-6,0Z",
    heic: "M0,5.8L5.4,-4H-5.4Z",
    png: "M-1.8,-5.5H1.8V-1.8H5.5V1.8H1.8V5.5H-1.8V1.8H-5.5V-1.8H-1.8Z"
  };
  document.querySelectorAll("#chart-legend path.mark").forEach(function (m) { m.setAttribute("d", shapes[m.closest("button").dataset.format]); });
  var ch = { photo: +(document.querySelector("#chart-photos [aria-pressed=true]") || { dataset: { photo: 0 } }).dataset.photo, hidden: {}, pts: [], hot: null };
  var svg = el("chart-svg"), tip = el("chart-tip"), plot = el("chart-plot");
  function node(tag, attrs, parent) {
    var n = document.createElementNS(NS, tag);
    for (var k in attrs) n.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(n);
    return n;
  }
  function kbText(v) { return (v < 10 ? (Math.round(v * 10) / 10) : Math.round(v)) + " KB"; }
  function drawChart() {
    var photo = demo.photos[ch.photo];
    var W = plot.clientWidth, narrow = W < 560;
    var H = Math.round(Math.min(560, Math.max(320, W * 0.6)));
    var m = { l: narrow ? 46 : 58, r: narrow ? 12 : 104, t: 22, b: 46 };
    svg.setAttribute("viewBox", "0 0 " + W + " " + H);
    svg.setAttribute("width", W); svg.setAttribute("height", H);
    while (svg.firstChild) svg.removeChild(svg.firstChild);
    var rs = photo.results.filter(function (r) { return r.score != null; });
    var minS = Math.min.apply(null, rs.map(function (r) { return r.score; }));
    var x0 = Math.min(60, Math.floor(minS / 10) * 10), x1 = 100;
    var kbs = rs.map(function (r) { return r.bytes / 1000; }).concat([photo.original.bytes / 1000]);
    var y0 = Math.log10(Math.min.apply(null, kbs) * 0.8), y1 = Math.log10(Math.max.apply(null, kbs) * 1.25);
    var X = function (v) { return m.l + (v - x0) / (x1 - x0) * (W - m.l - m.r); };
    var Y = function (kb) { return H - m.b - (Math.log10(kb) - y0) / (y1 - y0) * (H - m.t - m.b); };
    var g = node("g", { "class": "axes" }, svg);
    // Grid and ticks: scores every 10, sizes at 1-2-5 steps.
    for (var v = x0; v <= x1; v += 10) {
      node("line", { x1: X(v), x2: X(v), y1: m.t, y2: H - m.b, "class": "grid" }, g);
      node("text", { x: X(v), y: H - m.b + 18, "class": "tick", "text-anchor": "middle" }, g).textContent = v;
    }
    for (var e = Math.floor(y0); e <= Math.ceil(y1); e++) [1, 2, 5].forEach(function (k) {
      var kb = k * Math.pow(10, e), lk = Math.log10(kb);
      if (lk < y0 || lk > y1) return;
      node("line", { x1: m.l, x2: W - m.r, y1: Y(kb), y2: Y(kb), "class": "grid" }, g);
      node("text", { x: m.l - 8, y: Y(kb) + 4, "class": "tick", "text-anchor": "end" }, g).textContent = kb >= 1 ? kb : kb.toFixed(1);
    });
    node("text", { x: m.l + (W - m.l - m.r) / 2, y: H - 8, "class": "axis-title", "text-anchor": "middle" }, g).textContent = "Quality score (SSIMULACRA2) →";
    node("text", { x: 14, y: m.t + (H - m.t - m.b) / 2, "class": "axis-title", "text-anchor": "middle", transform: "rotate(-90 14 " + (m.t + (H - m.t - m.b) / 2) + ")" }, g).textContent = "File size, KB →";
    // Reference lines.
    [[88, "Visually lossless"], [70, "Smaller"]].forEach(function (ref) {
      if (ref[0] < x0) return;
      node("line", { x1: X(ref[0]), x2: X(ref[0]), y1: m.t, y2: H - m.b, "class": "ref" }, g);
      node("text", { x: narrow ? X(ref[0]) : X(ref[0]) - 4, y: m.t - 8, "class": "ref-label", "text-anchor": narrow ? "middle" : "end" }, g).textContent = narrow ? ref[0] : ref[1] + " " + ref[0];
    });
    var ok = photo.original.bytes / 1000;
    node("line", { x1: m.l, x2: W - m.r, y1: Y(ok), y2: Y(ok), "class": "ref" }, g);
    node("text", { x: m.l + 6, y: Y(ok) - 6, "class": "ref-label" }, g).textContent = "The cut as saved, " + kbText(ok);
    // One line and a set of markers per format.
    ch.pts = [];
    var ends = [];
    demo.formats.forEach(function (f) {
      if (ch.hidden[f.id]) return;
      var mine = rs.filter(function (r) { return r.format === f.id; }).sort(function (a, b) { return a.score - b.score; });
      if (!mine.length) return;
      var s = node("g", { "class": "series fmt-" + f.id }, svg);
      if (mine.length > 1) node("polyline", { points: mine.map(function (r) { return X(r.score) + "," + Y(r.bytes / 1000); }).join(" "), "class": "line" }, s);
      mine.forEach(function (r) {
        var px = X(r.score), py = Y(r.bytes / 1000);
        var label = r.format_name + ", " + levelName[r.level] + ": " + kbText(r.bytes / 1000) + ", score " + r.score;
        var pt = node("g", { "class": "pt", transform: "translate(" + px + "," + py + ")", tabindex: "0", role: "button", "aria-label": label + ". Open in the comparison." }, s);
        node("path", { d: shapes[f.id], "class": "mark" }, pt);
        var rec = { r: r, x: px, y: py, n: pt };
        ch.pts.push(rec);
        pt.addEventListener("focus", function () { hot(rec); });
        pt.addEventListener("blur", function () { hot(null); });
        pt.addEventListener("keydown", function (ev) { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); open(rec); } });
      });
      var last = mine[mine.length - 1];
      ends.push({ f: f, y: Y(last.bytes / 1000), px: X(last.score), py: Y(last.bytes / 1000) });
    });
    // Direct labels in the right margin, at each line's top-right end, nudged apart so they don't overlap.
    if (!narrow) {
      ends.sort(function (a, b) { return a.y - b.y; });
      for (var i = 1; i < ends.length; i++) if (ends[i].y - ends[i - 1].y < 15) ends[i].y = ends[i - 1].y + 15;
      var over = ends.length ? ends[ends.length - 1].y - (H - m.b) : 0;
      if (over > 0) ends.forEach(function (d) { d.y -= over; });
      ends.forEach(function (d) {
        // A faint leader joins a label to its line when the line ends short of the right edge.
        if (W - m.r - d.px > 12 || Math.abs(d.y - d.py) > 6)
          node("polyline", { points: (d.px + 8) + "," + d.py + " " + (W - m.r - 2) + "," + d.py + " " + (W - m.r + 6) + "," + d.y, "class": "leader fmt-" + d.f.id }, svg);
        node("text", { x: W - m.r + 10, y: d.y + 4, "class": "end-label" }, svg).textContent = d.f.name;
      });
    }
  }
  function hot(rec) {
    if (ch.hot) ch.hot.n.classList.remove("hot");
    ch.hot = rec;
    if (!rec) { tip.hidden = true; return; }
    rec.n.classList.add("hot");
    var r = rec.r, photo = demo.photos[ch.photo];
    tip.textContent = "";
    var b = document.createElement("strong"); b.textContent = kbText(r.bytes / 1000); tip.appendChild(b);
    [r.format_name + " · " + levelName[r.level],
     "Score " + r.score + " · " + r.saved + "% smaller than the cut",
     r.full_kb != null ? "Whole photo: " + r.full_kb + " KB (was " + photo.full.kb + " KB)" : ""].forEach(function (t) {
      if (!t) return; var d = document.createElement("div"); d.textContent = t; tip.appendChild(d);
    });
    var c = document.createElement("div"); c.className = "muted"; c.textContent = "Click to compare with the original"; tip.appendChild(c);
    tip.hidden = false;
    var W = plot.clientWidth, tw = tip.offsetWidth, th = tip.offsetHeight;
    var left = rec.x + 14; if (left + tw > W) left = rec.x - 14 - tw;
    var top = rec.y - th - 12; if (top < 0) top = rec.y + 14;
    tip.style.left = Math.max(0, left) + "px"; tip.style.top = top + "px";
  }
  function open(rec) { show(String(ch.photo), rec.r.format, rec.r.level); }
  function nearest(e) {  // the closest point within 24px of the pointer
    var b = svg.getBoundingClientRect(), x = e.clientX - b.left, y = e.clientY - b.top, best = null, bd = 24 * 24;
    ch.pts.forEach(function (p) { var d = (p.x - x) * (p.x - x) + (p.y - y) * (p.y - y); if (d < bd) { bd = d; best = p; } });
    return best;
  }
  svg.addEventListener("pointermove", function (e) { if (e.pointerType === "mouse") { var p = nearest(e); if (p !== ch.hot) hot(p); svg.style.cursor = p ? "pointer" : ""; } });
  svg.addEventListener("pointerleave", function (e) { if (e.pointerType === "mouse") hot(null); });
  svg.addEventListener("click", function (e) {
    var p = nearest(e);
    if (!p) { hot(null); return; }
    // Touch: the first tap shows the details, a second tap on the same point opens it.
    if (e.pointerType && e.pointerType !== "mouse" && ch.hot !== p) { hot(p); return; }
    open(p);
  });
  document.querySelectorAll("#chart-photos button").forEach(function (b) {
    b.addEventListener("click", function () {
      ch.photo = +b.dataset.photo;
      document.querySelectorAll("#chart-photos button").forEach(function (o) { o.setAttribute("aria-pressed", String(o === b)); });
      hot(null); drawChart();
    });
  });
  document.querySelectorAll("#chart-legend button").forEach(function (b) {
    b.addEventListener("click", function () {
      var id = b.dataset.format;
      ch.hidden[id] = !ch.hidden[id];
      b.setAttribute("aria-pressed", String(!ch.hidden[id]));
      hot(null); drawChart();
    });
  });
  window.addEventListener("resize", drawChart);
  drawChart();

  render();
})();
</script>
