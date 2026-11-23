---
title: Format comparison
description: The same photos in JPEG, WebP, AVIF, JPEG XL, HEIC and PNG at several quality levels, shown by your own browser and zoomed in, with real file sizes and scores.
permalink: /compare/
wide: true
---
{%- assign demo = site.data.format_demo -%}
{%- assign strip_photo = demo.photos | where: "id", "portrait" | first | default: demo.photos[0] -%}

# Format comparison

<p class="applies">Applies to {% include pills.html on="both" %}</p>

<p class="lead">The same photos, compressed by ImageCompress into each format at four quality levels. Every picture on this page is the real compressed file, drawn by your own browser.</p>

Each example is a {{ demo.crop.w }}×{{ demo.crop.h }} detail cut from a larger photo, so you can look at single pixels. The sizes are for that cut; the tables at the end also give the size of the whole photo. The score is [SSIMULACRA2]({{ site.baseurl }}/formats/#how-visually-lossless-and-smaller-work), a measure of how close a result looks to the original: 90 and up is very hard to tell apart, 70 is a small loss that is hard to spot, and below 50 the damage is easy to see.

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

## All results

Sizes are for the {{ demo.crop.w }}×{{ demo.crop.h }} cut, with the whole photo underneath. PNG is lossless, so it is the same at every level. **View** opens a result in the comparison above, next to the original.

{% for p in demo.photos %}
### {{ p.title }}

<p class="credit">{{ p.credit }}. Whole photo {{ p.full.w }}×{{ p.full.h }}, {{ p.full.kb }} KB. The cut, uncompressed as PNG: {{ p.original.kb }} KB.</p>

<div class="table-scroll">
<table class="demo-table">
  <thead><tr><th>Format</th>{% for l in demo.levels %}<th>{{ l.name }}</th>{% endfor %}</tr></thead>
  <tbody>
  {%- assign pi = forloop.index0 %}
  {%- for f in demo.formats %}
    {%- assign has = p.results | where: "format", f.id %}{% if has.size == 0 %}{% continue %}{% endif %}
    <tr><th scope="row">{{ f.name }}</th>
    {%- if f.id == "png" %}
      {%- assign r = has | first %}
      <td colspan="{{ demo.levels.size }}"><a href="{{ site.baseurl }}/assets/demo/{{ r.src }}">{{ r.kb }} KB</a> <span class="muted">(lossless, {{ r.saved }}% smaller than the uncompressed cut)</span> <button type="button" class="view" data-photo="{{ pi }}" data-format="{{ f.id }}" data-level="{{ r.level }}">View</button><br><span class="muted">Whole photo: {{ r.full_kb }} KB</span></td>
    {%- else %}
    {%- for l in demo.levels %}
      {%- assign r = p.results | where: "format", f.id | where: "level", l.id | first %}
      <td>{% if r %}<a href="{{ site.baseurl }}/assets/demo/{{ r.src }}">{{ r.kb }} KB</a> <span class="muted">score {{ r.score }}</span> <button type="button" class="view" data-photo="{{ pi }}" data-format="{{ f.id }}" data-level="{{ l.id }}">View</button><br><span class="muted">{{ r.saved }}% smaller · whole photo {{ r.full_kb }} KB</span>{% else %}<span class="muted">n/a</span>{% endif %}</td>
    {%- endfor %}
    {%- endif %}
    </tr>
  {%- endfor %}
  </tbody>
</table>
</div>
{% endfor %}

The cuts and results were made with `scripts/docs_format_demo.sh` in the [source repository]({{ site.repo }}), using imgcompress {{ demo.version }}. "Visually lossless" and "Smaller" pick the quality for each image by searching for a score of 88 and 70. "Quality 50" and "Quality 25" are fixed settings, there to show where each format starts to break down. Photo credits are on the [Acknowledgements]({{ site.baseurl }}/acknowledgements/#sample-photos) page.

<script>
(function () {
  document.documentElement.classList.add("js");
  var demo = {{ demo | jsonify }};
  var base = "{{ site.baseurl }}/assets/demo/";
  var CW = demo.crop.w, CH = demo.crop.h;
  var levelName = {}; demo.levels.forEach(function (l) { levelName[l.id] = l.name; });
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
    if (!r && which === "png") r = rs[0];  // PNG is lossless: one result for every level
    if (!r) return null;
    var lvl = which === "png" ? "Lossless" : levelName[r.level];
    return { src: r.src, name: r.format_name,
      tag: r.format_name + " · " + lvl + " · " + r.kb + " KB · " + r.saved + "% smaller" + (r.score != null ? " · score " + r.score : "") };
  }
  function load(side, choice) {
    var img = imgs[side], miss = panes[side].querySelector(".missing");
    miss.hidden = true; img.hidden = false;
    el("tag-" + side).textContent = choice ? choice.tag : "";
    if (!choice) { img.hidden = true; img.removeAttribute("src"); miss.textContent = "No result"; miss.hidden = false; return; }
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
    el("cmp-note").textContent = lv
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
  var drag = null;
  viewer.addEventListener("pointerdown", function (e) {
    var onDivider = e.target === divider || divider.contains(e.target);
    drag = { mode: onDivider || st.zoom === "fit" ? "split" : "pan", x: e.clientX, y: e.clientY, cx: st.cx, cy: st.cy };
    viewer.setPointerCapture(e.pointerId);
    move(e);
    e.preventDefault();
  });
  viewer.addEventListener("pointermove", move);
  viewer.addEventListener("pointerup", function () { drag = null; });
  viewer.addEventListener("pointercancel", function () { drag = null; });
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

  // "View" buttons in the tables: show that result against the original.
  document.querySelectorAll("button.view").forEach(function (b) {
    b.addEventListener("click", function () {
      el("cmp-photo").value = b.dataset.photo;
      el("cmp-level").value = b.dataset.level === "lossless" ? "vl" : b.dataset.level;
      el("cmp-a").value = "original";
      el("cmp-b").value = b.dataset.format;
      render();
      el("cmp").scrollIntoView({ behavior: "smooth", block: "start" });
    });
  });

  render();
})();
</script>
