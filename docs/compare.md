---
title: Format comparison
description: The same photos in JPEG, WebP, AVIF, JPEG XL and HEIC at several quality levels, shown by your own browser, with real file sizes and scores.
permalink: /compare/
wide: true
---
{%- assign demo = site.data.format_demo -%}

# Format comparison

<p class="applies">Applies to {% include pills.html on="both" %}</p>

<p class="lead">The same photos, compressed by ImageCompress into each format at four quality levels. Every picture on this page is the real compressed file, drawn by your own browser.</p>

Each example is a {{ demo.crop.w }}×{{ demo.crop.h }} detail cut from a larger photo, shown at its actual pixels so you can see what each format does to fine detail. The sizes under each image are for that crop. The table at the end also lists the sizes for the whole photo. The score is [SSIMULACRA2]({{ site.baseurl }}/formats/#how-visually-lossless-and-smaller-work), a measure of how close the result looks to the original: 90 and up is very hard to tell apart, 70 is a small loss that is hard to spot, and below 50 the damage is easy to see.

<p class="support" id="support">Checking which formats this browser can show…</p>

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

If a format does not show here, your browser cannot open it yet. The file is still fine: [Which apps open which format]({{ site.baseurl }}/formats/#which-apps-open-which-format) lists where it does work.

## Compare side by side

<div class="cmp" id="cmp">
  <div class="cmp-controls">
    <label>Photo
      <select id="cmp-photo">
        {%- for p in demo.photos %}<option value="{{ forloop.index0 }}">{{ p.title }}</option>{% endfor %}
      </select></label>
    <label>Quality
      <select id="cmp-level">
        {%- for l in demo.levels %}<option value="{{ l.id }}">{{ l.name }}</option>{% endfor %}
      </select></label>
    <label>Left
      <select id="cmp-a"><option value="original">Original</option>
        {%- for f in demo.formats %}<option value="{{ f.id }}">{{ f.name }}</option>{% endfor %}
      </select></label>
    <label>Right
      <select id="cmp-b"><option value="original">Original</option>
        {%- for f in demo.formats %}<option value="{{ f.id }}"{% if f.id == "avif" %} selected{% endif %}>{{ f.name }}</option>{% endfor %}
      </select></label>
  </div>
  <div class="cmp-pair">
    <figure class="cmp-side" id="cmp-side-a"></figure>
    <figure class="cmp-side" id="cmp-side-b"></figure>
  </div>
</div>
<noscript><p>The side-by-side view needs JavaScript. Every result is also listed below.</p></noscript>

## All results

{% for p in demo.photos %}
### {{ p.title }}

<p class="credit">{{ p.credit }}. Whole photo {{ p.full.w }}×{{ p.full.h }}, original {{ p.full.kb }} KB.</p>

<div class="demo-grid">
  <figure class="demo-cell">
    <div class="demo-img" style="aspect-ratio: {{ demo.crop.w }} / {{ demo.crop.h }}"><img src="{{ site.baseurl }}/assets/demo/{{ p.original.src }}" width="{{ demo.crop.w }}" height="{{ demo.crop.h }}" alt="{{ p.title }}, original" loading="lazy" decoding="async" data-format="Original"></div>
    <figcaption><b>Original</b> · {{ p.original.kb }} KB</figcaption>
  </figure>
  {%- for r in p.results %}{% if r.level == "vl" %}
  <figure class="demo-cell">
    <div class="demo-img" style="aspect-ratio: {{ demo.crop.w }} / {{ demo.crop.h }}"><img src="{{ site.baseurl }}/assets/demo/{{ r.src }}" width="{{ demo.crop.w }}" height="{{ demo.crop.h }}" alt="{{ p.title }}, {{ r.format_name }}, Visually lossless" loading="lazy" decoding="async" data-format="{{ r.format_name }}"></div>
    <figcaption><b>{{ r.format_name }}</b> · {{ r.kb }} KB · {{ r.saved }}% smaller · score {{ r.score }}</figcaption>
  </figure>
  {%- endif %}{% endfor %}
</div>

<div class="table-scroll">
<table class="demo-table">
  <thead><tr><th>Format</th>{% for l in demo.levels %}<th>{{ l.name }}</th>{% endfor %}</tr></thead>
  <tbody>
  {%- for f in demo.formats %}
    {%- assign has = p.results | where: "format", f.id %}{% if has.size == 0 %}{% continue %}{% endif %}
    <tr><th scope="row">{{ f.name }}</th>
    {%- for l in demo.levels %}
      {%- assign r = p.results | where: "format", f.id | where: "level", l.id | first %}
      <td>{% if r %}<a href="{{ site.baseurl }}/assets/demo/{{ r.src }}">{{ r.kb }} KB</a> <span class="muted">({{ r.saved }}% smaller, score {{ r.score }})</span><br><span class="muted">Whole photo: {{ r.full_kb }} KB</span>{% else %}<span class="muted">n/a</span>{% endif %}</td>
    {%- endfor %}
    </tr>
  {%- endfor %}
  </tbody>
</table>
</div>
{% endfor %}

The crops and results were made with `scripts/docs_format_demo.sh` in the [source repository]({{ site.repo }}), using imgcompress {{ demo.version }}. "Visually lossless" and "Smaller" pick the quality for each image by searching for a score of 88 and 70. "Quality 50" and "Quality 25" are fixed settings, shown so you can see where each format starts to break down. Photo credits are on the [Acknowledgements]({{ site.baseurl }}/acknowledgements/#sample-photos) page.

<script>
(function () {
  var demo = {{ demo | jsonify }};
  var base = "{{ site.baseurl }}/assets/demo/";

  // Swap any image the browser can't decode for a same-sized note naming the format.
  function unsupported(img) {
    var box = document.createElement("div");
    box.className = "demo-missing";
    box.textContent = "Your browser can't display " + (img.dataset.format || "this format");
    img.replaceWith(box);
  }
  function watch(img) {
    if (img.complete && img.naturalWidth === 0 && img.getAttribute("src")) unsupported(img);
    else img.addEventListener("error", function () { unsupported(img); }, { once: true });
  }
  document.querySelectorAll(".demo-img img").forEach(watch);

  // One line saying which formats this browser draws, tested with each format's smallest file.
  var support = document.getElementById("support");
  var pending = demo.formats.length, yes = [], no = [];
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
      (no.length ? ". It can't show " + no.join(" or ") + "." : ". That's all of them.");
  }

  // Side-by-side view.
  var el = function (id) { return document.getElementById(id); };
  function side(fig, photo, level, which) {
    fig.innerHTML = "";
    var w = demo.crop.w, h = demo.crop.h, src, cap, name;
    if (which === "original") {
      src = photo.original.src; name = "Original"; cap = "<b>Original</b> · " + photo.original.kb + " KB";
    } else {
      var r = photo.results.filter(function (x) { return x.format === which && x.level === level; })[0];
      if (!r) { fig.innerHTML = '<div class="demo-missing">No result</div>'; return; }
      src = r.src; name = r.format_name;
      cap = "<b>" + r.format_name + "</b> · " + r.kb + " KB · " + r.saved + "% smaller · score " + r.score;
    }
    var wrap = document.createElement("div");
    wrap.className = "demo-img";
    wrap.style.aspectRatio = w + " / " + h;
    var img = new Image(w, h);
    img.alt = photo.title + ", " + name;
    img.dataset.format = name;
    img.decoding = "async";
    img.addEventListener("error", function () { unsupported(img); }, { once: true });
    img.src = base + src;
    wrap.appendChild(img);
    var fc = document.createElement("figcaption");
    fc.innerHTML = cap;
    fig.appendChild(wrap);
    fig.appendChild(fc);
  }
  function render() {
    var photo = demo.photos[+el("cmp-photo").value], level = el("cmp-level").value;
    side(el("cmp-side-a"), photo, level, el("cmp-a").value);
    side(el("cmp-side-b"), photo, level, el("cmp-b").value);
  }
  ["cmp-photo", "cmp-level", "cmp-a", "cmp-b"].forEach(function (id) { el(id).addEventListener("change", render); });
  render();
})();
</script>
