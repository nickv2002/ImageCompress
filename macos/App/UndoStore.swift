import Foundation
import Observation

/// One finished file that can be put back.
struct UndoEntry: Codable, Identifiable, Equatable, Sendable {
    var id = UUID()
    var date = Date()
    var name: String
    var inBytes: UInt64
    var outBytes: UInt64
    var undo: UndoInfo
    /// The file the job read (optional: entries saved before this was recorded decode without it).
    var source: String?

    /// Where the original is once this entry is undone: put back from its copy, or left where it was.
    var sourcePath: String {
        if let source { return source }
        // older entries: a restore target other than the output is the original that was trashed or deleted;
        // otherwise the job most likely ran in place
        return undo.restore.first { $0.path != undo.output }?.path ?? undo.output
    }
}

/// What to do with a file that is added again.
enum AgainPlan: Equatable {
    /// Nothing known about it: compress it as it is.
    case asNew
    /// It is (or came from) a result that can still be undone: put the original back, then compress `source`.
    case fromOriginal(UndoEntry, source: String)
}

/// Undo history that survives relaunches. Backups are kept for a day, up to 1 GB in total, in a temporary folder
/// so they never outlive a restart, or about three days, even if the app is never opened again.
@MainActor @Observable
final class UndoStore {
    static let shared = UndoStore()
    static let maxAge: TimeInterval = 24 * 3600
    static let maxBytes: UInt64 = 1_000_000_000

    private(set) var entries: [UndoEntry] = []
    private let root: URL
    private let tempRoot: URL

    init(root: URL? = nil, tempRoot: URL? = nil) {
        self.root = root ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("ImageCompress/Undo", isDirectory: true)
        self.tempRoot = tempRoot ?? Self.bootTemporaryRoot()
        load()
        prune()
        sweepOrphans()
    }

    /// Undo copies go in /private/tmp: macOS empties it at startup, and its nightly tmp_cleaner deletes files
    /// untouched for three days, so copies go away even if the app is never opened again. The folder is per user
    /// and private; if one already exists that is not ours, the per-user temp folder (cleaned after three days)
    /// is used instead.
    static func bootTemporaryRoot() -> URL {
        let fm = FileManager.default
        let dir = URL(fileURLWithPath: "/private/tmp/ImageCompressUndo-\(getuid())", isDirectory: true)
        try? fm.createDirectory(at: dir, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        var st = stat()
        if lstat(dir.path, &st) == 0, (st.st_mode & S_IFMT) == S_IFDIR, st.st_uid == getuid(), st.st_mode & 0o077 == 0 {
            return dir
        }
        return fm.temporaryDirectory.appendingPathComponent("ImageCompressUndo", isDirectory: true)
    }

    private var indexURL: URL { root.appendingPathComponent("entries.json") }
    /// Where the engine puts originals.
    var backupDir: URL { tempRoot }
    /// Where backups of local files used to go; swept like any orphan, then removed once empty.
    private var oldBackupDir: URL { root.appendingPathComponent("backups", isDirectory: true) }

    private func load() {
        guard let d = try? Data(contentsOf: indexURL) else { return }
        let dec = JSONDecoder(); dec.dateDecodingStrategy = .iso8601
        entries = (try? dec.decode([UndoEntry].self, from: d)) ?? []
    }
    private func save() {
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let enc = JSONEncoder(); enc.dateEncodingStrategy = .iso8601
        if let d = try? enc.encode(entries) { try? d.write(to: indexURL, options: .atomic) }
    }

    /// Whether every backup this entry needs is still on disk.
    func isAvailable(_ e: UndoEntry) -> Bool {
        e.undo.restore.allSatisfy { FileManager.default.fileExists(atPath: $0.backup) }
    }

    /// Why Undo can't run for this entry, or nil when it can. A cheap size check; the engine also compares hashes.
    func blocker(_ e: UndoEntry) -> String? {
        guard isAvailable(e) else { return "The saved original is gone" }
        let size = (try? URL(fileURLWithPath: e.undo.output).resourceValues(forKeys: [.fileSizeKey]).fileSize).map { UInt64($0) }
        guard let size else { return "The compressed file was moved or deleted" }
        return size == e.undo.outSize ? nil : "The file was changed after it was compressed"
    }

    /// An entry with its output and source paths resolved once (matching runs for every file added).
    struct Keyed: Sendable {
        var entry: UndoEntry, output: String, source: String
    }

    /// The newest entry that can still be undone for a file being added again (`key`: its symlink-resolved
    /// path). First the entry that wrote it (photo.avif made from photo.png, or photo.png compressed in place);
    /// unless `outputsOnly`, then one made from it whose result went elsewhere (the original was kept).
    /// `isLive` says whether Undo can run (see `blocker`). Entries are newest first.
    nonisolated static func plan(forKey k: String, in keyed: [Keyed], outputsOnly: Bool = false,
                                 isLive: (UndoEntry) -> Bool) -> AgainPlan {
        if let m = keyed.first(where: { $0.output == k && isLive($0.entry) }) { return .fromOriginal(m.entry, source: m.entry.sourcePath) }
        if !outputsOnly, let m = keyed.first(where: { $0.source == k && $0.output != k && isLive($0.entry) }) {
            return .fromOriginal(m.entry, source: m.entry.sourcePath)
        }
        return .asNew
    }

    nonisolated static func key(_ p: String) -> String { URL(fileURLWithPath: p).resolvingSymlinksInPath().path }

    /// `plan(forKey:)` for each path against this store, with the cheap on-disk check.
    func plans(for paths: [String], outputsOnly: Bool = false) -> [AgainPlan] {
        guard !entries.isEmpty else { return paths.map { _ in .asNew } }
        let keyed = entries.map { Keyed(entry: $0, output: Self.key($0.undo.output), source: Self.key($0.sourcePath)) }
        return paths.map { Self.plan(forKey: Self.key($0), in: keyed, outputsOnly: outputsOnly, isLive: { blocker($0) == nil }) }
    }

    /// A big run adds hundreds of entries: write the index at most about once a second while they stream in
    /// (prune at the end of the run writes it again). The first add always writes right away.
    func add(_ e: UndoEntry) {
        entries.insert(e, at: 0)
        if Date().timeIntervalSince(lastSave) > 1 { save(); lastSave = Date() }
        else if !saveScheduled {
            saveScheduled = true
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(1))
                saveScheduled = false; save(); lastSave = Date()
            }
        }
    }
    /// Writes the index now (on quit, so throttled entries are not lost and their backups stay prunable).
    func flush() { if saveScheduled || Date().timeIntervalSince(lastSave) <= 1 { save() } }
    private var lastSave = Date.distantPast
    private var saveScheduled = false

    private func drop(_ e: UndoEntry) {
        for r in e.undo.restore {
            try? FileManager.default.removeItem(atPath: r.backup)
            rmdir(URL(fileURLWithPath: r.backup).deletingLastPathComponent().path)   // the per-file slot folder; fails harmlessly unless empty
        }
        entries.removeAll { $0.id == e.id }
    }

    private func size(of e: UndoEntry) -> UInt64 {
        e.undo.restore.reduce(0) { sum, r in
            let n = (try? URL(fileURLWithPath: r.backup).resourceValues(forKeys: [.fileSizeKey]))?.fileSize ?? 0
            return sum + UInt64(n)
        }
    }

    /// Forgets entries older than a day or whose backups vanished, then evicts the oldest until under the size cap.
    func prune(now: Date = Date(), maxBytes: UInt64 = UndoStore.maxBytes) {
        for e in entries where now.timeIntervalSince(e.date) > Self.maxAge || (!e.undo.restore.isEmpty && !isAvailable(e)) { drop(e) }
        var total: UInt64 = 0
        for e in entries.sorted(by: { $0.date > $1.date }) {   // newest first: the oldest are the ones that go
            total += size(of: e)
            if total > maxBytes { drop(e) }
        }
        save()
    }

    /// Removes backup slot folders that no entry points to (left when the app was killed before the history was
    /// saved). Only folders created more than `maxAge` ago, so a run in progress is never touched. Runs at launch.
    func sweepOrphans(now: Date = Date()) {
        let fm = FileManager.default
        let used = Set(entries.flatMap { $0.undo.restore.map { URL(fileURLWithPath: $0.backup).deletingLastPathComponent().standardizedFileURL.path } })
        for dir in [oldBackupDir, backupDir] {
            let slots = (try? fm.contentsOfDirectory(at: dir, includingPropertiesForKeys: [.creationDateKey, .isDirectoryKey])) ?? []
            for slot in slots {
                let v = try? slot.resourceValues(forKeys: [.creationDateKey, .isDirectoryKey])
                guard v?.isDirectory == true, let made = v?.creationDate, now.timeIntervalSince(made) > Self.maxAge,
                      !used.contains(slot.standardizedFileURL.path) else { continue }
                try? fm.removeItem(at: slot)
            }
        }
        rmdir(oldBackupDir.path)
    }

    /// Reverts one file through the engine. Throws the engine's reason when it refuses.
    func undo(_ id: UUID, executable: URL = CLI.executable()) async throws {
        guard let e = entries.first(where: { $0.id == id }) else { throw UndoError.message("This undo is no longer available.") }
        let file = FileManager.default.temporaryDirectory.appendingPathComponent("imgcompress-undo-\(UUID().uuidString).json")
        try JSONEncoder().encode(e.undo).write(to: file)
        defer { try? FileManager.default.removeItem(at: file) }
        let r = await CLI.exec(["undo", file.path], executable: executable)
        guard r.ok else {
            let msg = r.stderr.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "error: ", with: "")
            throw UndoError.message(msg.isEmpty ? "Undo failed." : msg.prefix(1).uppercased() + msg.dropFirst())
        }
        entries.removeAll { $0.id == id }
        save()
        prune()
    }

    /// Names of the volumes holding any of `paths` that have no Trash. One check per volume.
    nonisolated static func volumesWithoutTrash(_ paths: [String]) -> [String] {
        var seen: [URL: Bool] = [:], names: [String] = []
        for p in paths {
            let u = URL(fileURLWithPath: p)
            guard let v = try? u.resourceValues(forKeys: [.volumeURLKey, .volumeNameKey]), let vol = v.volume, seen[vol] == nil else { continue }
            let hasTrash = (try? FileManager.default.url(for: .trashDirectory, in: .userDomainMask, appropriateFor: u, create: false)) != nil
            seen[vol] = hasTrash
            if !hasTrash { names.append(v.volumeName ?? vol.lastPathComponent) }
        }
        return names
    }
}

enum UndoError: Error, LocalizedError {
    case message(String)
    var errorDescription: String? { if case .message(let m) = self { m } else { nil } }
}
