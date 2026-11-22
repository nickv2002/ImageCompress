import Foundation

/// The generator for the 18 shipped toolbar presets (`Preset.shippedAxisPresets`); the toolbar itself is
/// driven by favorited presets (PresetAxes.swift). The two questions most people ever need to answer, mapped onto engine options:
/// 1. keep each image's format, or change to a modern one?
/// 2. how much loss is acceptable?
///
/// Everything else stays at the engine defaults and lives under "Advanced" in the inspector.
struct SimpleChoice: Equatable, Sendable {
    enum Format: Equatable, Sendable {
        case keep
        case change(String)   // engine format name, one of `SimpleChoice.targetFormats`
    }

    enum Quality: String, CaseIterable, Identifiable, Sendable {
        case lossless, visuallyLossless, smaller
        var id: String { rawValue }

        var title: String {
            switch self {
            case .lossless: "Lossless"
            case .visuallyLossless: "Visually lossless"
            case .smaller: "Smaller"
            }
        }
        var hint: String {
            switch self {
            case .lossless: "Every pixel stays exactly the same. Smallest safe savings."
            case .visuallyLossless: "Technically lossy, but you would not see a difference. Modest savings for JPEG, much more with a modern format."
            case .smaller: "A little loss that is hard to spot, for a big size reduction."
            }
        }
        /// SSIMULACRA2 target for the per-image quality search (nil = lossless).
        /// Measured with the engine's own SSIMULACRA2: the search lands in
        /// [target, target + 1.5]. 88 is just under the usual "visually lossless" bar of 90, which on JPEG
        /// sources saves nothing in the same format; 70 is "high quality, hard to spot" (about 55-65% smaller).
        var target: Int? {
            switch self { case .lossless: nil; case .visuallyLossless: 88; case .smaller: 70 }
        }
        var pngQuantize: [Int]? {
            switch self { case .lossless: nil; case .visuallyLossless: [80, 95]; case .smaller: [70, 90] }
        }
    }

    /// First-class conversion targets.
    static let primaryFormats: [(id: String, title: String)] = [("jxl", "JPEG XL"), ("avif", "AVIF"), ("heic", "HEIC")]
    /// The classic formats are equally first-class conversion targets (listed after the modern ones).
    static let secondaryFormats: [(id: String, title: String)] = [("webp", "WebP"), ("jpeg", "JPEG"), ("png", "PNG")]
    static var targetFormats: [(id: String, title: String)] { primaryFormats + secondaryFormats }
    static let defaultModern = "jxl"

    var format: Format
    var quality: Quality

    /// "keep|lossless" or "jxl|smaller": the pre-presets UserDefaults format, read once to migrate the sticky choice.
    var stored: String {
        let f: String; switch format { case .keep: f = "keep"; case .change(let id): f = id }
        return f + "|" + quality.rawValue
    }
    init(format: Format, quality: Quality) { self.format = format; self.quality = quality }
    init?(stored: String) {
        let p = stored.split(separator: "|").map(String.init)
        guard p.count == 2, let q = Quality(rawValue: p[1]) else { return nil }
        if p[0] == "keep" { self.init(format: .keep, quality: q) }
        else if Self.targetFormats.contains(where: { $0.id == p[0] }) { self.init(format: .change(p[0]), quality: q) }
        else { return nil }
    }

    /// Keys this control owns. Applying a choice clears all of them first.
    static let managedKeys = ["format", "target-quality", "png-quantize", "jpeg-quality", "jxl-quality",
                              "webp-quality", "avif-quality", "heic-quality"]

    /// The explicit overrides for this choice (nulls are explicit so they beat a preset's values).
    var overrides: [String: JSONValue] {
        var o: [String: JSONValue] = [:]
        switch format {
        case .keep: o["format"] = .string("keep")
        case .change(let f): o["format"] = .string(f)
        }
        if let t = quality.target { o["target-quality"] = .int(t) } else { o["target-quality"] = .null }
        if let q = quality.pngQuantize { o["png-quantize"] = .array(q.map { .int($0) }) } else { o["png-quantize"] = .null }
        if quality == .lossless {
            o["jpeg-quality"] = .null; o["jxl-quality"] = .null; o["webp-quality"] = .null
        }
        if case .change(let f) = format {
            // AVIF/HEIC have no exact-lossless switch here; 100 is the closest. HEIC ignores the
            // quality search, so give it fixed qualities that land near the other formats' sizes.
            if f == "avif", quality == .lossless { o["avif-quality"] = .int(100) }
            if f == "heic" { o["heic-quality"] = .int(quality == .lossless ? 100 : quality == .visuallyLossless ? 90 : 50) }
        }
        return o
    }
}
