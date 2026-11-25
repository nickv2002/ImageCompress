import Foundation
import Testing

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private let cliBinary = repoRoot.appendingPathComponent("target/release/imgcompress")
private func tempDir() throws -> URL {
    let d = FileManager.default.temporaryDirectory.appendingPathComponent("ic-test-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
    return d
}

struct EventTests {
    @Test func parsesEvents() throws {
        let lines = [
            #"{"event":"queued","files":2}"#,
            #"{"event":"started","path":"/a.png"}"#,
            #"{"event":"stage","path":"/a.png","stage":"quantize"}"#,
            #"{"event":"finished","path":"/a.png","output":"/a.png","in_bytes":1000,"out_bytes":400,"encoder":"oxipng","trace":[["decode",1000],["oxipng",400]]}"#,
            #"{"event":"skipped","path":"/b.png","reason":"larger"}"#,
            #"{"event":"failed","path":"/c.png","error":"bad"}"#,
            #"{"event":"totals","processed":1,"skipped":1,"failed":1,"in_bytes":1000,"out_bytes":400}"#,
            #"{"event":"future_thing","x":1}"#,
        ]
        let ev = lines.compactMap(JobEvent.parse(line:))
        #expect(ev.count == 8)
        #expect(ev[0] == .queued(files: 2))
        #expect(ev[3] == .finished(path: "/a.png", output: "/a.png", inBytes: 1000, outBytes: 400, encoder: "oxipng",
                                   trace: [TraceEntry(stage: "decode", bytes: 1000), TraceEntry(stage: "oxipng", bytes: 400)]))
        #expect(ev[6] == .totals(Totals(processed: 1, skipped: 1, failed: 1, inBytes: 1000, outBytes: 400)))
        #expect(ev[7] == .unknown("future_thing"))
        #expect(JobEvent.parse(line: "not json") == nil)
    }

    @Test func splitsPartialLines() {
        var s = LineSplitter()
        #expect(s.feed(Data("{\"a\":".utf8)).isEmpty)
        #expect(s.feed(Data("1}\n{\"b\":2}\n{\"c".utf8)) == ["{\"a\":1}", "{\"b\":2}"])
        #expect(s.flush() == "{\"c")
    }
}

struct OptionsTests {
    @Test func partialRoundTripKeepsNullAndUnknownKeys() throws {
        let json = #"{"format":"jxl","jpeg-quality":82,"webp-quality":null,"future":{"a":[1,"x"]},"png-quantize":[60,80]}"#
        let o = try JSONDecoder().decode(Options.self, from: Data(json.utf8))
        #expect(o.string("format") == "jxl")
        #expect(o.int("jpeg-quality") == 82)
        #expect(o["webp-quality"] == .null)       // explicit null (lossless) is not "absent"
        #expect(o["recursive"] == nil)            // partial: nothing invented
        #expect(o["future"] != nil)
        let back = try JSONDecoder().decode(Options.self, from: JSONEncoder().encode(o))
        #expect(back == o)
    }

    @MainActor @Test func layering() {
        let base = Options(["recursive": .bool(false), "jpeg-quality": .int(80)])
        let preset = Preset(name: "P", options: Options(["jpeg-quality": .int(70), "originals": .string("trash")]))
        let over = Options(["jpeg-quality": .null])
        let eff = AppSettings.effective(base: base, preset: preset, overrides: over)
        #expect(eff["recursive"] == .bool(false))
        #expect(eff["jpeg-quality"] == .null)
        #expect(eff.originalsMode == "trash" && eff.isDestructive)
    }

    @Test func schemaParsing() throws {
        let json = #"""
        {"properties":{"format":{"$ref":"#/$defs/Format","default":"keep"},
          "metadata":{"$ref":"#/$defs/Metadata","default":"icc"},
          "jpeg-quality":{"type":["integer","null"],"default":null,"description":"d"}},
         "$defs":{"Format":{"type":"string","enum":["keep","jpeg"]},
           "Metadata":{"oneOf":[{"type":"string","const":"none"},{"type":"string","const":"icc"}]}}}
        """#
        let s = try #require(OptionsSchema.parse(Data(json.utf8)))
        #expect(s.properties["format"]?.enumValues == ["keep", "jpeg"])
        #expect(s.properties["metadata"]?.enumValues == ["none", "icc"])
        #expect(s.defaultValue("jpeg-quality") == .null)
        #expect(s.has("format") && !s.has("nope"))
    }

    @Test func simpleChoiceStoredMigrates() {
        for f in [SimpleChoice.Format.keep] + SimpleChoice.targetFormats.map({ .change($0.id) }) {
            for q in SimpleChoice.Quality.allCases {
                let c = SimpleChoice(format: f, quality: q)
                #expect(SimpleChoice(stored: c.stored) == c)
            }
        }
        #expect(SimpleChoice(stored: "bogus|lossless") == nil)
    }

    private static func shipped() -> AxisSet {
        AxisSet(items: Preset.shippedAxisPresets.map { p in
            let a = Preset.shippedAxes(stableID: p.stableID)!
            return AxisPreset(preset: p, format: a.format!, quality: a.quality!)
        })
    }

    @Test func shippedPresetsMatchOldChoices() {
        let set = Self.shipped()
        #expect(set.items.count == 21)
        #expect(Set(set.items.map(\.preset.name)).count == 21 && Set(set.items.compactMap(\.preset.stableID)).count == 21)
        #expect(set.formats == ["keep", "jxl", "avif", "heic", "webp", "jpeg", "png"])
        #expect(set.qualities == ["lossless", "visually-lossless", "smaller"])
        for ap in set.items {
            #expect(ap.preset.showInFinder == Preset.finderDefaults.contains(ap.preset.stableID!) && ap.preset.options["originals"] == nil)
            #expect(PresetAxes(json: ap.preset.extra["toolbar"]) == PresetAxes(format: ap.format, quality: ap.quality, favorite: true))
            // identical to the old hard-coded SimpleChoice values
            let q = SimpleChoice.Quality.allCases.first { AxisTags.qualityTag($0) == ap.quality }!
            let f: SimpleChoice.Format = ap.format == "keep" ? .keep : .change(ap.format)
            #expect(ap.preset.options.values == SimpleChoice(format: f, quality: q).overrides)
        }
    }

    /// The default state (stock preset + default pair) must read as "no changes": Reset and Save as Preset
    /// disabled, the presets menu checks the active preset.
    @Test func baselineIsWhatTheToolbarPairWrites() {
        let set = Self.shipped()
        #expect(set.baseline(nil) == Options())
        var o = Options()
        set.apply(set.exact(format: "keep", quality: "lossless")!, to: &o)
        #expect(set.baseline(AxisSet.defaultChoice) == o)
        o["jpeg-quality"] = .int(70)   // an inspector edit on top of the pair
        #expect(set.baseline(AxisSet.defaultChoice) != o)
        // a pair that is no longer a favorite falls back the same way launch does
        #expect(set.baseline((format: "jxl", quality: "nonexistent")) == { var x = Options(); set.apply(set.resolve(format: "jxl", quality: "nonexistent", touched: .format)!, to: &x); return x }())
    }

    @Test func applyThenReadRoundTrips() {
        let set = Self.shipped()
        for ap in set.items {
            var o = Options(["jpeg-quality": .int(85), "originals": .string("trash"), "avif-quality": .int(30)])
            set.apply(ap, to: &o)
            #expect(set.matching(o)?.preset.name == ap.preset.name)
            #expect(o.string("originals") == "trash")  // never touches originals
        }
        #expect(set.matching(Options(["format": .string("tiff")])) == nil)
        #expect(set.matching(Options())?.preset.stableID == "keep-lossless")   // absent == null, absent format == keep
        let tiers = ["jxl", "avif", "heic", "keep"].map { f in Set(AxisTags.knownQualities.map { set.exact(format: f, quality: $0)!.preset.options.values.description }).count }
        #expect(tiers == [3, 3, 3, 3])
        let vl = set.exact(format: "heic", quality: "visually-lossless")!.preset.options
        #expect(vl["heic-quality"] == .int(90))
        #expect(set.exact(format: "avif", quality: "lossless")!.preset.options["avif-quality"] == .int(100))
        let keepVL = set.exact(format: "keep", quality: "visually-lossless")!.preset.options
        #expect(keepVL["target-quality"] == .int(88) && keepVL["png-quantize"] == .array([.int(80), .int(95)]))
        #expect(set.exact(format: "keep", quality: "smaller")!.preset.options["target-quality"] == .int(70))
    }

    @Test func favoritesDriveTheAxes() {
        var set = Self.shipped()
        // unfavoriting every "smaller" removes that quality; a custom pair appears after the known ones
        set.items.removeAll { $0.quality == "smaller" }
        #expect(set.qualities == ["lossless", "visually-lossless"])
        let custom = AxisPreset(preset: Preset(name: "Web", options: Options(["format": .string("webp"), "webp-quality": .int(60)])), format: "webp", quality: "web")
        set.items.append(custom)
        #expect(set.qualities == ["lossless", "visually-lossless", "web"])
        #expect(set.resolve(format: "webp", quality: "web", touched: .quality)?.preset.name == "Web")
        // missing pair: touched axis keeps its value, the other moves to the nearest tag
        #expect(set.resolve(format: "avif", quality: "web", touched: .quality)?.format == "webp")
        #expect(set.resolve(format: "webp", quality: "visually-lossless", touched: .format)?.quality == "visually-lossless")
        set.items.removeAll { $0.format == "png" && $0.quality == "lossless" }
        #expect(set.resolve(format: "png", quality: "lossless", touched: .format)?.quality == "visually-lossless")
        // custom format tags sort after the classic ones
        set.items.append(AxisPreset(preset: Preset(name: "Tiny"), format: "tiny", quality: "lossless"))
        #expect(set.formats.last == "tiny")
        #expect(AxisTags.formatGroup("tiny") == 3 && AxisTags.formatTitle("tiny") == "tiny" && AxisTags.formatTitle("avif") == "Convert to AVIF")
        // switching presets clears the keys a previous favorite set (here webp-quality)
        var o = Options()
        set.apply(custom, to: &o)
        #expect(o["webp-quality"] == .int(60))
        set.apply(set.exact(format: "keep", quality: "lossless")!, to: &o)
        #expect(o["webp-quality"] == .null)
    }
}

struct PresetTests {
    @Test func fileRoundTripPreservesUnknownKeys() throws {
        let dir = try tempDir().appendingPathComponent("Presets")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try Data(#"{"name":"Lossy","show_in_finder":true,"shortcut":"1","options":{"png-quantize":[65,85],"jpeg-quality":null}}"#.utf8)
            .write(to: dir.appendingPathComponent("lossy.json"))
        try Data(#"{"options":{}}"#.utf8).write(to: dir.appendingPathComponent("Hidden.json"))
        try Data("{".utf8).write(to: dir.appendingPathComponent(".tmp-x.json"))   // temp files are skipped
        let l = try #require(PresetFolder.load(from: dir, order: dir.appendingPathComponent("none")))
        #expect(l.problems.isEmpty)
        #expect(l.presets.map(\.name) == ["Hidden", "Lossy"])   // not built-ins: by name; a missing name is the file name
        let lossy = l.presets[1]
        #expect(lossy.showInFinder && !l.presets[0].showInFinder && lossy.stableID == "lossy")
        #expect(lossy.options["jpeg-quality"] == .null && lossy.extra["shortcut"] == .string("1"))
        try PresetFolder.write(lossy, in: dir)
        let raw = try JSONSerialization.jsonObject(with: Data(contentsOf: dir.appendingPathComponent("lossy.json"))) as! [String: Any]
        #expect(raw["id"] == nil && raw["show_in_finder"] as? Bool == true && raw["shortcut"] as? String == "1")
        #expect(try PresetFolder.read(dir.appendingPathComponent("lossy.json")) == lossy)
        #expect(Set(try FileManager.default.contentsOfDirectory(atPath: dir.path)) == ["lossy.json", "Hidden.json", ".tmp-x.json"])  // no temp files left
    }

    @Test func builtInsAreTheToolbarPresets() {
        #expect(Preset.builtIn.count == 21)
        #expect(Preset.builtIn.allSatisfy { !$0.isDestructive })
        #expect(Preset.builtIn.filter(\.showInFinder).compactMap(\.stableID) == ["keep-lossless", "keep-smaller", "jxl-visually-lossless"])
    }

    /// A broken or clashing file is listed as a problem, kept out of the list, and never written over.
    @MainActor @Test func badFilesAreSkippedNotOverwritten() throws {
        let root = try tempDir(), dir = root.appendingPathComponent("Presets")
        let store = PresetStore(directory: dir, order: root.appendingPathComponent("preset-order.json"))
        store.ensureFile()
        let broken = #"{"name":"X" "options":{}}"#
        try Data(broken.utf8).write(to: dir.appendingPathComponent("x.json"))
        try Data(#"{"name":"preserve · lossless"}"#.utf8).write(to: dir.appendingPathComponent("twin.json"))
        store.reload()
        #expect(store.presets.count == 21 && store.problems.map(\.file).sorted() == ["twin.json", "x.json"] && store.loadError != nil)
        let added = store.add(name: "X"); store.flush()
        #expect(added.stableID == "x-2")
        #expect(try String(contentsOf: dir.appendingPathComponent("x.json"), encoding: .utf8) == broken)   // untouched
        store.restoreDefaults()
        #expect(store.presets == Preset.builtIn && store.problems.isEmpty && PresetFolder.files(in: dir).count == 21)
    }

    @Test func originalsOnlyMatterForNewFiles() {
        #expect(!Options(["destination": .string("in-place"), "format": .string("keep")]).leavesOriginals)
        #expect(!Options().leavesOriginals)
        #expect(Options(["destination": .string("alongside")]).leavesOriginals)
        #expect(Options(["destination": .string("in-place"), "format": .string("jxl")]).leavesOriginals)
        #expect(Options(["destination": .string("/tmp/out")]).leavesOriginals)
    }

    @Test func localVolumesHaveATrash() throws {
        let dir = try tempDir()
        #expect(UndoStore.volumesWithoutTrash([dir.path, dir.appendingPathComponent("missing.png").path]).isEmpty)
    }

    @MainActor @Test func storeEditing() throws {
        let root = try tempDir(), dir = root.appendingPathComponent("Presets"), order = root.appendingPathComponent("preset-order.json")
        let store = PresetStore(directory: dir, order: order)   // no folder yet: the built-ins, nothing written
        #expect(store.presets.count == 21 && store.plainPresets.isEmpty && store.axisSet.items.count == 21)
        #expect(store.resolve("jpg")?.stableID == "jpeg-visually-lossless" && store.resolve("preserve · smaller")?.stableID == "keep-smaller")
        #expect(!FileManager.default.fileExists(atPath: dir.path))
        store.ensureFile()
        #expect(PresetFolder.files(in: dir).count == 21 && !FileManager.default.fileExists(atPath: order.path))
        let dup = try #require(store.duplicate("Preserve · Smaller"))
        #expect(dup.stableID == "preserve-smaller-copy")   // its own file, not the original's
        #expect(dup.name == "Preserve · Smaller Copy" && store.index(of: dup.name) == (store.index(of: "Preserve · Smaller") ?? 0) + 1)
        #expect(store.axes(for: dup).favorite == false && store.axes(for: dup).quality == "smaller")   // a copy is not in the toolbar
        let added = store.add(name: "Mine")
        #expect(store.add(name: "Mine").name == "Mine 2")
        #expect(store.plainPresets.map(\.name) == ["Mine", "Mine 2"])
        store.move(from: IndexSet(integer: store.index(of: "Mine")!), to: 0)
        #expect(store.presets[0].name == "Mine")
        // tags and favorites: stored in the preset, survive a rename
        store.setAxes(PresetAxes(format: "webp", quality: "web", favorite: true), for: added)
        #expect(store.axisSet.items.count == 22 && store.plainPresets.map(\.name) == ["Mine 2"])
        _ = store.rename("Mine", to: "Web")
        #expect(store.preset(named: "Web")?.stableID == "web")   // the file follows a name it was made from
        _ = store.rename("Preserve · Lossless", to: "Plain")
        #expect(store.preset(named: "Plain")?.stableID == "keep-lossless")   // built-ins keep their ids
        #expect(store.axes(for: store.preset(named: "Web")!).quality == "web")
        var star = store.axes(for: store.preset(named: "AVIF · Smaller")!); star.favorite = false
        store.setAxes(star, for: store.preset(named: "AVIF · Smaller")!)
        #expect(store.axisSet.items.count == 21 && store.axisSet.qualities.contains("smaller"))
        store.delete("Mine 2")
        store.flush()
        let files = Set(PresetFolder.files(in: dir).map(\.lastPathComponent))
        #expect(files.contains("web.json") && !files.contains("mine.json") && !files.contains("mine-2.json") && files.count == 23)
        let reread = PresetStore(directory: dir, order: order)
        #expect(reread.presets.map(\.name) == store.presets.map(\.name))   // order kept
        #expect(reread.axes(for: reread.preset(named: "AVIF · Smaller")!).favorite == false)
        #expect(reread.axes(for: reread.preset(named: "Web")!).quality == "web")
        #expect(store.rename("Web", to: "Plain") == "Web")  // duplicate names refused
        // a hand edit is picked up
        try Data(#"{"name":"Hand","options":{"jobs":1}}"#.utf8).write(to: dir.appendingPathComponent("hand.json"))
        store.reload()
        #expect(store.presets.last?.name == "Hand")
    }
}

struct JobFileTests {
    @Test func writeThenTakeDeletes() throws {
        let dir = try tempDir()
        let job = JobFile(preset: "Lossy", overrides: ["jpeg-quality": .int(70)], paths: ["/x.png", "/y dir"], start: true)
        let id = UUID()
        let url = try job.write(uuid: id, in: dir)
        #expect(url.absoluteString == "imagecompress://job/\(id.uuidString)")
        #expect(url.host == "job" && url.lastPathComponent == id.uuidString)
        let taken = try #require(JobFile.take(uuid: url.lastPathComponent, in: dir))
        #expect(taken == job)
        #expect(JobFile.take(uuid: id.uuidString, in: dir) == nil)   // consumed
    }

    @Test func acceptsLowercaseUUIDFromCLI() throws {
        let dir = try tempDir()
        let id = UUID().uuidString.lowercased()
        try Data(#"{"paths":["/x.png"]}"#.utf8).write(to: dir.appendingPathComponent("\(id).json"))
        #expect(JobFile.take(uuid: id, in: dir)?.paths == ["/x.png"])
    }

    @Test func refusesNonUUID() throws {
        let dir = try tempDir()
        #expect(JobFile.take(uuid: "../etc/passwd", in: dir) == nil)
        #expect(JobFile.take(uuid: "abc", in: dir) == nil)
    }
}

struct EngineIntegrationTests {
    /// Drift check: every key in the Rust schema must have a control in the option catalog.
    @Test func schemaCoveredByCatalog() async throws {
        guard FileManager.default.isExecutableFile(atPath: cliBinary.path) else { Issue.record("build the CLI first (make cli)"); return }
        let r = await CLI.exec(["options", "schema"], executable: cliBinary)
        #expect(r.ok)
        let schema = try #require(OptionsSchema.parse(r.stdout))
        let catalogKeys = Set(OptionCatalog.all.map(\.key))
        let ignored: Set<String> = ["dry-run", "png-zopfli"]   // CLI only
        let missing = Set(schema.properties.keys).subtracting(catalogKeys).subtracting(ignored)
        #expect(missing.isEmpty, "Options without a Settings control: \(missing.sorted())")
        // Controls with enums must have schema enum values.
        for spec in OptionCatalog.all where schema.has(spec.key) {
            if case .choice = spec.control { #expect(schema.properties[spec.key]?.enumValues?.isEmpty == false, "\(spec.key) has no enum values") }
        }
    }

    /// Every preset the engine exports must decode (when the engine has `presets export`).
    @Test func decodesEnginePresetsExport() async throws {
        guard FileManager.default.isExecutableFile(atPath: cliBinary.path) else { return }
        // Isolated from the real App Group Presets folder.
        let empty = try tempDir().path
        let r = await CLI.exec(["presets", "export"], executable: cliBinary, environment: ["IMGCOMPRESS_CONFIG": empty])
        guard r.ok else { return }   // engine without the preset layer yet
        let f = try JSONDecoder().decode(PresetsFile.self, from: r.stdout)
        #expect(!f.presets.isEmpty)
        // The Swift fallback list must mirror the engine's built-ins.
        #expect(f.presets == Preset.builtIn)
        let again = try JSONDecoder().decode(PresetsFile.self, from: JSONEncoder().encode(f))
        #expect(again == f)
    }

    /// Real engine: run one fixture through the preview path and expect a smaller result.
    @Test func previewShrinksFixture() async throws {
        guard FileManager.default.isExecutableFile(atPath: cliBinary.path) else { Issue.record("build the CLI first (make cli)"); return }
        let src = repoRoot.appendingPathComponent("fixtures/png/flat_illustration.png")
        let res = try await Preview.render(file: src, options: Options(["png-quantize": .array([.int(65), .int(85)])]), executable: cliBinary)
        #expect(res.inBytes > 0 && res.outBytes > 0 && res.outBytes < res.inBytes)
        #expect(FileManager.default.fileExists(atPath: res.output.path))
        #expect(FileManager.default.fileExists(atPath: src.path))   // the original is never touched
    }

    /// Real engine: the exact options file the app writes is accepted by `run`.
    @Test func runStreamsEventsAndRewritesInPlace() async throws {
        guard FileManager.default.isExecutableFile(atPath: cliBinary.path) else { Issue.record("build the CLI first (make cli)"); return }
        let dir = try tempDir()
        let file = dir.appendingPathComponent("p.png")
        try FileManager.default.copyItem(at: repoRoot.appendingPathComponent("fixtures/png/palette.png"), to: file)
        let before = Preview.size(file)
        let runner = EngineRunner(executable: cliBinary)
        var finished: UInt64?
        var sawTotals = false
        for try await e in runner.run(paths: [file.path], options: Options(["history": .bool(false)])) {
            if case .finished(_, _, _, let o, _, _, _) = e { finished = o }
            if case .totals = e { sawTotals = true }
        }
        #expect(sawTotals)
        let out = try #require(finished)
        #expect(out <= before && Preview.size(file) == out)
    }


    @MainActor @Test func freshStateSelectsPreserveLossless() throws {
        let root = try tempDir()
        let set = PresetStore(directory: root.appendingPathComponent("Presets"), order: root.appendingPathComponent("o.json")).axisSet
        // nothing stored: the default pair resolves, applies as overrides, and then matches itself
        let c = AxisSet.defaultChoice
        let p = try #require(set.resolve(format: c.format, quality: c.quality, touched: .format))
        var overrides = Options()
        set.apply(p, to: &overrides)
        let eff = AppSettings.effective(base: Options(["destination": .string("in-place")]), preset: nil, overrides: overrides)
        let m = try #require(set.matching(eff, prefer: c))
        #expect(m.format == "keep" && m.quality == "lossless")
        #expect(AxisSet.matches(p.preset, eff))
        for (k, v) in p.preset.options.values { #expect(eff[k] == v) }   // the engine really gets the lossless options
    }
}

@MainActor
struct UndoStoreTests {
    private func entry(_ name: String, age: TimeInterval, backup: String, size: Int) throws -> UndoEntry {
        try Data(count: size).write(to: URL(fileURLWithPath: backup))
        var e = UndoEntry(name: name, inBytes: 10, outBytes: 5,
                          undo: UndoInfo(output: "/x/\(name)", outSize: 5, outHash: "h", restore: [.init(path: "/x/\(name)", backup: backup)]))
        e.date = Date().addingTimeInterval(-age)
        return e
    }

    @Test func pruneDropsOldMissingAndOversizedEntries() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("undo-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let store = UndoStore(root: dir.appendingPathComponent("root"), tempRoot: dir.appendingPathComponent("tmp"))
        let old = try entry("old", age: 25 * 3600, backup: dir.appendingPathComponent("old").path, size: 10)
        let gone = try entry("gone", age: 60, backup: dir.appendingPathComponent("gone").path, size: 10)
        try FileManager.default.removeItem(atPath: dir.appendingPathComponent("gone").path)
        let a = try entry("a", age: 300, backup: dir.appendingPathComponent("a").path, size: 600)
        let b = try entry("b", age: 100, backup: dir.appendingPathComponent("b").path, size: 600)
        for e in [old, gone, a, b] { store.add(e) }
        store.prune(maxBytes: 1000)
        // old expired, gone has no backup, a is the oldest of the two that no longer fit under 1000 bytes
        #expect(store.entries.map(\.name) == ["b"])
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("a").path))
        #expect(!FileManager.default.fileExists(atPath: dir.appendingPathComponent("old").path))
    }

    /// Backups with no entry (the app was killed before saving its history) go once they are a day old.
    @Test func orphanedBackupsAreSweptAfterADay() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("undo-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let store = UndoStore(root: dir.appendingPathComponent("root"), tempRoot: dir.appendingPathComponent("tmp"))
        let fm = FileManager.default
        func slot(_ name: String, old: Bool = false) throws -> URL {
            let s = (old ? dir.appendingPathComponent("root/backups") : store.backupDir).appendingPathComponent(name)
            try fm.createDirectory(at: s, withIntermediateDirectories: true)
            try Data("x".utf8).write(to: s.appendingPathComponent("img.png"))
            return s
        }
        let orphan = try slot("orphan"), fresh = try slot("fresh"), kept = try slot("kept"), legacy = try slot("legacy", old: true)
        store.add(try entry("kept", age: 60, backup: kept.appendingPathComponent("img.png").path, size: 1))
        let later = Date().addingTimeInterval(UndoStore.maxAge + 60)
        try fm.setAttributes([.creationDate: Date().addingTimeInterval(-UndoStore.maxAge - 60)], ofItemAtPath: orphan.path)
        try fm.setAttributes([.creationDate: Date().addingTimeInterval(-UndoStore.maxAge - 60)], ofItemAtPath: legacy.path)
        store.sweepOrphans()
        #expect(!fm.fileExists(atPath: orphan.path) && !fm.fileExists(atPath: legacy.path))
        #expect(!fm.fileExists(atPath: dir.appendingPathComponent("root/backups").path))   // the old folder goes once empty
        #expect(fm.fileExists(atPath: fresh.path) && fm.fileExists(atPath: kept.path))
        store.sweepOrphans(now: later)   // a referenced backup is left to `prune`
        #expect(fm.fileExists(atPath: kept.path) && !fm.fileExists(atPath: fresh.path))
    }

    @Test func entriesSurviveARelaunch() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("undo-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let first = UndoStore(root: dir, tempRoot: dir.appendingPathComponent("tmp"))
        first.add(try { try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true); return try entry("k", age: 5, backup: dir.appendingPathComponent("k").path, size: 4) }())
        let second = UndoStore(root: dir, tempRoot: dir.appendingPathComponent("tmp"))
        #expect(second.entries.map(\.name) == ["k"])
    }

    // MARK: Adding a file again: which undo entry it maps to

    private func info(output: String, restore: [(String, String)]) -> UndoInfo {
        UndoInfo(output: output, outSize: 5, outHash: "h", restore: restore.map { .init(path: $0.0, backup: $0.1) })
    }
    private func keyed(_ es: [UndoEntry]) -> [UndoStore.Keyed] { es.map { .init(entry: $0, output: $0.undo.output, source: $0.sourcePath) } }

    @Test func inPlaceResultMapsToItsEntry() {
        let e = UndoEntry(name: "p.png", inBytes: 10, outBytes: 5, undo: info(output: "/x/p.png", restore: [("/x/p.png", "/b/1/p.png")]), source: "/x/p.png")
        #expect(UndoStore.plan(forKey: "/x/p.png", in: keyed([e]), isLive: { _ in true }) == .fromOriginal(e, source: "/x/p.png"))
        #expect(UndoStore.plan(forKey: "/x/other.png", in: keyed([e]), isLive: { _ in true }) == .asNew)
        // blocked (edited, moved, original gone): treated as a new file
        #expect(UndoStore.plan(forKey: "/x/p.png", in: keyed([e]), isLive: { _ in false }) == .asNew)
    }

    /// PNG → AVIF with originals kept: nothing is restored, yet photo.avif maps back to photo.png, and so
    /// does photo.png itself (its result is put away first, so the new one doesn't clash or pile up).
    @Test func renamedResultMapsBackToTheKeptOriginal() {
        let e = UndoEntry(name: "photo.png", inBytes: 10, outBytes: 5, undo: info(output: "/x/photo.avif", restore: []), source: "/x/photo.png")
        #expect(UndoStore.plan(forKey: "/x/photo.avif", in: keyed([e]), isLive: { _ in true }) == .fromOriginal(e, source: "/x/photo.png"))
        #expect(UndoStore.plan(forKey: "/x/photo.png", in: keyed([e]), isLive: { _ in true }) == .fromOriginal(e, source: "/x/photo.png"))
        // following a chain back only looks at what a job wrote
        #expect(UndoStore.plan(forKey: "/x/photo.png", in: keyed([e]), outputsOnly: true, isLive: { _ in true }) == .asNew)
    }

    /// PNG → AVIF with originals trashed, recorded before entries kept their source: the restore target is the original.
    @Test func olderEntriesFindTheirSourceInTheRestoreList() {
        let trashed = UndoEntry(name: "photo.png", inBytes: 10, outBytes: 5, undo: info(output: "/x/photo.avif", restore: [("/x/photo.png", "/b/1/photo.png")]))
        #expect(trashed.sourcePath == "/x/photo.png")
        #expect(UndoStore.plan(forKey: "/x/photo.avif", in: keyed([trashed]), isLive: { _ in true }) == .fromOriginal(trashed, source: "/x/photo.png"))
        let inPlace = UndoEntry(name: "p.png", inBytes: 10, outBytes: 5, undo: info(output: "/x/p.png", restore: [("/x/p.png", "/b/2/p.png")]))
        #expect(inPlace.sourcePath == "/x/p.png")
        // an index written by an older version (no "source") still loads
        let old = #"{"id":"\#(UUID().uuidString)","date":"2026-01-01T00:00:00Z","name":"p.png","inBytes":10,"outBytes":5,"undo":{"output":"/x/p.png","out_size":5,"out_hash":"h","restore":[]}}"#
        let dec = JSONDecoder(); dec.dateDecodingStrategy = .iso8601
        #expect((try? dec.decode(UndoEntry.self, from: Data(old.utf8)))?.source == nil)
    }

    /// The newest live entry wins: a second run on the same file shadows the first.
    @Test func newestLiveEntryWins() {
        let older = UndoEntry(name: "p.png", inBytes: 10, outBytes: 5, undo: info(output: "/x/p.webp", restore: []), source: "/x/p.png")
        let newer = UndoEntry(name: "p.png", inBytes: 10, outBytes: 4, undo: info(output: "/x/p.avif", restore: []), source: "/x/p.png")
        #expect(UndoStore.plan(forKey: "/x/p.png", in: keyed([newer, older]), isLive: { _ in true }) == .fromOriginal(newer, source: "/x/p.png"))
        #expect(UndoStore.plan(forKey: "/x/p.png", in: keyed([newer, older]), isLive: { $0.id == older.id }) == .fromOriginal(older, source: "/x/p.png"))
    }

    /// Real engine: compress in place with an undo copy, add the result again with other settings: the
    /// original is put back first, the row is queued for the original, and the new run starts from its bytes.
    @Test func addingAResultAgainStartsFromTheOriginal() async throws {
        guard FileManager.default.isExecutableFile(atPath: cliBinary.path) else { Issue.record("build the CLI first (make cli)"); return }
        let dir = try tempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let undo = UndoStore(root: dir.appendingPathComponent("root"), tempRoot: dir.appendingPathComponent("tmp"))
        let presets = PresetStore(directory: dir.appendingPathComponent("Presets"), order: dir.appendingPathComponent("o.json"))
        let store = JobStore(presets: presets, defaults: UserDefaults(suiteName: "ic-test-\(UUID().uuidString)")!, undoStore: undo)
        let file = dir.appendingPathComponent("p.png")
        try FileManager.default.copyItem(at: repoRoot.appendingPathComponent("fixtures/png/flat_illustration.png"), to: file)
        let original = try Data(contentsOf: file)

        // first run: lossy, in place, with an undo copy; the row and entry are what `apply(.finished)` records
        // (apply itself also drives the Dock tile, which needs a running app)
        let lossy = Options(["history": .bool(false), "png-quantize": .array([.int(40), .int(60)])])
        var row = JobItem(path: file.path, isDirectory: false)
        for try await e in EngineRunner(executable: cliBinary).run(paths: [file.path], options: lossy, backupDir: undo.backupDir) {
            guard case .finished(let p, let out, let i, let o, _, _, let info?) = e else { continue }
            let entry = UndoEntry(name: row.name, inBytes: i, outBytes: o, undo: info, source: p)
            undo.add(entry)
            row.status = .done; row.inBytes = i; row.outBytes = o; row.output = out; row.undoEntry = entry
        }
        store.items = [row]
        #expect(row.status == .done && row.undoEntry?.sourcePath == file.path)
        #expect(try Data(contentsOf: file) != original)
        #expect(store.canCompressAgain([row.id]))

        // adding the result again puts the original back and hands it on to be queued
        var queued: [URL] = []
        await withCheckedContinuation { (done: CheckedContinuation<Void, Never>) in
            store.prepareAgain([file], executable: cliBinary) { urls in queued = urls; store.add(urls); done.resume() }
        }
        #expect(queued.map(\.path) == [file.path])
        #expect(try Data(contentsOf: file) == original)
        #expect(undo.entries.isEmpty)
        #expect(store.items.count == 1 && store.items[0].status == .queued && store.items[0].undoEntry == nil)

        // the next run measures against the original
        var inBytes: UInt64?
        for try await e in EngineRunner(executable: cliBinary).run(paths: [file.path], options: Options(["history": .bool(false)]), backupDir: undo.backupDir) {
            if case .finished(_, _, let i, _, _, _, _) = e { inBytes = i }
        }
        #expect(inBytes == UInt64(original.count))
    }

    /// When the engine refuses to put the original back, nothing is queued and the file is left alone.
    @Test func refusedRestoreQueuesNothing() async throws {
        guard FileManager.default.isExecutableFile(atPath: cliBinary.path) else { Issue.record("build the CLI first (make cli)"); return }
        let dir = try tempDir()
        defer { try? FileManager.default.removeItem(at: dir) }
        let undo = UndoStore(root: dir.appendingPathComponent("root"), tempRoot: dir.appendingPathComponent("tmp"))
        let presets = PresetStore(directory: dir.appendingPathComponent("Presets"), order: dir.appendingPathComponent("o.json"))
        let store = JobStore(presets: presets, defaults: UserDefaults(suiteName: "ic-test-\(UUID().uuidString)")!, undoStore: undo)
        let file = dir.appendingPathComponent("p.png"), backup = dir.appendingPathComponent("tmp/1/p.png")
        try FileManager.default.createDirectory(at: backup.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("original".utf8).write(to: backup)
        try Data("12345".utf8).write(to: file)   // same size as recorded, different bytes: only the engine's hash check catches it
        let e = UndoEntry(name: "p.png", inBytes: 8, outBytes: 5, undo: info(output: file.path, restore: [(file.path, backup.path)]), source: file.path)
        undo.add(e)
        let urls = await store.restoreOriginals([(file, .fromOriginal(e, source: file.path))], executable: cliBinary)
        #expect(urls.isEmpty)
        // a notice, carried into the next run (an error would be cleared when the other files start)
        #expect(store.notice?.contains("p.png") == true)
        #expect(try Data(contentsOf: file) == Data("12345".utf8))
        #expect(FileManager.default.fileExists(atPath: backup.path))
    }

    @Test func networkUndoCopiesLiveInAPrivateFolderClearedAtStartup() throws {
        let dir = UndoStore.bootTemporaryRoot()
        #expect(dir.path == "/private/tmp/ImageCompressUndo-\(getuid())")
        let attrs = try FileManager.default.attributesOfItem(atPath: dir.path)
        #expect((attrs[.posixPermissions] as? Int) == 0o700)
        #expect((attrs[.ownerAccountID] as? Int).map(UInt32.init) == getuid())
    }
}
