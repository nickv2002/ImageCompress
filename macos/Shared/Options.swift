import Foundation

/// A (possibly partial) set of engine options: the JSON object that
/// `imgcompress` accepts via `--options-file` and stores in preset files.
///
/// Backed by a plain key/value map so keys this build does not know about
/// survive decode/encode untouched, and an explicit `null` (e.g. "lossless")
/// stays distinguishable from an absent key ("use the default").
struct Options: Codable, Equatable, Sendable {
    var values: [String: JSONValue] = [:]

    init(_ values: [String: JSONValue] = [:]) { self.values = values }
    init(from decoder: Decoder) throws { values = try decoder.singleValueContainer().decode([String: JSONValue].self) }
    func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        try c.encode(values)
    }

    subscript(key: String) -> JSONValue? {
        get { values[key] }
        set { values[key] = newValue }
    }

    func string(_ k: String) -> String? { if case .string(let s)? = values[k] { s } else { nil } }
    func int(_ k: String) -> Int? { if case .number(let n)? = values[k] { Int(n) } else { nil } }
    func bool(_ k: String) -> Bool? { if case .bool(let b)? = values[k] { b } else { nil } }

    /// `self` layered over `base` (keys in `self` win).
    func merged(over base: Options) -> Options {
        Options(base.values.merging(values) { _, mine in mine })
    }

    var originalsMode: String { string("originals") ?? "keep" }
    var isDestructive: Bool { originalsMode != "keep" }
    /// Whether the originals setting applies: the result is a new file (another format, or saved alongside or
    /// in a folder). In place, the source itself is replaced and `originals` is never used.
    var leavesOriginals: Bool { (string("format") ?? "keep") != "keep" || (string("destination") ?? "in-place") != "in-place" }
}

extension JSONValue {
    static func int(_ i: Int) -> JSONValue { .number(Double(i)) }
    var intValue: Int? { if case .number(let n) = self { Int(n) } else { nil } }
    var boolValue: Bool? { if case .bool(let b) = self { b } else { nil } }
    var stringValue: String? { if case .string(let s) = self { s } else { nil } }
    var isNull: Bool { if case .null = self { true } else { false } }
}
