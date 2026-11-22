import Foundation

/// The toolbar's two controls (Format, Quality) are driven by presets. A preset is one combination
/// of output format + quality; it carries two display tags (app-side metadata, see `PresetAxes`) and
/// a favorite flag. Favorited presets with both tags populate the toolbar; the controls stay independent.
///
/// Tags are plain strings. The well-known ones below get proper titles and a canonical order; any
/// other string is a custom tag, shown verbatim after the well-known ones.
enum AxisTags {
    static let keep = "keep"
    /// Format tags that equal the engine's format ids, in toolbar order (modern first, then classic).
    static let modernFormats = ["jxl", "avif", "heic"]
    static let classicFormats = ["webp", "jpeg", "png"]
    static let knownFormats = [keep] + modernFormats + classicFormats
    static let knownQualities = ["lossless", "visually-lossless", "smaller"]

    private static let formatTitles = ["jxl": "JPEG XL", "avif": "AVIF", "heic": "HEIC", "webp": "WebP", "jpeg": "JPEG", "png": "PNG"]

    static func formatTitle(_ tag: String) -> String {
        tag == keep ? "Preserve format" : formatTitles[tag].map { "Convert to \($0)" } ?? tag
    }
    /// Short name without "Convert to" (for preset names and Settings pickers).
    static func formatShort(_ tag: String) -> String { tag == keep ? "Preserve" : formatTitles[tag] ?? tag }

    static func qualityTitle(_ tag: String) -> String {
        switch tag {
        case "lossless": "Lossless"
        case "visually-lossless": "Visually lossless"
        case "smaller": "Smaller"
        default: tag
        }
    }
    static func qualityHint(_ tag: String) -> String? {
        SimpleChoice.Quality.allCases.first { qualityTag($0) == tag }?.hint
    }

    static func qualityTag(_ q: SimpleChoice.Quality) -> String {
        switch q { case .lossless: "lossless"; case .visuallyLossless: "visually-lossless"; case .smaller: "smaller" }
    }
    static func formatTag(_ f: SimpleChoice.Format) -> String {
        switch f { case .keep: keep; case .change(let id): id }
    }

    /// Divider group of a format tag: 0 preserve, 1 modern, 2 classic, 3 custom.
    static func formatGroup(_ tag: String) -> Int {
        tag == keep ? 0 : modernFormats.contains(tag) ? 1 : classicFormats.contains(tag) ? 2 : 3
    }
}

/// Per-preset toolbar metadata, stored in each preset file as `"toolbar": {"format", "quality", "favorite"}`
/// (the CLI keeps unknown keys when it rewrites the file).
struct PresetAxes: Codable, Equatable, Sendable {
    var format: String?
    var quality: String?
    var favorite = false
    var isTagged: Bool { format != nil && quality != nil }

    init(format: String? = nil, quality: String? = nil, favorite: Bool = false) {
        self.format = format; self.quality = quality; self.favorite = favorite
    }
    init?(json: JSONValue?) {
        guard case .object(let d)? = json else { return nil }
        if case .string(let f)? = d["format"] { format = f }
        if case .string(let q)? = d["quality"] { quality = q }
        favorite = d["favorite"]?.boolValue ?? false
    }
    var json: JSONValue {
        var d: [String: JSONValue] = ["favorite": .bool(favorite)]
        if let format { d["format"] = .string(format) }
        if let quality { d["quality"] = .string(quality) }
        return .object(d)
    }
}

/// A favorited, fully tagged preset: one cell of the format x quality grid.
struct AxisPreset: Equatable, Sendable {
    var preset: Preset
    var format: String
    var quality: String
}

extension Preset {
    /// Stable id of the shipped preset for a (format, quality) pair, e.g. `avif-visually-lossless`.
    /// None of these collides with an engine built-in id (`lossless`, `jpg`, `avif`, ...).
    static func shippedID(format: String, quality: String) -> String { "\(format)-\(quality)" }

    /// Shipped presets that are in the Finder menu by default.
    static let finderDefaults: Set<String> = ["keep-lossless", "keep-smaller", "jxl-visually-lossless"]

    /// The built-in presets: the 21 below. Must match the engine's `presets::builtin()` (a test compares them).
    static var builtIn: [Preset] { shippedAxisPresets }

    /// The 21 shipped presets (Preserve + 7 formats x 3 qualities), generated from `SimpleChoice` so the
    /// toolbar behaves exactly as the old hard-coded controls. `originals` is never set.
    static let shippedAxisPresets: [Preset] = {
        let formats: [SimpleChoice.Format] = [.keep] + SimpleChoice.targetFormats.map { .change($0.id) }
        return formats.flatMap { f in
            SimpleChoice.Quality.allCases.map { q in
                let c = SimpleChoice(format: f, quality: q)
                let ft = AxisTags.formatTag(f), qt = AxisTags.qualityTag(q)
                let id = shippedID(format: ft, quality: qt)
                return Preset(stableID: id,
                              name: "\(AxisTags.formatShort(ft)) · \(AxisTags.qualityTitle(qt))",
                              showInFinder: finderDefaults.contains(id), options: Options(c.overrides),
                              extra: ["toolbar": PresetAxes(format: ft, quality: qt, favorite: true).json])
            }
        }
    }()

    /// Default metadata of a shipped preset (all favorited), derived from its stable id.
    static func shippedAxes(stableID: String?) -> PresetAxes? {
        guard let id = stableID else { return nil }
        for f in AxisTags.knownFormats { for q in AxisTags.knownQualities where shippedID(format: f, quality: q) == id {
            return PresetAxes(format: f, quality: q, favorite: true)
        } }
        return nil
    }
}

/// The grid of favorited presets, and the rules for resolving a (format, quality) pair against it.
///
/// Resolution rule when the exact pair has no preset:
/// the control the user just touched keeps its value and the other axis moves to the nearest available tag
/// (distance in canonical order), so what is highlighted always matches what will run.
struct AxisSet: Equatable, Sendable {
    var items: [AxisPreset]

    /// What a launch with nothing stored starts on: Preserve format + Lossless.
    static let defaultChoice = (format: AxisTags.keep, quality: "lossless")

    private static func rank(_ tag: String, known: [String], custom: [String]) -> Int {
        known.firstIndex(of: tag) ?? (known.count + (custom.firstIndex(of: tag) ?? 0))
    }
    private func ordered(_ tags: [String], known: [String]) -> [String] {
        var seen: [String] = []
        for t in tags where !seen.contains(t) { seen.append(t) }
        let custom = seen.filter { !known.contains($0) }
        return seen.sorted { Self.rank($0, known: known, custom: custom) < Self.rank($1, known: known, custom: custom) }
    }
    /// Distinct format tags in toolbar order: Preserve, modern, classic, then custom in favorite order.
    var formats: [String] { ordered(items.map(\.format), known: AxisTags.knownFormats) }
    var qualities: [String] { ordered(items.map(\.quality), known: AxisTags.knownQualities) }

    private func distance(_ a: String, _ b: String, tags: [String]) -> Int {
        abs((tags.firstIndex(of: a) ?? 0) - (tags.firstIndex(of: b) ?? 0))
    }

    func exact(format: String, quality: String) -> AxisPreset? {
        items.first { $0.format == format && $0.quality == quality }
    }

    /// The preset for the pair, or the nearest one when the pair is missing. `touched` says which control moved.
    enum Axis { case format, quality }
    func resolve(format: String, quality: String, touched: Axis) -> AxisPreset? {
        if let e = exact(format: format, quality: quality) { return e }
        switch touched {
        case .quality:   // keep the quality, nearest format
            let fs = formats
            return items.filter { $0.quality == quality }
                .min { distance($0.format, format, tags: fs) < distance($1.format, format, tags: fs) }
                ?? items.filter { $0.format == format }.min { distance($0.quality, quality, tags: qualities) < distance($1.quality, quality, tags: qualities) }
        case .format:    // keep the format, nearest quality
            let qs = qualities
            return items.filter { $0.format == format }
                .min { distance($0.quality, quality, tags: qs) < distance($1.quality, quality, tags: qs) }
                ?? items.filter { $0.quality == quality }.min { distance($0.format, format, tags: formats) < distance($1.format, format, tags: formats) }
        }
    }

    /// Every option key any favorite sets, plus the base keys: applying a preset clears these first.
    var managedKeys: Set<String> {
        items.reduce(into: Set(SimpleChoice.managedKeys)) { $0.formUnion($1.preset.options.values.keys) }
    }

    /// The overrides a toolbar pair stands for on its own (what Reset and launch put back). Nil choice: none.
    func baseline(_ c: (format: String, quality: String)?) -> Options {
        var o = Options()
        if let c, let p = resolve(format: c.format, quality: c.quality, touched: .format) { apply(p, to: &o) }
        return o
    }

    func apply(_ p: AxisPreset, to options: inout Options) {
        for k in managedKeys { options[k] = nil }
        for (k, v) in p.preset.options.values { options[k] = v }
    }

    private static func norm(_ v: JSONValue?) -> JSONValue? { if case .null? = v { nil } else { v } }

    /// Does `effective` carry everything this preset sets? Absent and null are equal; an absent format is "keep".
    static func matches(_ p: Preset, _ effective: Options) -> Bool {
        p.options.values.allSatisfy { k, v in
            if k == "format" { return (effective.string("format") ?? "keep") == v.stringValue }
            return norm(effective[k]) == norm(v)
        }
    }

    /// The preset the effective options correspond to, or nil when customised beyond any favorite.
    /// Ties prefer the most specific preset, then the sticky pair, then favorite order.
    func matching(_ effective: Options, prefer: (format: String, quality: String)? = nil) -> AxisPreset? {
        let hits = items.filter { Self.matches($0.preset, effective) }
        let best = hits.map { $0.preset.options.values.count }.max()
        let top = hits.filter { $0.preset.options.values.count == best }
        if let p = prefer, let m = top.first(where: { $0.format == p.format && $0.quality == p.quality }) { return m }
        return top.first
    }

    /// One line for the caption under the pickers.
    static func outcome(format: String, quality: String) -> String {
        if format == AxisTags.keep { return "Each image stays in its current format." }
        let name = AxisTags.formatShort(format)
        let base = "Images become \(name); originals are kept next to the new files."
        if quality == "lossless", ["avif", "heic", "webp"].contains(format) {
            return base + " Lossless is often larger than a JPEG, so those files may be skipped; try Visually lossless."
        }
        return base
    }
}
