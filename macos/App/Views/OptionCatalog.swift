import Foundation

/// How an option is edited. The catalog only describes presentation; which options exist
/// (and their defaults and enum values) always comes from the engine's schema.
enum OptionControl: Sendable {
    case toggle
    case int(ClosedRange<Int>)
    /// Number, or `null` meaning `nullLabel` (e.g. lossless, off, same as quality).
    case optionalInt(ClosedRange<Int>, nullLabel: String, whenOn: Int)
    case range(ClosedRange<Int>, nullLabel: String)
    case choice
    case intChoice([Int])
    case text
    case destination   // in-place / alongside / a folder chosen with an open panel
    case kilobytes
}

struct OptionSpec: Identifiable, Sendable {
    var key: String
    var title: String
    var hint: String
    var control: OptionControl
    var id: String { key }
}

struct PanelSpec: Identifiable, Sendable {
    var id: String
    var title: String
    var symbol: String
    var specs: [OptionSpec]
}

enum OptionCatalog {
    static let panels: [PanelSpec] = [
        PanelSpec(id: "output", title: "Output", symbol: "square.and.arrow.down", specs: [
            .init(key: "format", title: "Convert to", hint: "Keep the original format, or convert every image.", control: .choice),
            .init(key: "destination", title: "Destination", hint: "Replace each file, save next to it with the suffix, or collect results in a folder you choose.", control: .destination),
            .init(key: "suffix", title: "Suffix", hint: "Added to the file name when writing alongside the original.", control: .text),
            .init(key: "originals", title: "Originals", hint: "For results saved alongside or in a folder: keep the source, move it to the Trash, or delete it. Replace in place always overwrites the file.", control: .choice),
            .init(key: "replace-if", title: "Replace when", hint: "Only write a result that is smaller, or always.", control: .choice),
            .init(key: "overwrite", title: "If the file exists", hint: "When the output name is already taken by another file: skip the image, save under a new name (name-2), or replace it. Images in one run never overwrite each other.", control: .choice),
            .init(key: "preserve-dates", title: "Preserve dates", hint: "Keep the original modification date.", control: .toggle),
        ]),
        PanelSpec(id: "resize", title: "Resize", symbol: "arrow.up.left.and.arrow.down.right", specs: [
            .init(key: "max-dimension", title: "Max dimension (px)", hint: "Shrink images whose longest side exceeds this. Smaller files, lower detail.", control: .optionalInt(16...16384, nullLabel: "Off", whenOn: 2048)),
            .init(key: "scale", title: "Scale (%)", hint: "Scale every image by a percentage.", control: .optionalInt(1...100, nullLabel: "Off", whenOn: 50)),
        ]),
        PanelSpec(id: "metadata", title: "Metadata", symbol: "tag", specs: [
            .init(key: "metadata", title: "Keep metadata", hint: "Stripping saves bytes; the colour profile (ICC) is what keeps colours correct.", control: .choice),
            .init(key: "convert-to-srgb", title: "Convert to sRGB", hint: "Convert wide-gamut images to sRGB so every viewer renders them the same.", control: .toggle),
            .init(key: "compact-icc", title: "Compact ICC profile", hint: "Swap sRGB-equivalent profiles for a tiny one.", control: .toggle),
            .init(key: "strip-previews", title: "Strip embedded previews", hint: "Remove EXIF thumbnails and other embedded previews.", control: .toggle),
        ]),
        PanelSpec(id: "jpeg", title: "JPEG", symbol: "photo", specs: [
            .init(key: "jpeg-encoder", title: "Encoder", hint: "jpegli: smaller at equal quality. mozjpeg: the long-standing reference.", control: .choice),
            .init(key: "jpeg-quality", title: "Quality", hint: "Higher is larger and closer to the source. Lossless only re-packs JPEG files; other images converted to JPEG are skipped.", control: .optionalInt(1...100, nullLabel: "Lossless", whenOn: 85)),
            .init(key: "jpeg-progressive", title: "Progressive", hint: "Usually a little smaller; images load in passes.", control: .toggle),
            .init(key: "jpeg-subsampling", title: "Chroma subsampling", hint: "4:2:0 is smallest; 4:4:4 keeps sharp colour edges.", control: .choice),
            .init(key: "jpeg-strategy", title: "Strategy", hint: "Lossless re-pack, re-encode, or whichever is smaller.", control: .choice),
        ]),
        PanelSpec(id: "png", title: "PNG", symbol: "photo.fill", specs: [
            .init(key: "png-quantize", title: "Quantize quality", hint: "Reduces to a palette; files that cannot reach the minimum stay lossless.", control: .range(0...100, nullLabel: "Lossless only")),
            .init(key: "png-quantize-speed", title: "Quantize speed", hint: "Slower gives slightly better palettes.", control: .int(1...11)),
            .init(key: "png-dither", title: "Dithering", hint: "Smooths gradients after palette reduction at a small size cost.", control: .toggle),
            .init(key: "png-max-colors", title: "Max colours", hint: "Upper bound for the palette size.", control: .int(2...256)),
            .init(key: "png-optimize-level", title: "Optimize level", hint: "Higher searches harder for a smaller file and takes longer.", control: .int(0...6)),
            // png-zopfli is left to the CLI (--png-zopfli): many times slower for under 1%, sometimes larger.
            .init(key: "png-interlace", title: "Interlace", hint: "Adam7 interlacing makes files larger.", control: .toggle),
        ]),
        PanelSpec(id: "webp", title: "WebP", symbol: "globe", specs: [
            .init(key: "webp-quality", title: "Quality", hint: "Higher is larger and closer to the source.", control: .optionalInt(1...100, nullLabel: "Lossless", whenOn: 80)),
            .init(key: "webp-method", title: "Method", hint: "Higher is slower and a little smaller.", control: .int(0...6)),
            .init(key: "webp-alpha-quality", title: "Alpha quality", hint: "Quality of the transparency channel.", control: .int(1...100)),
        ]),
        PanelSpec(id: "jxl", title: "JPEG XL", symbol: "sparkles", specs: [
            .init(key: "jxl-quality", title: "Quality", hint: "Lossless re-packs JPEGs reversibly, about 20% smaller.", control: .optionalInt(1...100, nullLabel: "Lossless", whenOn: 85)),
            .init(key: "jxl-effort", title: "Effort", hint: "Higher is slower and smaller.", control: .int(1...10)),
            .init(key: "jxl-jpeg-transcode", title: "Reversible JPEG transcode", hint: "Recompress JPEG data without any pixel change.", control: .toggle),
            .init(key: "jxl-progressive", title: "Progressive", hint: "Loads in passes; slightly larger.", control: .toggle),
        ]),
        PanelSpec(id: "avif", title: "AVIF", symbol: "sparkle", specs: [
            .init(key: "avif-quality", title: "Quality", hint: "Higher is larger and closer to the source.", control: .int(1...100)),
            .init(key: "avif-alpha-quality", title: "Alpha quality", hint: "Transparency quality. Same as quality when off.", control: .optionalInt(1...100, nullLabel: "Same as quality", whenOn: 90)),
            .init(key: "avif-speed", title: "Speed", hint: "Higher is faster and larger. Lower is slower and smaller.", control: .int(0...10)),
            .init(key: "avif-depth", title: "Bit depth", hint: "10-bit reduces banding in gradients.", control: .intChoice([8, 10])),
        ]),
        PanelSpec(id: "heic", title: "HEIC", symbol: "camera", specs: [
            .init(key: "heic-quality", title: "Quality", hint: "Higher is larger and closer to the source.", control: .int(1...100)),
            .init(key: "heic-depth", title: "Bit depth", hint: "8 or 10 bits per channel.", control: .intChoice([8, 10])),
        ]),
        PanelSpec(id: "gif", title: "GIF", symbol: "play.rectangle", specs: [
            .init(key: "gif-optimize-level", title: "Optimize level", hint: "3 tries the most strategies.", control: .int(1...3)),
            .init(key: "gif-lossy", title: "Lossy", hint: "Trades fidelity for size; off keeps every pixel.", control: .optionalInt(0...200, nullLabel: "Off", whenOn: 80)),
            .init(key: "gif-max-colors", title: "Max colours", hint: "Fewer colours, smaller files.", control: .int(2...256)),
        ]),
        PanelSpec(id: "animation", title: "Animation", symbol: "film", specs: [
            .init(key: "anim-to", title: "Convert animations to", hint: "Animated GIF, APNG, WebP and AVIF. Keep optimizes in the same format; animated WebP and AVIF are far smaller than GIF.", control: .choice),
        ]),
        PanelSpec(id: "svg", title: "SVG", symbol: "scribble.variable", specs: [
            .init(key: "svg-precision", title: "Precision", hint: "Decimal places kept in numbers and paths. Fewer is smaller.", control: .int(0...5)),
            .init(key: "svg-aggressive", title: "Aggressive", hint: "Also rename ids and rewrite transforms. Breaks SVGs that scripts or CSS refer to by id.", control: .toggle),
            .init(key: "svg-max-size", title: "Maximum file size (KB)", hint: "Skip larger files; the optimiser is slow on huge SVGs. 0 means no limit.", control: .kilobytes),
        ]),
        PanelSpec(id: "quality-search", title: "Quality Search", symbol: "scope", specs: [
            .init(key: "target-quality", title: "Target score", hint: "Picks the smallest quality that reaches this SSIMULACRA2 score. 70 is about visually lossless.", control: .optionalInt(1...100, nullLabel: "Off", whenOn: 70)),
            .init(key: "target-tolerance", title: "Tolerance", hint: "Accepted distance from the target.", control: .int(0...20)),
            .init(key: "max-encodes", title: "Max encodes", hint: "Upper bound on trial encodes per image.", control: .int(1...10)),
        ]),
    ]

    static let general: [OptionSpec] = [
        .init(key: "recursive", title: "Process folders recursively", hint: "Descend into dropped folders.", control: .toggle),
        .init(key: "include-hidden", title: "Include hidden files", hint: "Also process dot-files and files in hidden folders.", control: .toggle),
        .init(key: "min-size", title: "Minimum file size (KB)", hint: "Skip files smaller than this.", control: .kilobytes),
        .init(key: "jobs", title: "Parallel jobs", hint: "0 uses every core.", control: .int(0...64)),
        .init(key: "history", title: "Remember processed files", hint: "Skip files that were already optimized.", control: .toggle),
    ]

    /// The few options most people ever touch; everything else lives under "Advanced" in the inspector.
    static let essentialKeys = ["originals", "max-dimension", "metadata"]
    static var essentials: [OptionSpec] {
        let byKey = Dictionary(uniqueKeysWithValues: panels.flatMap(\.specs).map { ($0.key, $0) })
        return essentialKeys.compactMap { byKey[$0] }
    }
    /// Panels with the essentials removed, for the Advanced disclosure.
    /// "format" is owned by the Goal control, so it is not offered a second time.
    static var advancedPanels: [PanelSpec] {
        panels.map { PanelSpec(id: $0.id, title: $0.title, symbol: $0.symbol, specs: $0.specs.filter { !essentialKeys.contains($0.key) && $0.key != "format" }) }
            .filter { !$0.specs.isEmpty }
    }

    /// General settings people actually touch; the rest sit under "More Processing Options".
    static let generalCommonKeys = ["recursive", "history"]

    /// Preset editor: the few output options up front, everything else under Advanced.
    static let presetCommonKeys = ["format", "originals", "destination", "max-dimension", "metadata"]
    static var presetCommon: [OptionSpec] {
        let byKey = Dictionary(uniqueKeysWithValues: panels.flatMap(\.specs).map { ($0.key, $0) })
        return presetCommonKeys.compactMap { byKey[$0] }
    }
    static var presetAdvancedPanels: [PanelSpec] {
        panels.map { PanelSpec(id: $0.id, title: $0.title, symbol: $0.symbol, specs: $0.specs.filter { !presetCommonKeys.contains($0.key) }) }
            .filter { !$0.specs.isEmpty }
    }

    static var all: [OptionSpec] { general + panels.flatMap(\.specs) }

    /// Human titles for the engine's enum values in pop-ups; unknown values fall back to the raw string.
    static func choiceTitle(_ key: String, _ value: String) -> String {
        let t: [String: [String: String]] = [
            "originals": ["keep": "Keep", "trash": "Move to Trash", "delete": "Delete"],
            "metadata": ["none": "None", "icc": "Colour profile only", "all": "All", "exif-no-gps": "All except location"],
            "overwrite": ["skip": "Skip the file", "rename": "Add a number", "replace": "Replace"],
            "replace-if": ["smaller": "Only if smaller", "always": "Always"],
            "anim-to": ["keep": "Same format", "webp": "WebP", "avif": "AVIF"],
            "format": ["keep": "Preserve", "jpeg": "JPEG", "png": "PNG", "webp": "WebP", "heic": "HEIC", "avif": "AVIF", "jxl": "JPEG XL"],
            "jpeg-encoder": ["jpegli": "jpegli", "mozjpeg": "MozJPEG"],
            "jpeg-subsampling": ["auto": "Automatic", "444": "4:4:4 (sharpest)", "422": "4:2:2", "420": "4:2:0 (smallest)"],
        ]
        return t[key]?[value] ?? value
    }
}
