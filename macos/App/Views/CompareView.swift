import SwiftUI
import AppKit
import ImageIO
import CoreImage
import CoreImage.CIFilterBuiltins

/// Before/after comparison with zoom down to single pixels.
///
/// A finished file is compared as it is on disk: the original from its Undo copy (or the untouched source when the
/// result went elsewhere) against the result, so the sheet opens at once. Only when neither is available (a file not
/// compressed yet, or its Undo copy expired) is a preview encoded with today's settings by `imgcompress preview`.
struct CompareView: View {
    let item: JobItem
    let options: Options
    @Environment(\.dismiss) private var dismiss
    @Environment(\.displayScale) private var displayScale

    enum Mode: String, CaseIterable, Identifiable {
        case slider = "Slider", sideBySide = "Side by Side", difference = "Difference"
        var id: Self { self }
    }

    @AppStorage("compareMode") private var mode: Mode = .slider
    @State private var before: CGImage?
    @State private var after: CGImage?
    @State private var diff: CGImage?
    @State private var beforeBytes: UInt64?
    @State private var afterBytes: UInt64?
    /// The result shown was encoded just now, not read from disk.
    @State private var previewed = false
    @State private var score: Double?
    @State private var scoring = false
    /// Why there is no score (the result was resized, or the engine could not read a file).
    @State private var scoreNote: String?
    /// Shown only if decoding takes long enough to notice.
    @State private var slowLoad = false
    @State private var error: String?
    @State private var split: CGFloat = 0.5
    /// Screen pixels per image pixel; nil fits the image to the view.
    @State private var zoom: CGFloat?
    /// The image pixel at the middle of the view; nil is the middle of the image.
    @State private var center: CGPoint?
    @State private var panStart: CGPoint?
    @State private var canvasSize: CGSize = .zero

    private static let levels: [CGFloat] = [1, 2, 4, 8, 16]

    private static var startSize: CGSize {
        let screen = NSScreen.main?.visibleFrame.size ?? CGSize(width: 1280, height: 800)
        return CGSize(width: max(760, min(1200, screen.width * 0.85)), height: max(520, min(860, screen.height * 0.85)))
    }

    var body: some View {
        VStack(spacing: 0) {
            controls.padding(.horizontal).padding(.vertical, 8)
            Divider()
            ZStack {
                if before != nil, after != nil {
                    canvas
                } else if let error {
                    Text(error).foregroundStyle(.red).padding()
                } else {
                    ProgressView(previewed ? "Encoding preview…" : "Loading…").opacity(previewed || slowLoad ? 1 : 0)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(nsColor: .underPageBackgroundColor))
            Divider()
            footer.padding(.horizontal).padding(.vertical, 8)
        }
        // A sheet opens at its minimum size, so make that most of the screen (up to 1200 × 860).
        .frame(minWidth: Self.startSize.width, maxWidth: .infinity, minHeight: Self.startSize.height, maxHeight: .infinity)
        .background { shortcuts }
        .task { await load() }
        .task { try? await Task.sleep(for: .milliseconds(400)); slowLoad = true }
    }

    // MARK: Loading

    private func load() async {
        if let (b, a) = Self.onDisk(item) {
            await show(before: b, after: a)
            return
        }
        previewed = true
        do {
            let r = try await Preview.render(file: item.url, options: options)
            await show(before: item.url, after: r.output)
        } catch { self.error = error.localizedDescription }
    }

    private func show(before b: URL, after a: URL) async {
        let (ib, ia) = await Task.detached { (Self.decode(b), Self.decode(a)) }.value
        guard let ib, let ia else { error = "The images could not be displayed."; return }
        before = ib; after = ia
        beforeBytes = Preview.size(b); afterBytes = Preview.size(a)
        diff = await Task.detached { Self.difference(ib, ia) }.value
        scoring = true
        let r = await CLI.exec(["score", b.path, a.path, "--json"])
        scoring = false
        if r.ok, let d = r.text.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: d) as? [String: Any] { score = obj["score"] as? Double }
        else if r.stderr.contains("different sizes") { scoreNote = "No similarity score: the result was resized" }
        else if !r.stderr.contains("unrecognized subcommand") { scoreNote = "No similarity score for this file" }
    }

    /// The original and the result as they are on disk, when both still exist.
    static func onDisk(_ item: JobItem) -> (URL, URL)? {
        guard item.status == .done, !item.restored, !item.isDirectory else { return nil }
        let fm = FileManager.default
        let out = item.output ?? item.path
        guard fm.fileExists(atPath: out) else { return nil }
        let restores = item.undoEntry?.undo.restore ?? []
        if let b = (restores.first { $0.path == item.path } ?? restores.first)?.backup, fm.fileExists(atPath: b) {
            return (URL(fileURLWithPath: b), URL(fileURLWithPath: out))
        }
        if out != item.path, fm.fileExists(atPath: item.path) { return (item.url, URL(fileURLWithPath: out)) }
        return nil
    }

    /// Full-size decode with EXIF orientation applied, so both sides line up pixel for pixel.
    nonisolated static func decode(_ url: URL) -> CGImage? {
        guard let src = CGImageSourceCreateWithURL(url as CFURL, nil),
              let props = CGImageSourceCopyPropertiesAtIndex(src, 0, nil) as? [CFString: Any],
              let w = props[kCGImagePropertyPixelWidth] as? Int, let h = props[kCGImagePropertyPixelHeight] as? Int else { return nil }
        let opts: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true,
                                     kCGImageSourceCreateThumbnailWithTransform: true,
                                     kCGImageSourceThumbnailMaxPixelSize: max(w, h),
                                     kCGImageSourceShouldCacheImmediately: true]
        return CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary)
    }

    /// |before − after| per channel, brightened so small differences show: black means identical.
    nonisolated static let diffGain: CGFloat = 10
    nonisolated static func difference(_ a: CGImage, _ b: CGImage) -> CGImage? {
        let ia = CIImage(cgImage: a)
        var ib = CIImage(cgImage: b)
        if ib.extent.size != ia.extent.size {
            ib = ib.transformed(by: CGAffineTransform(scaleX: ia.extent.width / ib.extent.width, y: ia.extent.height / ib.extent.height))
        }
        let d = CIFilter.differenceBlendMode()
        d.inputImage = ia; d.backgroundImage = ib
        let m = CIFilter.colorMatrix()
        m.inputImage = d.outputImage
        m.rVector = CIVector(x: diffGain, y: 0, z: 0, w: 0)
        m.gVector = CIVector(x: 0, y: diffGain, z: 0, w: 0)
        m.bVector = CIVector(x: 0, y: 0, z: diffGain, w: 0)
        m.aVector = CIVector(x: 0, y: 0, z: 0, w: 0)
        m.biasVector = CIVector(x: 0, y: 0, z: 0, w: 1)
        guard let out = m.outputImage else { return nil }
        // Work on the stored values, not linear light, so the brightness matches the pixel difference.
        let ctx = CIContext(options: [.workingColorSpace: NSNull(), .outputColorSpace: NSNull()])
        return ctx.createCGImage(out, from: ia.extent)
    }

    // MARK: Geometry (view points; image measured in pixels)

    private var imageSize: CGSize { before.map { CGSize(width: $0.width, height: $0.height) } ?? CGSize(width: 1, height: 1) }
    private func pane(_ v: CGSize) -> CGSize { mode == .sideBySide ? CGSize(width: max((v.width - 1) / 2, 1), height: v.height) : v }
    private func fitScale(_ v: CGSize) -> CGFloat { min(v.width / imageSize.width, v.height / imageSize.height) }
    private func scale(_ v: CGSize) -> CGFloat { zoom.map { $0 / displayScale } ?? fitScale(v) }

    private func clamped(_ c: CGPoint?, _ v: CGSize, _ s: CGFloat) -> CGPoint {
        let c = c ?? CGPoint(x: imageSize.width / 2, y: imageSize.height / 2)
        func axis(_ c: CGFloat, _ len: CGFloat, _ view: CGFloat) -> CGFloat {
            let half = view / 2 / s
            return len <= 2 * half ? len / 2 : min(max(c, half), len - half)
        }
        return CGPoint(x: axis(c.x, imageSize.width, v.width), y: axis(c.y, imageSize.height, v.height))
    }

    private func imageRect(_ v: CGSize) -> CGRect {
        let s = scale(v), c = clamped(center, v, s)
        return CGRect(x: v.width / 2 - c.x * s, y: v.height / 2 - c.y * s, width: imageSize.width * s, height: imageSize.height * s)
    }

    private func canPan(_ v: CGSize) -> Bool {
        let r = imageRect(v)
        return r.width > v.width + 0.5 || r.height > v.height + 0.5
    }

    /// Sets the zoom, keeping the image pixel under `anchor` (a point in the pane) where it is.
    private func setZoom(_ z: CGFloat?, anchor: CGPoint? = nil) {
        let v = pane(canvasSize)
        let s0 = scale(v), c0 = clamped(center, v, s0)
        let a = anchor ?? CGPoint(x: v.width / 2, y: v.height / 2)
        let px = CGPoint(x: c0.x + (a.x - v.width / 2) / s0, y: c0.y + (a.y - v.height / 2) / s0)
        zoom = z.map { min(max($0, 0.02), 32) }
        let s1 = scale(v)
        center = clamped(CGPoint(x: px.x - (a.x - v.width / 2) / s1, y: px.y - (a.y - v.height / 2) / s1), v, s1)
    }

    private var currentZoom: CGFloat { zoom ?? fitScale(pane(canvasSize)) * displayScale }

    private func zoomIn() {
        setZoom(Self.levels.first { $0 > currentZoom * 1.01 } ?? Self.levels.last)
    }

    private func zoomOut() {
        let fit = fitScale(pane(canvasSize)) * displayScale
        if let lower = Self.levels.last(where: { $0 < currentZoom * 0.99 }), lower > fit { setZoom(lower) } else { setZoom(nil) }
    }

    // MARK: Canvas

    private var canvas: some View {
        GeometryReader { geo in
            let v = geo.size
            Group {
                switch mode {
                case .slider: sliderView(v)
                case .sideBySide:
                    HStack(spacing: 1) {
                        single(before, pane(v), label: "Original")
                        single(after, pane(v), label: "Compressed")
                    }
                case .difference: single(diff, v, label: "Difference, brightened \(Int(Self.diffGain))×: black is identical")
                }
            }
            .overlay(alignment: .topLeading) { if canPan(pane(v)) { navigator(pane(v)) } }
            .background {
                ScrollCatcher(onScroll: { delta, precise in pan(by: delta, precise: precise) },
                              onMagnify: { m, p in magnify(m, at: p) },
                              onZoomScroll: { dy, p in magnify(dy * 0.01, at: p) })
            }
            .onAppear { canvasSize = v }
            .onChange(of: v) { canvasSize = $1 }
        }
    }

    private func layer(_ img: CGImage?, _ v: CGSize) -> some View {
        let r = imageRect(v)
        return Group {
            if let img {
                Image(decorative: img, scale: 1)
                    .resizable()
                    .interpolation(scale(v) * displayScale >= 2 ? .none : .high)   // show real pixels when zoomed in
                    .frame(width: r.width, height: r.height)
                    .position(x: r.midX, y: r.midY)
            }
        }
        .frame(width: v.width, height: v.height)
    }

    private func single(_ img: CGImage?, _ v: CGSize, label: String) -> some View {
        layer(img, v)
            .clipped()
            .contentShape(Rectangle())
            .gesture(drag(v, movesSplit: false))
            .simultaneousGesture(doubleClick(v))
            .overlay(alignment: .bottomLeading) { tag(label) }
    }

    private func sliderView(_ v: CGSize) -> some View {
        ZStack {
            layer(after, v)
            layer(before, v).mask(alignment: .leading) { Rectangle().frame(width: v.width * split) }
            handle(v)
        }
        .frame(width: v.width, height: v.height)
        .clipped()
        .contentShape(Rectangle())
        .coordinateSpace(.named("canvas"))
        .gesture(drag(v, movesSplit: true))
        .simultaneousGesture(doubleClick(v))
        .overlay(alignment: .bottomLeading) { tag("Original") }
        .overlay(alignment: .bottomTrailing) { tag("Compressed") }
    }

    private func handle(_ v: CGSize) -> some View {
        ZStack {
            Rectangle().fill(.white).frame(width: 2).shadow(color: .black.opacity(0.5), radius: 2)
            Image(systemName: "arrow.left.and.right")
                .font(.caption.weight(.bold))
                .padding(7)
                .background(.regularMaterial, in: Circle())
                .shadow(radius: 2)
        }
        .frame(width: 30)
        .frame(maxHeight: .infinity)
        .contentShape(Rectangle())
        .position(x: v.width * split, y: v.height / 2)
        .pointerStyle(.columnResize)
        .highPriorityGesture(DragGesture(minimumDistance: 0, coordinateSpace: .named("canvas")).onChanged {
            split = min(max($0.location.x / v.width, 0), 1)
        })
        .accessibilityElement()
        .accessibilityLabel("Divider")
        .accessibilityValue("\(Int(split * 100)) percent original")
        .accessibilityAdjustableAction { dir in
            split = min(max(split + (dir == .increment ? 0.05 : -0.05), 0), 1)
        }
    }

    /// Drag pans a zoomed-in image; at Fit, dragging on the slider view moves the divider.
    private func drag(_ v: CGSize, movesSplit: Bool) -> some Gesture {
        DragGesture(minimumDistance: 0).onChanged { g in
            if canPan(v) {
                let s = scale(v)
                if panStart == nil { panStart = clamped(center, v, s) }
                center = CGPoint(x: panStart!.x - g.translation.width / s, y: panStart!.y - g.translation.height / s)
            } else if movesSplit {
                split = min(max(g.location.x / v.width, 0), 1)
            }
        }
        .onEnded { _ in
            panStart = nil
            center = clamped(center, v, scale(v))
        }
    }

    /// Double-click switches between Fit and actual pixels at the clicked spot.
    private func doubleClick(_ v: CGSize) -> some Gesture {
        SpatialTapGesture(count: 2).onEnded { e in
            setZoom(zoom == nil ? max(1, Self.levels.first { $0 > currentZoom * 1.01 } ?? 1) : nil, anchor: e.location)
        }
    }

    private func pan(by delta: CGSize, precise: Bool) {
        let v = pane(canvasSize)
        guard canPan(v) else { return }
        let s = scale(v), c = clamped(center, v, s), k: CGFloat = precise ? 1 : 12
        center = clamped(CGPoint(x: c.x - delta.width * k / s, y: c.y - delta.height * k / s), v, s)
    }

    private func magnify(_ m: CGFloat, at p: CGPoint) {
        let v = pane(canvasSize)
        let anchor = mode == .sideBySide && p.x > v.width ? CGPoint(x: p.x - v.width - 1, y: p.y) : p
        let z = currentZoom * (1 + m)
        setZoom(z / displayScale <= fitScale(v) ? nil : z, anchor: anchor)
    }

    private func tag(_ text: String) -> some View {
        Text(text)
            .font(.caption)
            .padding(.horizontal, 8).padding(.vertical, 3)
            .background(.regularMaterial, in: Capsule())
            .padding(8)
            .allowsHitTesting(false)
    }

    /// A small copy of the image with the visible area outlined; click or drag in it to move around.
    private func navigator(_ v: CGSize) -> some View {
        let k = 160 / max(imageSize.width, imageSize.height)
        let tw = imageSize.width * k, th = imageSize.height * k
        let s = scale(v), c = clamped(center, v, s)
        let vis = CGRect(x: (c.x - v.width / 2 / s) * k, y: (c.y - v.height / 2 / s) * k, width: v.width / s * k, height: v.height / s * k)
            .intersection(CGRect(x: 0, y: 0, width: tw, height: th))
        return ZStack(alignment: .topLeading) {
            if let img = after ?? before {
                Image(decorative: img, scale: 1).resizable().interpolation(.medium).frame(width: tw, height: th)
            }
            Rectangle()
                .fill(Color.white.opacity(0.15))
                .overlay(Rectangle().strokeBorder(Color.accentColor, lineWidth: 1.5))
                .frame(width: max(vis.width, 3), height: max(vis.height, 3))
                .offset(x: vis.minX, y: vis.minY)
        }
        .frame(width: tw, height: th, alignment: .topLeading)
        .clipped()
        .contentShape(Rectangle())
        .gesture(DragGesture(minimumDistance: 0).onChanged { g in
            center = clamped(CGPoint(x: g.location.x / k, y: g.location.y / k), v, s)
        })
        .padding(4)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 7))
        .shadow(radius: 3)
        .padding(10)
        .help("Click or drag to move around the image")
        .accessibilityHidden(true)
    }

    // MARK: Controls

    private var controls: some View {
        HStack(spacing: 12) {
            Picker("View", selection: $mode) {
                ForEach(Mode.allCases) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented).labelsHidden().fixedSize()
            Spacer()
            Text(String(format: "%.0f%%", currentZoom * 100))
                .monospacedDigit().foregroundStyle(.secondary)
                .help("Image pixels shown at this size on screen")
            Picker("Zoom", selection: Binding(get: { zoom ?? 0 }, set: { setZoom($0 == 0 ? nil : $0) })) {
                Text("Fit").tag(CGFloat(0))
                ForEach([1, 2, 4, 8] as [CGFloat], id: \.self) { Text("\(Int($0))×").tag($0) }
            }
            .pickerStyle(.segmented).labelsHidden().fixedSize()
            .help("1× shows one image pixel per screen pixel. Pinch or ⌘-scroll to zoom; scroll or drag to move.")
        }
        .disabled(before == nil)
    }

    private var footer: some View {
        HStack(spacing: 14) {
            VStack(alignment: .leading, spacing: 2) {
                Text(item.name).lineLimit(1).truncationMode(.middle)
                Text(sizesText).font(.caption).foregroundStyle(.secondary).monospacedDigit()
            }
            Spacer()
            scoreView
            Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
        }
    }

    private var sizesText: String {
        let what = previewed ? (Self.replacedInPlace(item) ? "Current file" : "Original") : "Original"
        var s = "\(what) \(beforeBytes.map(fmt) ?? fmt(Preview.size(item.url)))"
        if let b = beforeBytes, let a = afterBytes {
            let pct = b > 0 ? (1 - Double(a) / Double(b)) * 100 : 0
            s += " → \(previewed ? "preview" : "result") \(fmt(a)) (\(String(format: "%.1f", pct))% smaller)"
        }
        if previewed && Self.replacedInPlace(item) { s += ". The original is gone, so this re-encodes the current file with today's settings." }
        return s
    }

    @ViewBuilder private var scoreView: some View {
        if let score {
            let v = Self.verdict(score)
            Label {
                VStack(alignment: .trailing, spacing: 2) {
                    Text(v.text)
                    Text("Similarity \(String(format: "%.1f", score)) of 100").font(.caption).foregroundStyle(.secondary).monospacedDigit()
                }
            } icon: { Image(systemName: v.symbol).foregroundStyle(v.color) }
            .help("SSIMULACRA2, a measure of how different the two look to a person: 100 is identical, 88 and up "
                  + "can't be told apart at normal viewing (the Visually lossless target), around 70 shows small "
                  + "differences up close (the Smaller target), and below 50 the difference is easy to see.")
        } else if let scoreNote {
            Text(scoreNote).font(.caption).foregroundStyle(.secondary)
        } else if scoring {
            HStack(spacing: 6) { ProgressView().controlSize(.small); Text("Measuring difference…").foregroundStyle(.secondary) }
        }
    }

    static func verdict(_ s: Double) -> (text: String, symbol: String, color: Color) {
        switch s {
        case 99.5...: ("Identical", "equal.circle.fill", .green)
        case 88...: ("Visually lossless", "checkmark.circle.fill", .green)
        case 70...: ("Small differences up close", "circle.lefthalf.filled", .yellow)
        case 50...: ("Visible differences", "exclamationmark.circle.fill", .orange)
        default: ("Large differences", "exclamationmark.triangle.fill", .red)
        }
    }

    /// A finished in-place run overwrote the source, so "before" is no longer the original file.
    static func replacedInPlace(_ item: JobItem) -> Bool { item.status == .done && (item.output == nil || item.output == item.path) }

    /// Keyboard: Space closes (as in Quick Look), ⌘= / ⌘- zoom, ⌘0 actual pixels, ⌘9 fit.
    private var shortcuts: some View {
        Group {
            Button("Close") { dismiss() }.keyboardShortcut(.space, modifiers: [])
            Button("Zoom In") { zoomIn() }.keyboardShortcut("=", modifiers: .command)
            Button("Zoom In ") { zoomIn() }.keyboardShortcut("+", modifiers: .command)
            Button("Zoom Out") { zoomOut() }.keyboardShortcut("-", modifiers: .command)
            Button("Actual Pixels") { setZoom(1) }.keyboardShortcut("0", modifiers: .command)
            Button("Fit") { setZoom(nil) }.keyboardShortcut("9", modifiers: .command)
        }
        .opacity(0).allowsHitTesting(false).accessibilityHidden(true)
    }
}

/// Trackpad scrolling and pinching over the canvas, which SwiftUI does not expose on macOS.
/// Watches the window's events rather than sitting on top, so clicks and drags still reach the SwiftUI views.
private struct ScrollCatcher: NSViewRepresentable {
    var onScroll: (CGSize, Bool) -> Void
    var onMagnify: (CGFloat, CGPoint) -> Void
    var onZoomScroll: (CGFloat, CGPoint) -> Void

    func makeNSView(context: Context) -> CatcherView { let v = CatcherView(); v.owner = self; return v }
    func updateNSView(_ v: CatcherView, context: Context) { v.owner = self }

    final class CatcherView: NSView {
        var owner: ScrollCatcher?
        private var monitor: Any?
        override var isFlipped: Bool { true }
        override func hitTest(_ point: NSPoint) -> NSView? { nil }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if let monitor { NSEvent.removeMonitor(monitor); self.monitor = nil }
            guard window != nil else { return }   // removed again when the sheet closes
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.scrollWheel, .magnify]) { [weak self] e in
                guard let self, let owner = self.owner, e.window === self.window else { return e }
                let p = self.convert(e.locationInWindow, from: nil)
                guard self.bounds.contains(p) else { return e }
                if e.type == .magnify {
                    owner.onMagnify(e.magnification, p)
                } else if e.modifierFlags.contains(.command) {
                    owner.onZoomScroll(e.scrollingDeltaY * (e.hasPreciseScrollingDeltas ? 1 : 10), p)
                } else {
                    owner.onScroll(CGSize(width: e.scrollingDeltaX, height: e.scrollingDeltaY), e.hasPreciseScrollingDeltas)
                }
                return nil
            }
        }
    }
}
