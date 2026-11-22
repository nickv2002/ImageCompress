import Foundation

/// One preset file in the Presets folder: `{"name", "show_in_finder", "options": {partial Options}, ...}`.
struct Preset: Codable, Identifiable, Equatable, Sendable {
    /// The file name without `.json` (`--preset` matches it first). Not stored inside the file.
    var stableID: String?
    var name: String
    var showInFinder: Bool
    var options: Options
    var extra: [String: JSONValue] = [:]

    var id: String { name }
    var isDestructive: Bool { options.isDestructive }

    init(stableID: String? = nil, name: String, showInFinder: Bool = true, options: Options = Options(), extra: [String: JSONValue] = [:]) {
        self.stableID = stableID; self.name = name; self.showInFinder = showInFinder; self.options = options; self.extra = extra
    }

    private struct K: CodingKey {
        var stringValue: String; var intValue: Int? { nil }
        init?(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { nil }
    }
    private static let known: Set<String> = ["id", "name", "show_in_finder", "options"]

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        stableID = try? c.decodeIfPresent(String.self, forKey: K(stringValue: "id")!)
        name = (try c.decodeIfPresent(String.self, forKey: K(stringValue: "name")!)) ?? ""
        showInFinder = (try? c.decodeIfPresent(Bool.self, forKey: K(stringValue: "show_in_finder")!)) ?? false
        options = (try? c.decodeIfPresent(Options.self, forKey: K(stringValue: "options")!)) ?? Options()
        for k in c.allKeys where !Self.known.contains(k.stringValue) {
            extra[k.stringValue] = try? c.decode(JSONValue.self, forKey: k)
        }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: K.self)
        try c.encodeIfPresent(stableID, forKey: K(stringValue: "id")!)
        try c.encode(name, forKey: K(stringValue: "name")!)
        try c.encode(showInFinder, forKey: K(stringValue: "show_in_finder")!)
        try c.encode(options, forKey: K(stringValue: "options")!)
        for (k, v) in extra { try c.encode(v, forKey: K(stringValue: k)!) }
    }
}

/// A list of presets as `imgcompress presets list --json` and `presets export` (no name) print it.
struct PresetsFile: Codable, Equatable, Sendable {
    var version = 1
    var presets: [Preset] = []
}

struct PresetsFileError: Error, LocalizedError {
    var message: String
    var errorDescription: String? { message }
}

/// The Presets folder: one `<id>.json` per preset, shared with the CLI and the Finder extension. List order comes
/// from `preset-order.json` beside it (ids), then the built-ins, then the rest by name (same as the engine).
enum PresetFolder {
    static var directory: URL { AppGroup.presetsDirectory }
    static var orderURL: URL { AppGroup.presetOrderURL }

    /// Ids of the 21 built-ins in list order (the engine's `presets::builtin`).
    static let builtinIDs: [String] = ["keep", "jxl", "avif", "heic", "webp", "jpeg", "png"].flatMap { f in
        ["lossless", "visually-lossless", "smaller"].map { "\(f)-\($0)" }
    }

    static func slug(_ name: String) -> String {
        let s = String(name.lowercased().map { $0.isASCII && ($0.isLetter || $0.isNumber) ? $0 : "-" })
        return s.split(separator: "-").joined(separator: "-")
    }

    /// `*.json` files, not hidden (temp files from atomic writes are skipped).
    static func files(in dir: URL = directory) -> [URL] {
        let names = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
        return names.filter { !$0.hasPrefix(".") && $0.lowercased().hasSuffix(".json") }.sorted().map { dir.appendingPathComponent($0) }
    }

    /// One preset file; its id is the file name and a missing name falls back to it. Throws with where it went wrong.
    static func read(_ url: URL) throws -> Preset {
        let data = try Data(contentsOf: url)
        var p: Preset
        do { p = try JSONDecoder().decode(Preset.self, from: data) }
        catch DecodingError.dataCorrupted(let c) {
            throw PresetsFileError(message: (c.underlyingError as NSError?)?.userInfo[NSDebugDescriptionErrorKey] as? String ?? c.debugDescription)
        } catch DecodingError.typeMismatch(_, let c) {
            throw PresetsFileError(message: "wrong kind of value at \(c.codingPath.map(\.stringValue).joined(separator: "."))")
        }
        p.stableID = url.deletingPathExtension().lastPathComponent
        if p.name.trimmingCharacters(in: .whitespaces).isEmpty { p.name = p.stableID! }
        return p
    }

    struct Loaded: Sendable {
        var presets: [Preset]
        /// File name and why it was skipped.
        var problems: [(file: String, message: String)]
    }

    /// Nil when the folder does not exist yet.
    static func load(from dir: URL = directory, order: URL = orderURL) -> Loaded? {
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: dir.path, isDirectory: &isDir), isDir.boolValue else { return nil }
        var presets: [Preset] = [], problems: [(String, String)] = []
        for url in files(in: dir) {
            do {
                let p = try read(url)
                if let other = presets.first(where: { $0.name.caseInsensitiveCompare(p.name) == .orderedSame
                    || $0.stableID?.caseInsensitiveCompare(p.stableID ?? "") == .orderedSame }) {
                    problems.append((url.lastPathComponent, "same name as \(other.stableID ?? other.name).json"))
                } else { presets.append(p) }
            } catch {
                problems.append((url.lastPathComponent, error.localizedDescription))
            }
        }
        return Loaded(presets: sorted(presets, order: readOrder(order)), problems: problems)
    }

    static func sorted(_ presets: [Preset], order: [String]) -> [Preset] {
        func key(_ p: Preset) -> (Int, Int, String) {
            let id = p.stableID ?? ""
            if let i = order.firstIndex(of: id) { return (0, i, "") }
            if let i = builtinIDs.firstIndex(of: id) { return (1, i, "") }
            return (2, 0, p.name.lowercased())
        }
        return presets.sorted { key($0) < key($1) }
    }

    static func readOrder(_ url: URL = orderURL) -> [String] {
        (try? JSONDecoder().decode([String].self, from: Data(contentsOf: url))) ?? []
    }
    static func writeOrder(_ ids: [String], to url: URL = orderURL) throws {
        try JSONEncoder().encode(ids).write(to: url, options: .atomic)
    }

    static func url(for id: String, in dir: URL = directory) -> URL { dir.appendingPathComponent("\(id).json") }

    /// Atomic write of `<stableID>.json`, without the id inside.
    static func write(_ p: Preset, in dir: URL = directory) throws {
        guard let id = p.stableID, !id.isEmpty else { throw PresetsFileError(message: "preset has no file name") }
        var q = p; q.stableID = nil
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try (enc.encode(q) + Data("\n".utf8)).write(to: url(for: id, in: dir), options: .atomic)
    }

    static func remove(_ id: String, in dir: URL = directory) throws {
        let u = url(for: id, in: dir)
        if FileManager.default.fileExists(atPath: u.path) { try FileManager.default.removeItem(at: u) }
    }

    /// An id not used by any `.json` file in the folder (readable or not) nor in `taken`.
    static func uniqueID(_ base: String, in dir: URL = directory, taken: Set<String> = []) -> String {
        let base = base.isEmpty ? "preset" : base
        let used = Set(files(in: dir).map { $0.deletingPathExtension().lastPathComponent.lowercased() } + taken.map { $0.lowercased() })
        if !used.contains(base.lowercased()) { return base }
        return (2...).lazy.map { "\(base)-\($0)" }.first { !used.contains($0.lowercased()) }!
    }

    /// Names, sizes and dates of the files: changes when anything in the folder is edited, added or removed.
    static func fingerprint(_ dir: URL = directory, order: URL = orderURL) -> [String] {
        (files(in: dir) + [order]).map { u in
            let v = try? u.resourceValues(forKeys: [.contentModificationDateKey, .fileSizeKey])
            return "\(u.lastPathComponent)|\(v?.fileSize ?? -1)|\(v?.contentModificationDate?.timeIntervalSince1970 ?? 0)"
        }
    }
}

/// `{preset, overrides, paths, start}` written by `imgcompress open` / the Finder extension.
/// The `imagecompress://job/<uuid>` URL carries only the uuid; the payload never travels in the URL.
struct JobFile: Codable, Equatable, Sendable {
    var preset: String?
    var overrides: [String: JSONValue]?
    var paths: [String]
    var start: Bool?

    static func isValid(uuid: String) -> Bool {
        UUID(uuidString: uuid) != nil
    }

    /// Writes the job file and returns the URL to open.
    @discardableResult
    func write(uuid: UUID = UUID(), in dir: URL = AppGroup.jobsDirectory) throws -> URL {
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try JSONEncoder().encode(self).write(to: dir.appendingPathComponent("\(uuid.uuidString).json"), options: .atomic)
        return URL(string: "\(AppGroup.urlScheme)://job/\(uuid.uuidString)")!
    }

    /// Reads and deletes the job file for `uuid` (refuses anything that is not a UUID).
    static func take(uuid: String, in dir: URL = AppGroup.jobsDirectory) -> JobFile? {
        guard isValid(uuid: uuid) else { return nil }
        let url = dir.appendingPathComponent("\(uuid.uppercased()).json")
        let alt = dir.appendingPathComponent("\(uuid.lowercased()).json")
        for u in [url, alt] where FileManager.default.fileExists(atPath: u.path) {
            defer { try? FileManager.default.removeItem(at: u) }
            guard let data = try? Data(contentsOf: u) else { return nil }
            return try? JSONDecoder().decode(JobFile.self, from: data)
        }
        return nil
    }
}
