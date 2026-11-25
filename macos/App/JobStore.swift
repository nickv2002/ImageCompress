import Foundation
import Observation
import AppKit

enum JobStatus: Equatable {
    case queued, running(String), done, skipped(String), failed(String)
}

struct JobItem: Identifiable, Equatable {
    let id = UUID()
    var path: String
    var isDirectory: Bool
    var status: JobStatus = .queued
    var inBytes: UInt64?
    var outBytes: UInt64?
    var output: String?
    var trace: [TraceEntry] = []
    var name: String { (path as NSString).lastPathComponent }
    var url: URL { URL(fileURLWithPath: path) }
    var savings: Double? {
        guard let i = inBytes, let o = outBytes, i > 0 else { return nil }
        return (Double(i) - Double(o)) / Double(i)
    }
    var revealURL: URL { URL(fileURLWithPath: output ?? path) }
    /// The undo entry for this result, and whether it has been undone.
    /// A copy of this result's undo entry, so rows never observe the whole undo list (that made every row
    /// re-render on every finished file: quadratic in big runs).
    var undoEntry: UndoEntry?
    var undoID: UUID? { undoEntry?.id }
    var restored = false
    /// The settings of the run that made this result (to offer compressing it again when they change).
    var madeWith: Options?
    /// The symlink-resolved path, worked out once: matching engine events against it is a hot path.
    let key: String

    init(path: String, isDirectory: Bool) {
        self.path = path; self.isDirectory = isDirectory
        key = URL(fileURLWithPath: path).resolvingSymlinksInPath().path
    }
}

@MainActor @Observable
final class JobStore {
    static let shared = JobStore()

    var items: [JobItem] = []
    var presetName: String
    /// One-run overrides edited in the inspector (applied on top of the preset).
    var overrides = Options()
    var isRunning = false
    var totals: Totals?
    var lastError: String?
    /// A heads-up that is not an error (e.g. undo copies of network files are temporary).
    var notice: String?
    var showUndoHistory = false
    private var runOutputs: [URL] = []
    /// Rows of this run that have an undo record, for Edit > Undo.
    private var runUndoable: [UUID] = []
    /// Files of the last run that were put back since (shown in the footer).
    private(set) var undoneInRun = 0
    let undoStore: UndoStore
    /// A notice that must survive the start of the next run (it is about the files that run compresses).
    private var carryNotice: String?
    /// The settings showing when the Compress Again banner was dismissed (or when a Finder hand-off gave the
    /// window its own settings back): the banner stays hidden until they change or another run finishes.
    private var offerDismissedAt: Options?
    var progressDone = 0
    var progressTotal = 0
    private var runner: EngineRunner?
    private var stopRequested = false
    /// Files arrived while a run was in progress: start another run for them when it ends.
    private var runAgain = false
    /// What the window was set to before a Finder hand-off took over the preset.
    private struct Saved { var presetName: String; var overrides: Options; var choice: [String]?; var legacy: String? }
    private var handoffRestore: Saved?
    /// Finder hand-off ids already taken this session.
    private var usedJobs = Set<String>()

    let presets: PresetStore
    let settings: AppSettings

    private let defaults: UserDefaults

    init(presets: PresetStore = .shared, settings: AppSettings = .shared, defaults: UserDefaults = .standard, undoStore: UndoStore = .shared) {
        self.defaults = defaults
        self.undoStore = undoStore
        self.presets = presets
        self.settings = settings
        presetName = settings.defaultPreset
        if presets.preset(named: presetName) == nil { presetName = "" }
        restoreChoice(orDefault: isStock)
    }

    /// The Format/Quality toolbar choice is sticky: the (formatTag, qualityTag) pair is remembered across runs and launches.
    private static let choiceKey = "axisChoice"
    private static let legacyChoiceKey = "simpleChoice"   // pre-presets "keep|lossless" form, migrated on read
    /// What a fresh install (nothing stored) starts on: Preserve format + Lossless.
    static let defaultChoice = AxisSet.defaultChoice
    var axisChoice: (format: String, quality: String)? {
        let d = defaults
        if let a = d.stringArray(forKey: Self.choiceKey), a.count == 2 { return (a[0], a[1]) }
        if let c = d.string(forKey: Self.legacyChoiceKey).flatMap(SimpleChoice.init(stored:)) {
            return (AxisTags.formatTag(c.format), AxisTags.qualityTag(c.quality))
        }
        return nil
    }
    func rememberChoice(format: String, quality: String) {
        defaults.set([format, quality], forKey: Self.choiceKey)
        defaults.removeObject(forKey: Self.legacyChoiceKey)
    }
    /// Applies a favorite as this run's overrides and makes its pair the sticky choice.
    func choose(_ p: AxisPreset) {
        presets.axisSet.apply(p, to: &overrides)
        rememberChoice(format: p.format, quality: p.quality)
    }
    /// Re-applies the sticky pair. If no favorite has exactly that pair any more (it was unfavorited), the
    /// nearest one runs instead and the stored pair is left alone; unfavoriting while a pair is active
    /// never changes the current overrides (the pickers show Custom instead).
    /// `orDefault`: with nothing stored, fall back to Preserve + Lossless (launch only, and only when the stock preset is active).
    /// The sticky pair that Reset puts back for the current preset (only the stock preset falls back to the default pair).
    private var stickyChoice: (format: String, quality: String)? { isStock ? (axisChoice ?? Self.defaultChoice) : axisChoice }
    /// No saved preset underneath: the toolbar's Format and Quality are the whole recipe.
    var isStock: Bool { preset == nil }
    /// True when the inspector holds changes beyond the toolbar's own pair: Reset and Save as Preset have something to do.
    var hasOneRunChanges: Bool { overrides != restingOverrides }
    /// The overrides when nothing was edited: just the toolbar's pair.
    var restingOverrides: Options { presets.axisSet.baseline(stickyChoice) }
    /// The presets menu checks a preset only when it runs exactly as saved (its own format and quality, no edits).
    func isActive(_ p: Preset) -> Bool {
        presetName == p.name && overrides == presets.axisSet.baseline(nil)
    }

    func restoreChoice(orDefault: Bool = false) {
        let set = presets.axisSet
        guard let c = orDefault ? (axisChoice ?? Self.defaultChoice) : axisChoice else { return }
        if let p = set.resolve(format: c.format, quality: c.quality, touched: .format) { set.apply(p, to: &overrides) }
    }

    /// Drops one-run changes and goes back to the toolbar's pair. Same rule as launch: with no stored choice the
    /// stock preset's default pair is what the toolbar shows, so it must come back too (else it reads "Custom").
    func resetOverrides() {
        overrides = Options()
        restoreChoice(orDefault: isStock)
    }

    var preset: Preset? { presetName.isEmpty ? nil : presets.preset(named: presetName) }
    /// How alerts name what is about to run.
    var recipeName: String {
        if let p = preset { return "Preset “\(p.name)”" }
        if let c = axisChoice { return "“\(AxisTags.formatTitle(c.format)), \(AxisTags.qualityTitle(c.quality))”" }
        return "This setting"
    }
    var hasQueued: Bool { items.contains { $0.status == .queued } }
    var queuedCount: Int { items.count { $0.status == .queued } }
    var effectiveOptions: Options { AppSettings.effective(base: settings.base, preset: preset, overrides: overrides) }

    private static func key(_ p: String) -> String { URL(fileURLWithPath: p).resolvingSymlinksInPath().path }

    /// A file the engine found inside a dropped folder, spelled the way the user dropped that folder
    /// (e.g. /tmp/x rather than /private/tmp/x), so rows from the same place show the same folder.
    static func spelled(_ path: String, like folders: [String]) -> String {
        let k = key(path)
        for f in folders {
            let fk = key(f)
            if k.hasPrefix(fk + "/") { return f + k.dropFirst(fk.count) }
        }
        return path
    }

    /// Queues files. A file already in the list is queued again once it has finished (a folder, or a file
    /// that is waiting or running, is left as it is). `prepareAgain` has already put back the originals it could.
    func add(_ urls: [URL]) {
        var asIs: [String] = []
        for u in urls where u.isFileURL {
            let k = Self.key(u.path)
            // an earlier result whose original could not be put back is compressed as it is: say so
            if items.contains(where: { $0.status == .done && !$0.restored && $0.output.map(Self.key) == k }) { asIs.append(u.lastPathComponent) }
            if let n = items.firstIndex(where: { $0.key == k }) {
                let row = items[n]
                guard !row.isDirectory, isFinished(row.status) else { continue }
                items[n] = JobItem(path: row.path, isDirectory: false)
                continue
            }
            let dir = (try? u.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) ?? false
            items.append(JobItem(path: u.path, isDirectory: dir))
        }
        if !asIs.isEmpty { sayCompressedAsIs(asIs) }
    }

    /// Drops, Add Files and Open With: queue the files and start right away (the Process button remains for
    /// anything left queued, e.g. after cancelling the Trash confirmation). Files added mid-run join the next run.
    /// Earlier results that can still be undone are put back to their originals first (see `prepareAgain`).
    func ingest(_ urls: [URL]) {
        // A dragged screenshot thumbnail first moves to where screenshots are saved.
        prepareAgain(urls.map { ScreenshotDrop.adopt($0) }) { [self] urls in
            add(urls)
            guard settings.autoProcess else { return }
            if isRunning { runAgain = hasQueued } else { process() }
        }
    }

    /// Removes finished/queued rows (never ones that are mid-run).
    func remove(ids: Set<UUID>) {
        items.removeAll { ids.contains($0.id) && !isRunningStatus($0.status) }
        if items.isEmpty { totals = nil; lastError = nil }
    }

    func clear() { if !isRunning { items.removeAll(); totals = nil; lastError = nil; progressDone = 0; progressTotal = 0 } }

    /// Dock drops, Open With, Services and imagecompress://job/<uuid> all end up here.
    func handleOpen(_ urls: [URL]) {
        var files: [URL] = []
        for u in urls {
            if u.scheme == AppGroup.urlScheme {
                let id = u.lastPathComponent
                guard u.host == "job", let job = JobFile.take(uuid: id) else {
                    // The same link can arrive twice (Apple Event and open-URLs); only an unseen one is an error.
                    if !usedJobs.contains(id) { notice = "That ImageCompress link has expired or was already used. Choose the action in Finder again." }
                    continue
                }
                usedJobs.insert(id)
                presets.reload()
                // The hand-off runs with its own preset, then the window goes back to what the user had chosen.
                if handoffRestore == nil {
                    handoffRestore = .init(presetName: presetName, overrides: overrides,
                                           choice: defaults.stringArray(forKey: Self.choiceKey), legacy: defaults.string(forKey: Self.legacyChoiceKey))
                }
                // A handed-off preset starts from itself: the toolbar's one-run overrides must not leak into it.
                if let p = job.preset {
                    if let resolved = presets.resolve(p) { choosePreset(resolved.name) }
                    else { notice = "The preset “\(p)” no longer exists, so the current settings were used." }
                }
                if let o = job.overrides { overrides = Options(o) }
                let start = job.start == true
                prepareAgain(job.paths.map { URL(fileURLWithPath: $0) }) { [self] urls in
                    add(urls)
                    if start { if isRunning { runAgain = true } else if hasQueued { process() } else { restoreAfterHandoff() } }
                }
            } else {
                files.append(u)
            }
        }
        if !files.isEmpty { ingest(files) }
    }

    private func restoreAfterHandoff() {
        guard let r = handoffRestore else { return }
        handoffRestore = nil
        presetName = r.presetName; overrides = r.overrides
        if let c = r.choice { defaults.set(c, forKey: Self.choiceKey) }
        if let l = r.legacy { defaults.set(l, forKey: Self.legacyChoiceKey) }
        offerDismissedAt = effectiveOptions   // the window's own settings coming back is no change to point out
    }

    func process() {
        guard !isRunning, hasQueued else { return }
        presets.reload()
        var options = effectiveOptions
        let targets = items.filter { $0.status == .queued }
        // Cancelling the Trash/Delete confirmation of a hand-off also drops its preset.
        switch confirmOriginals(options, paths: targets.map(\.path)) {
        case .cancel: restoreAfterHandoff(); return
        case .keepOriginals: options["originals"] = .string("keep")
        case .proceed: break
        }
        let targetIDs = Set(targets.map(\.id))
        runOptions = effectiveOptions
        let runner = EngineRunner(executable: EngineRunner.bundledExecutable())
        self.runner = runner
        isRunning = true; totals = nil; lastError = nil; notice = carryNotice; carryNotice = nil; stopRequested = false; runOutputs = []; runUndoable = []; undoneInRun = 0
        let backupDir = settings.keepUndo ? undoStore.backupDir : nil
        progressDone = 0; progressTotal = targets.count
        Feedback.update(done: 0, total: progressTotal)
        let stream = runner.run(paths: targets.map(\.path), options: options, backupDir: backupDir)
        Task {
            do {
                // Events are applied in batches about ten times a second: one list update per batch instead of
                // three per file keeps the window responsive in big runs.
                let flusher = Task { @MainActor [weak self] in
                    while !Task.isCancelled { try? await Task.sleep(for: .milliseconds(100)); self?.flushEvents() }
                }
                defer { flusher.cancel() }
                for try await e in stream { pendingEvents.append(e) }
            } catch {
                lastError = error.localizedDescription
            }
            flushEvents()
            for i in items.indices {
                // only this run's folders: one dropped mid-run is still waiting for the next run
                if items[i].isDirectory, targetIDs.contains(items[i].id), items[i].status == .queued || isRunningStatus(items[i].status) {
                    // a stopped folder only had part of its files processed: don't call it done
                    items[i].status = stopRequested ? .skipped("Stopped before every file was processed") : .done
                }
                else if isRunningStatus(items[i].status) { items[i].status = .failed("Stopped") }
            }
            // A non-zero exit just means some images failed; say so plainly instead of the raw status.
            let failed = items.filter { if case .failed = $0.status { return !$0.isDirectory } else { return false } }.count
            if lastError != nil, failed > 0 { lastError = nil; if totals == nil { let done = items.filter { $0.status == .done && !$0.isDirectory }; totals = Totals(processed: done.count, skipped: 0, failed: failed, inBytes: done.reduce(0) { $0 + ($1.inBytes ?? 0) }, outBytes: done.reduce(0) { $0 + ($1.outBytes ?? 0) }) } }
            // Results arrive in completion order; settle into a stable, Finder-like order.
            items.sort { $0.path.localizedStandardCompare($1.path) == .orderedAscending }
            isRunning = false
            self.runner = nil
            // Inspector changes stay for the next drop (every drop is a run now); only a Finder hand-off is undone.
            offerDismissedAt = nil   // new results: a dismissed offer may show again
            if !(runAgain && !stopRequested) { restoreAfterHandoff() }
            undoStore.prune()
            registerRunUndo(runUndoable)
            Feedback.finish(totals: totals, error: lastError, revealing: runOutputs)
            if runAgain && !stopRequested { runAgain = false; process() } else { runAgain = false }
        }
    }

    /// Picking a preset starts from that preset: one-run changes made in the inspector are dropped,
    /// so the Goal control and the toolbar always agree.
    func choosePreset(_ name: String) {
        guard name != presetName || !overrides.values.isEmpty else { return }
        presetName = name
        overrides = Options()
        defaults.removeObject(forKey: Self.choiceKey)   // the preset's own format/quality win
        defaults.removeObject(forKey: Self.legacyChoiceKey)
    }

    /// Toolbar title: the preset, marked when the inspector has changed anything on top of it.
    var presetTitle: String { !hasOneRunChanges ? presetName : "\(presetName) + changes" }
    /// Back to no saved preset: just the toolbar's Format and Quality.
    func chooseNoPreset() {
        guard !presetName.isEmpty else { return }
        presetName = ""; overrides = Options(); restoreChoice(orDefault: true)
    }

    /// Puts one finished file back the way it was.
    func undo(_ id: UUID) { undo([id]) }

    /// The history sheet undid an entry: update its row, if it is in the list.
    func markRestored(undoID: UUID) {
        if let m = items.firstIndex(where: { $0.undoID == undoID }) { items[m].restored = true; items[m].undoEntry = nil }
    }

    /// Rows that can still be put back.
    func canUndo(_ ids: [UUID]) -> Bool { items.contains { ids.contains($0.id) && $0.undoID != nil && !$0.restored } }

    /// Puts several files back, one after another; rows already restored are skipped.
    func undo(_ ids: [UUID]) {
        Task {
            var failures: [String] = []
            for id in ids {
                guard let n = items.firstIndex(where: { $0.id == id }), let uid = items[n].undoID, !items[n].restored else { continue }
                let name = items[n].name
                do {
                    try await undoStore.undo(uid)
                    if let m = items.firstIndex(where: { $0.id == id }) {
                        items[m].restored = true; items[m].undoEntry = nil
                        // the footer describes the last run: take this file back out of its savings
                        if runUndoable.contains(id), var t = totals {
                            t.processed = max(t.processed - 1, 0); undoneInRun += 1
                            t.inBytes -= min(t.inBytes, items[m].inBytes ?? 0); t.outBytes -= min(t.outBytes, items[m].outBytes ?? 0)
                            totals = t
                        }
                    }
                } catch { failures.append("\(name): \(error.localizedDescription)") }
            }
            lastError = failures.isEmpty ? nil : "Could not undo " + failures.joined(separator: "; ")
        }
    }

    /// Edit > Undo after a run: "Undo Compress 5 Images". Registered on the main window so ⌘Z works like Finder's.
    private func registerRunUndo(_ ids: [UUID]) {
        guard !ids.isEmpty else { return }
        let window = NSApp.windows.first { $0.identifier?.rawValue.hasPrefix("main") == true } ?? NSApp.mainWindow
        guard let um = window?.undoManager else { return }
        um.registerUndo(withTarget: self) { store in MainActor.assumeIsolated { store.undo(ids) } }
        let one = ids.count == 1 ? items.first { $0.id == ids[0] }?.name : nil
        um.setActionName(one.map { "Compress “\($0)”" } ?? "Compress \(ids.count) Images")
    }

    func stop() { stopRequested = true; runner?.stop() }

    enum OriginalsDecision { case proceed, keepOriginals, cancel }

    /// Asks before originals are deleted or trashed. When some files sit on a volume with no Trash (most network
    /// shares), says so up front and offers to keep the originals, instead of every conversion there failing.
    private func confirmOriginals(_ o: Options, paths: [String]) -> OriginalsDecision {
        let mode = o.originalsMode
        if mode == "trash", o.leavesOriginals, let volume = UndoStore.volumesWithoutTrash(paths).first {
            let alert = NSAlert()
            alert.messageText = "“\(volume)” has no Trash"
            alert.informativeText = "\(recipeName) moves originals to the Trash after converting them, which this volume (usually a network share) does not allow, so those files would fail. Keep the originals next to the new files for this run instead?"
            alert.addButton(withTitle: "Keep Originals")
            alert.addButton(withTitle: "Cancel")
            return alert.runModal() == .alertFirstButtonReturn ? .keepOriginals : .cancel
        }
        return confirmIfDestructive(o) ? .proceed : .cancel
    }

    private func confirmIfDestructive(_ o: Options) -> Bool {
        let mode = o.originalsMode
        guard mode == "delete" || (mode == "trash" && settings.confirmTrash) else { return true }
        let alert = NSAlert()
        alert.messageText = mode == "delete" ? "Permanently delete the original files?" : "Move the original files to the Trash?"
        alert.informativeText = "\(recipeName) replaces originals after a successful conversion."
        alert.addButton(withTitle: "Continue")
        alert.addButton(withTitle: "Cancel")
        if mode == "delete" { alert.alertStyle = .warning }
        return alert.runModal() == .alertFirstButtonReturn
    }

    private func isRunningStatus(_ s: JobStatus) -> Bool { if case .running = s { return true } else { return false } }
    private func isFinished(_ s: JobStatus) -> Bool { s != .queued && !isRunningStatus(s) }

    private func index(for path: String) -> Int {
        if let i = items.lastIndex(where: { $0.path == path }) { return i }   // same spelling: no file-system call
        let k = Self.key(path)
        if let i = items.firstIndex(where: { $0.key == k }) { return i }
        items.append(JobItem(path: Self.spelled(path, like: items.filter(\.isDirectory).map(\.path)), isDirectory: false))
        return items.count - 1
    }

    private func tick() {
        progressDone += 1
        Feedback.update(done: progressDone, total: max(progressTotal, progressDone))
    }

    private var pendingEvents: [JobEvent] = []
    /// The settings of the run in progress as the window had them (before a Keep Originals fallback).
    private var runOptions: Options?
    private func flushEvents() {
        guard !pendingEvents.isEmpty else { return }
        let batch = pendingEvents; pendingEvents = []
        for e in batch { apply(e) }
    }

    func apply(_ e: JobEvent) {
        switch e {
        case .queued(let n): progressTotal = max(progressTotal, n)
        case .started(let p): items[index(for: p)].status = .running("Starting")
        case .stage(let p, let s): items[index(for: p)].status = .running(s)
        case .finished(let p, let out, let i, let o, _, let trace, let undo):
            let n = index(for: p)
            items[n].status = .done; items[n].inBytes = i; items[n].outBytes = o; items[n].output = out; items[n].trace = trace
            items[n].madeWith = runOptions
            runOutputs.append(URL(fileURLWithPath: out))
            if let undo {
                let e = UndoEntry(name: items[n].name, inBytes: i, outBytes: o, undo: undo, source: p)
                undoStore.add(e); items[n].undoEntry = e; items[n].restored = false; runUndoable.append(items[n].id)
            }
            tick()
        case .skipped(let p, let r): items[index(for: p)].status = .skipped(r); tick()
        case .failed(let p, let err): items[index(for: p)].status = .failed(err); tick()
        case .totals(let t): totals = t
        case .unknown: break
        }
    }

    // MARK: Compressing again from the original

    /// A file added again whose earlier result can still be undone (it is that result, or the original it was
    /// made from) is put back to its original first, so the original, not the earlier result, is compressed
    /// with the current settings. Then `body` gets the files to queue; with no such file it runs right away.
    func prepareAgain(_ urls: [URL], executable: URL? = nil, then body: @escaping @MainActor ([URL]) -> Void) {
        let busy = Set(items.filter { !isFinished($0.status) }.map(\.key))
        let files = urls.filter { $0.isFileURL && !busy.contains(Self.key($0.path)) }
        let found = undoStore.plans(for: files.map(\.path))
        guard found.contains(where: { $0 != .asNew }) else { body(urls); return }
        let plan = Dictionary(zip(files, found), uniquingKeysWith: { a, _ in a })
        Task { body(await restoreOriginals(urls.map { ($0, plan[$0] ?? .asNew) }, executable: executable)) }
    }

    /// Puts the originals back for `plans` and returns the files to queue: each original in place of the file
    /// that was added. A file whose original the engine refuses to put back (it checks more than `blocker`
    /// does) is left exactly as it is and not queued; the reason is shown in the footer.
    func restoreOriginals(_ plans: [(URL, AgainPlan)], executable: URL? = nil) async -> [URL] {
        let exe = executable ?? CLI.executable()
        var out: [URL] = [], failures: [String] = []
        for (u, plan) in plans {
            guard case .fromOriginal(let e, var source) = plan else { out.append(u); continue }
            do { try await undoStore.undo(e.id, executable: exe) }
            catch { failures.append("“\(u.lastPathComponent)”: \(error.localizedDescription)"); continue }
            var undone = [e]
            // The original may itself be an earlier result (compressed again before this existed): go back to the first.
            for _ in 0..<8 {
                guard case .fromOriginal(let older, let s) = undoStore.plans(for: [source], outputsOnly: true)[0],
                      (try? await undoStore.undo(older.id, executable: exe)) != nil else { break }
                undone.append(older); source = s
            }
            resetRows(undone: undone, source: source)
            out.append(URL(fileURLWithPath: source))
        }
        // A notice, not lastError: the run that starts next for the other files would clear an error.
        if !failures.isEmpty {
            let say = "Could not put the original back, so nothing was compressed or changed for " + failures.joined(separator: "; ")
            notice = [carryNotice, say].compactMap { $0 }.joined(separator: " ")
            carryNotice = notice
        }
        return out
    }

    /// The rows of the results just undone become one waiting row for their original.
    private func resetRows(undone: [UndoEntry], source: String) {
        let ids = Set(undone.map(\.id)), src = Self.key(source)
        let gone = Set(undone.map { Self.key($0.undo.output) })
        let hits = items.indices.filter { i in
            let r = items[i]
            return !r.isDirectory && !isRunningStatus(r.status) && (r.undoID.map(ids.contains) == true || r.key == src || gone.contains(r.key))
        }
        guard let first = hits.first else { return }
        let path = items[first].key == src ? items[first].path : source
        for i in hits.dropFirst().reversed() { items.remove(at: i) }
        items[first] = JobItem(path: path, isDirectory: false)
    }

    private func sayCompressedAsIs(_ names: [String]) {
        let one = names.count == 1
        let what = one ? "“\(names[0])” is" : "\(names.count) images are"
        let why = !settings.keepUndo ? "Undo copies are turned off, so no original was kept"
            : one ? "its original copy is no longer available" : "their original copies are no longer available"
        notice = [carryNotice, "\(what) compressed again as it is now: \(why)."].compactMap { $0 }.joined(separator: " ")
        carryNotice = notice
    }

    /// What Compress Again from Original does for a row, or nil when it can't.
    private func againPlan(for r: JobItem, live: Set<UUID>? = nil) -> AgainPlan? {
        guard !r.isDirectory, r.status == .done else { return nil }
        if r.restored { return .asNew }   // its original is already back
        guard let e = r.undoEntry, live?.contains(e.id) ?? undoStore.entries.contains(where: { $0.id == e.id }),
              undoStore.blocker(e) == nil else { return nil }
        return .fromOriginal(e, source: e.sourcePath)
    }

    func canCompressAgain(_ ids: [UUID]) -> Bool { items.contains { ids.contains($0.id) && againPlan(for: $0) != nil } }

    /// Compress Again from Original: puts the originals of these finished rows back and compresses them with
    /// the current settings. Rows whose original can't be put back are left alone.
    func compressAgain(_ ids: [UUID]) {
        let plans = items.filter { ids.contains($0.id) }.compactMap { r in againPlan(for: r).map { (r.url, $0) } }
        guard !plans.isEmpty else { return }
        Task {
            let urls = await restoreOriginals(plans)
            guard !urls.isEmpty else { return }
            add(urls)
            if isRunning { runAgain = hasQueued } else { process() }
        }
    }

    /// Rows the settings banner offers to compress again: finished with other settings than the current ones,
    /// and still undoable. Empty during a run, and after the banner was dismissed until the settings change.
    var redoOffer: [UUID] {
        guard !isRunning, items.contains(where: { $0.madeWith != nil }) else { return [] }
        let now = effectiveOptions
        guard now != offerDismissedAt else { return [] }
        let live = Set(undoStore.entries.map(\.id))
        return items.filter { r in
            guard let m = r.madeWith, m != now, !r.restored, case .fromOriginal = againPlan(for: r, live: live) else { return false }
            return true
        }.map(\.id)
    }

    /// Hides the banner until the settings change again or another run finishes.
    func dismissRedoOffer() { offerDismissedAt = effectiveOptions }

    func addViaPanel() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = true
        panel.canChooseFiles = true
        if panel.runModal() == .OK { ingest(panel.urls) }
    }
}
