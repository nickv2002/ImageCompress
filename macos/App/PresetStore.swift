import Foundation
import Observation

/// Reads and writes the Presets folder (one JSON file per preset, shared with the CLI and the Finder extension).
/// Everything about a preset lives in its file, including its toolbar tags, so files can be edited by hand,
/// shared, or dropped in.
@MainActor @Observable
final class PresetStore {
    static let shared = PresetStore()

    var presets: [Preset] = []
    /// Files in the folder that could not be used, and why. They are never written over.
    private(set) var problems: [(file: String, message: String)] = []
    var saveError: String?
    let directory: URL
    let orderURL: URL
    /// What is on disk, by id, so a save writes only the files that changed.
    private var saved: [String: Preset] = [:]
    private var savedOrder: [String] = []
    private var fingerprint: [String]?
    private var pendingSave: Task<Void, Never>?

    init(directory: URL = AppGroup.presetsDirectory, order: URL = AppGroup.presetOrderURL) {
        self.directory = directory
        self.orderURL = order
        reload(force: true)
    }

    var loadError: String? {
        if let saveError { return saveError }
        guard let first = problems.first else { return nil }
        let more = problems.count > 1 ? " (and \(problems.count - 1) more)" : ""
        return "\(first.file) was skipped: \(first.message.trimmingCharacters(in: CharacterSet(charactersIn: ". ")))\(more)."
    }

    // MARK: toolbar axes

    /// A preset's toolbar tags: its `toolbar` key, else the shipped defaults for its id, else none.
    func axes(for p: Preset) -> PresetAxes {
        PresetAxes(json: p.extra["toolbar"]) ?? Preset.shippedAxes(stableID: p.stableID) ?? PresetAxes()
    }
    func setAxes(_ a: PresetAxes, for p: Preset) {
        guard let i = index(of: p.name) else { return }
        presets[i].extra["toolbar"] = a.json; commit()
    }

    /// Favorited presets with both tags, in list order: the grid behind the Format and Quality controls.
    var axisSet: AxisSet {
        AxisSet(items: presets.compactMap { p in
            let a = axes(for: p)
            guard a.favorite, let f = a.format, let q = a.quality else { return nil }
            return AxisPreset(preset: p, format: f, quality: q)
        })
    }
    /// Presets that do not map onto the two axes (no tags): the Presets menu and the Process menu list these.
    var plainPresets: [Preset] { presets.filter { !axes(for: $0).isTagged } }
    /// Distinct tags in use, for the Settings pickers.
    var customFormatTags: [String] { Array(Set(presets.compactMap { axes(for: $0).format }.filter { !AxisTags.knownFormats.contains($0) })).sorted() }
    var customQualityTags: [String] { Array(Set(presets.compactMap { axes(for: $0).quality }.filter { !AxisTags.knownQualities.contains($0) })).sorted() }

    func preset(named name: String) -> Preset? { presets.first { $0.name == name } }

    /// Resolves like `--preset`: stable id first, then name (case-insensitive).
    func resolve(_ key: String) -> Preset? {
        presets.first { $0.stableID == key } ?? presets.first { $0.name.caseInsensitiveCompare(key) == .orderedSame }
            ?? Self.legacyAlias[key.lowercased()].flatMap { id in presets.first { $0.stableID == id } }
    }
    /// Old built-in ids and their replacements (same as the engine's `presets::legacy_alias`).
    static let legacyAlias: [String: String] = [
        "lossless": "keep-lossless", "lossy": "keep-smaller",
        "jpg": "jpeg-visually-lossless", "jpeg": "jpeg-visually-lossless", "png": "png-visually-lossless",
        "heif": "heic-visually-lossless", "heic": "heic-visually-lossless",
        "avif": "avif-visually-lossless", "jxl": "jxl-visually-lossless",
    ]
    func index(of name: String) -> Int? { presets.firstIndex { $0.name == name } }

    /// Re-reads the folder when anything in it changed (hand or CLI edits, files dropped in). With no folder,
    /// the built-ins apply until `ensureFile` (at launch) or the first change writes them.
    func reload(force: Bool = false) {
        let fp = PresetFolder.fingerprint(directory, order: orderURL)
        if !force, fp == fingerprint { return }
        if let l = PresetFolder.load(from: directory, order: orderURL) {
            presets = l.presets; problems = l.problems
            saved = Dictionary(l.presets.compactMap { p in p.stableID.map { ($0, p) } }, uniquingKeysWith: { a, _ in a })
        } else {
            presets = Preset.builtIn; problems = []; saved = [:]
        }
        savedOrder = presets.compactMap(\.stableID)
        fingerprint = fp
    }

    /// Writes the folder if it does not exist yet, so the Finder menu and hand editing have files to use.
    func ensureFile() {
        if !FileManager.default.fileExists(atPath: directory.path) { saved = [:]; flush() }
    }

    /// Debounced save of the current `presets`.
    func commit() {
        pendingSave?.cancel()
        pendingSave = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled, let self else { return }
            self.flush()
        }
    }

    /// Writes changed presets, removes deleted ones, and saves the order when it was changed here.
    func flush() {
        pendingSave?.cancel()
        do {
            for p in presets where saved[p.stableID ?? ""] != p { try PresetFolder.write(p, in: directory) }
            let ids = Set(presets.compactMap(\.stableID))
            for id in saved.keys where !ids.contains(id) { try PresetFolder.remove(id, in: directory) }
            let order = presets.compactMap(\.stableID)
            if order != savedOrder { try PresetFolder.writeOrder(order, to: orderURL) }
            saved = Dictionary(presets.compactMap { p in p.stableID.map { ($0, p) } }, uniquingKeysWith: { a, _ in a })
            savedOrder = order; saveError = nil
        } catch { saveError = "Could not save presets: \(error.localizedDescription)" }
        fingerprint = PresetFolder.fingerprint(directory, order: orderURL)
    }

    /// A file name for a new preset: from its name, not used by any file in the folder (even unreadable ones).
    private func newID(for name: String, except: String? = nil) -> String {
        PresetFolder.uniqueID(PresetFolder.slug(name), in: directory, taken: Set(presets.compactMap(\.stableID)).subtracting([except ?? ""]))
    }

    func uniqueName(_ base: String) -> String {
        guard preset(named: base) != nil else { return base }
        var n = 2
        while preset(named: "\(base) \(n)") != nil { n += 1 }
        return "\(base) \(n)"
    }

    @discardableResult
    func add(name: String = "New Preset", options: Options = Options(), showInFinder: Bool = true) -> Preset {
        let n = uniqueName(name)
        let p = Preset(stableID: newID(for: n), name: n, showInFinder: showInFinder, options: options)
        presets.append(p); commit(); return p
    }

    @discardableResult
    func duplicate(_ name: String) -> Preset? {
        guard let src = preset(named: name), let i = index(of: name) else { return nil }
        var copy = src; copy.name = uniqueName("\(src.name) Copy"); copy.stableID = newID(for: copy.name)
        var a = axes(for: src); a.favorite = false; copy.extra["toolbar"] = a.json
        presets.insert(copy, at: i + 1); commit(); return copy
    }

    func delete(_ name: String) {
        presets.removeAll { $0.name == name }; commit()
    }

    func rename(_ old: String, to new: String) -> String {
        guard let i = index(of: old) else { return old }
        let trimmed = new.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty, trimmed == old || preset(named: trimmed) == nil else { return old }
        presets[i].name = trimmed
        // A file named after the preset follows the new name; built-ins and hand-named files keep theirs.
        if let id = presets[i].stableID, id == PresetFolder.slug(old), !PresetFolder.builtinIDs.contains(id) {
            presets[i].stableID = newID(for: trimmed, except: id)
        }
        commit(); return trimmed
    }

    func move(from: IndexSet, to: Int) {
        let moving = from.sorted().map { presets[$0] }
        let before = from.filter { $0 < to }.count
        for i in from.sorted().reversed() { presets.remove(at: i) }
        presets.insert(contentsOf: moving, at: to - before)
        commit()
    }

    /// Replaces every preset file with the 21 built-ins and forgets the saved order. Files that could not be read go too.
    func restoreDefaults() {
        pendingSave?.cancel()
        for u in PresetFolder.files(in: directory) { try? FileManager.default.removeItem(at: u) }
        try? FileManager.default.removeItem(at: orderURL)
        presets = Preset.builtIn; saved = [:]; savedOrder = presets.compactMap(\.stableID)
        flush()
        reload(force: true)
    }

    /// The file behind a preset, if it has been written.
    func fileURL(for p: Preset) -> URL? {
        guard let id = p.stableID else { return nil }
        let u = PresetFolder.url(for: id, in: directory)
        return FileManager.default.fileExists(atPath: u.path) ? u : nil
    }
}
