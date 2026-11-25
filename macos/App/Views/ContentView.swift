import SwiftUI
import QuickLookThumbnailing

struct ContentView: View {
    @Bindable var store: JobStore
    @State private var presets = PresetStore.shared
    @State private var schemaStore = SchemaStore.shared
    @State private var selection = Set<UUID>()
    @AppStorage("showInspector") private var showInspector = false
    @AppStorage("showAdvancedOptions") private var showAdvanced = false
    @State private var dropTargeted = false
    @State private var comparing: JobItem?
    @State private var savingPreset = false
    @State private var newPresetName = ""
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        VStack(spacing: 0) {
            if store.items.isEmpty { DropZone(targeted: dropTargeted) } else { jobList }
            Divider()
            footer
        }
        .frame(minWidth: 800, minHeight: 380)
        .overlay { FileDropTarget(isTargeted: $dropTargeted) { urls in withAnimation(.snappy) { store.ingest(urls) } } }
        .animation(.snappy, value: dropTargeted)
        .toolbar(id: "main") { toolbarItems }
        .inspector(isPresented: $showInspector) { inspector.inspectorColumnWidth(min: 300, ideal: 380, max: 520) }
        .sheet(isPresented: $store.showUndoHistory) { UndoHistoryView(undo: store.undoStore) }
        .sheet(item: $comparing) { item in CompareView(item: item, options: store.effectiveOptions) }
        .alert("Save as Preset", isPresented: $savingPreset) {
            TextField("Name", text: $newPresetName)
            Button("Save") {
                let p = presets.add(name: newPresetName.trimmingCharacters(in: .whitespaces), options: store.overrides.merged(over: store.preset?.options ?? Options()))
                store.presetName = p.name; store.overrides = Options()
            }
            .disabled(newPresetName.trimmingCharacters(in: .whitespaces).isEmpty)
            Button("Cancel", role: .cancel) {}
        } message: { Text("Saves the current preset together with your changes.") }
        .task { await schemaStore.load() }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in presets.reload() }
    }

    /// Default layout: Format + Quality at the left, actions in the middle, Options at the right. Items are
    /// id-based so View > Customize Toolbar works and the layout persists. The ids must never change.
    @ToolbarContentBuilder private var toolbarItems: some CustomizableToolbarContent {
        ToolbarItem(id: "format", placement: .navigation) { FormatMenu(goal: goal).disabled(store.isRunning).accessibilityLabel("Output format").accessibilityHint("Preserve each format or convert to another") }
        ToolbarItem(id: "quality", placement: .navigation) { QualityPicker(goal: goal).frame(minWidth: 300).disabled(store.isRunning) }
        ToolbarItem(id: "presets", placement: .principal) {
            Menu("Presets", systemImage: "bookmark") {
                if !presets.plainPresets.isEmpty {
                    Toggle("No Preset", isOn: Binding(get: { store.isStock }, set: { _ in store.chooseNoPreset() }))
                        .help("Just the Format and Quality controls")
                }
                ForEach(presets.plainPresets) { p in
                    Toggle(p.name, isOn: Binding(get: { store.isActive(p) }, set: { _ in store.choosePreset(p.name) }))
                }
                if !presets.plainPresets.isEmpty { Divider() }
                Button("Save Current Settings as Preset…") { newPresetName = ""; savingPreset = true }
                Button("Manage Presets…") { SettingsRouter.shared.pane = .presets; openSettings() }
            }
            .help("Saved presets").disabled(store.isRunning)
        }
        ToolbarItem(id: "add", placement: .principal) { Button("Add Files…", systemImage: "plus") { store.addViaPanel() }.help("Add images or folders (⌘O)") }
        ToolbarItem(id: "clear", placement: .principal) { Button("Clear List", systemImage: "eraser.line.dashed") { store.clear() }.help("Remove everything from the list (files are not touched)").disabled(store.isRunning || store.items.isEmpty) }
        // macOS supplies Flexible Space itself; this fixed one is how a plain Space reaches View > Customize Toolbar.
        if #available(macOS 26.0, *) { ToolbarSpacer(.fixed) }
        ToolbarItem(id: "options", placement: .primaryAction) { Toggle("Options", systemImage: "slider.horizontal.3", isOn: $showInspector).help("Show or hide options (⌥⌘I)").accessibilityLabel("Show options").accessibilityAddTraits(.isToggle) }
    }

    private var lower: Options { (store.preset?.options ?? Options()).merged(over: AppSettings.shared.base.merged(over: schemaStore.schema.defaults)) }
    private var goal: GoalModel { GoalModel(store: store, inherited: lower) }

    private var inspector: some View {
        let lower = (store.preset?.options ?? Options()).merged(over: AppSettings.shared.base.merged(over: schemaStore.schema.defaults))
        return Form {
            Section {
                Text("Changes apply to images you add from now on, until you choose Reset or another preset. Save as Preset keeps them for later.").font(.caption).foregroundStyle(.secondary)
                HStack {
                    Button("Reset") { store.resetOverrides() }.disabled(!store.hasOneRunChanges)
                    Button("Save as Preset…") { newPresetName = ""; savingPreset = true }.disabled(!store.hasOneRunChanges)
                }
            }
            Section("Common") {
                OptionsForm(schema: schemaStore.schema, specs: OptionCatalog.essentials, inherited: lower, edited: $store.overrides, baseline: store.restingOverrides)
            }
            Section {
                DisclosureGroup("Advanced", isExpanded: $showAdvanced) {
                    ForEach(OptionCatalog.advancedPanels) { panel in
                        if panel.specs.contains(where: { schemaStore.schema.has($0.key) }) {
                            Text(panel.title).font(.subheadline.weight(.semibold)).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading).padding(.top, 10)
                            OptionsForm(schema: schemaStore.schema, specs: panel.specs, inherited: lower, edited: $store.overrides, baseline: store.restingOverrides)
                        }
                    }
                }
            } footer: {
                Text("Defaults are tuned for most images. You rarely need to open this.")
            }
        }
        .formStyle(.grouped)
    }

    private var jobList: some View {
        let dupes = Dictionary(grouping: store.items, by: \.name).filter { $1.count > 1 }.keys
        return List(store.items, selection: $selection) { item in
            let children = item.isDirectory ? store.items.filter { !$0.isDirectory && $0.path.hasPrefix(item.path + "/") } : []
            JobRow(item: item, showFolder: dupes.contains(item.name),
                   childCount: children.count, undoneChildren: children.count { $0.restored })
                .tag(item.id)
                .draggable(item.revealURL)
        }
        // Right-click acts on the selection (or the clicked row); double-click / Return opens the results.
        .contextMenu(forSelectionType: UUID.self) { ids in
            let picked = store.items.filter { ids.contains($0.id) }
            if !picked.isEmpty {
                Button("Open") { open(picked) }
                Button("Reveal in Finder") { NSWorkspace.shared.activateFileViewerSelecting(picked.map(\.revealURL)) }
                Button("Before / After…") { comparing = picked[0] }.disabled(picked.count != 1 || picked[0].isDirectory)
                Button("Copy") { copy(picked) }
                Divider()
                Button(picked.count == 1 ? "Undo Compression" : "Undo Compression of \(picked.count) Images") { store.undo(picked.map(\.id)) }
                    .disabled(!store.canUndo(picked.map(\.id)))
                Button("Remove from List") { store.remove(ids: ids) }
            }
        } primaryAction: { ids in
            open(store.items.filter { ids.contains($0.id) })
        }
        .animation(store.isRunning ? nil : .snappy, value: store.items.count)   // no per-row animation in big runs
        .onDeleteCommand { store.remove(ids: selection); selection = [] }
        .onCopyCommand { selectedItems.map { NSItemProvider(object: $0.revealURL as NSURL) } }
        .background {
            // The list swallows Space before onKeyPress sees it, so use shortcuts: Space and ⌘Y (Quick Look's key).
            Group {
                Button("Before / After") { compareSelection() }.keyboardShortcut(.space, modifiers: [])
                Button("Before / After ") { compareSelection() }.keyboardShortcut("y", modifiers: .command)
            }
            .opacity(0).allowsHitTesting(false).accessibilityHidden(true)
        }
        .accessibilityLabel("Images")
    }

    private func compareSelection() {
        guard selection.count == 1, let item = selectedItems.first, !item.isDirectory else { return }
        comparing = item
    }
    private var selectedItems: [JobItem] { store.items.filter { selection.contains($0.id) } }
    /// Opens results in their default app (folders in Finder), like double-clicking in Finder.
    private func open(_ items: [JobItem]) {
        for i in items.prefix(20) { NSWorkspace.shared.open(i.revealURL) }
    }
    private func copy(_ items: [JobItem]) {
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.writeObjects(items.map { $0.revealURL as NSURL })
    }

    /// "3 processed, 1 skipped", leaving out zero counts; "2 undone" once files were put back.
    private func summary(_ t: Totals) -> String {
        var parts = t.processed > 0 || store.undoneInRun == 0 ? ["\(t.processed) processed"] : []
        if t.skipped > 0 { parts.append("\(t.skipped) skipped") }
        if t.failed > 0 { parts.append("\(t.failed) failed") }
        if store.undoneInRun > 0 { parts.append("\(store.undoneInRun) undone") }
        return parts.joined(separator: ", ")
    }

    private var footer: some View {
        HStack {
            HStack { status }.frame(maxWidth: .infinity, alignment: .leading)
            // Inspector changes now carry over to every drop: keep them visible even with the inspector closed.
            if store.hasOneRunChanges && !store.isRunning {
                Button { showInspector = true } label: { Label("Custom options", systemImage: "slider.horizontal.3") }
                    .buttonStyle(.borderless).foregroundStyle(.orange).help("Options differ from the preset; show them")
                Button("Reset") { store.resetOverrides() }.buttonStyle(.borderless).help("Go back to the preset's options")
                if store.queuedCount > 0 { Divider().frame(height: 16) }
            }
            // Drops start right away, so the action only shows up when there is something to act on: Stop during
            // a run, or Compress when files are waiting (auto-process off, a cancelled Trash confirmation, a Stop).
            // It lives here rather than in the toolbar so it never ends up in the » overflow.
            if store.isRunning {
                Button("Stop") { store.stop() }.help("Stop after the current images (⌘.)")
            } else if store.queuedCount > 0 {
                Button("Compress \(store.queuedCount == 1 ? "1 Image" : "\(store.queuedCount) Images")") { store.process() }
                    .buttonStyle(.borderedProminent).help("Compress the waiting images (⌘↩)")
            }
        }
        .frame(minHeight: 24)
        .padding(.horizontal, 10).padding(.vertical, 6)
        .animation(.snappy, value: store.isRunning)
    }

    @ViewBuilder private var status: some View {
        if let err = store.lastError {
            Text(err).foregroundStyle(.red).lineLimit(2).textSelection(.enabled)
        } else if store.isRunning {
            ProgressView(value: Double(store.progressDone), total: Double(max(store.progressTotal, 1))).frame(width: 140)
            Text("\(store.progressDone) of \(store.progressTotal)").monospacedDigit().contentTransition(.numericText())
        } else if store.queuedCount > 0 {
            Text("^[\(store.queuedCount) image](inflect: true) waiting").foregroundStyle(.secondary)
        } else if let n = store.notice {
            Label(n, systemImage: "info.circle").foregroundStyle(.secondary).lineLimit(2).font(.caption)
            Spacer()
            if let t = store.totals { Text(summary(t)).foregroundStyle(.secondary) }
        } else if let t = store.totals {
            let saved = t.inBytes > t.outBytes ? t.inBytes - t.outBytes : 0
            Text(summary(t))
            Spacer()
            if t.processed > 0 {
                Label("Saved \(fmt(saved))", systemImage: "arrow.down.circle.fill").foregroundStyle(.green).monospacedDigit().contentTransition(.numericText())
                Text("\(fmt(t.inBytes)) → \(fmt(t.outBytes))").monospacedDigit().foregroundStyle(.secondary)
            }
        } else if store.items.isEmpty {
            let s = AppSettings.shared
            Text((s.autoProcess ? "Images are compressed as soon as you add them." : "Add images, then choose Compress (⌘↩).")
                 + (s.keepUndo ? " Each one can be undone for 24 hours." : ""))
                .font(.caption).foregroundStyle(.secondary)
        } else {
            Text("^[\(store.items.count) item](inflect: true)").foregroundStyle(.secondary)
        }
    }
}

/// A folder for display: home as ~, and macOS's /private/{tmp,var,etc} firmlinks shown as the short form.
func displayFolder(of path: String) -> String {
    var dir = (path as NSString).deletingLastPathComponent
    for p in ["/private/tmp", "/private/var", "/private/etc"] where dir == p || dir.hasPrefix(p + "/") { dir.removeFirst("/private".count) }
    return (dir as NSString).abbreviatingWithTildeInPath
}

func fmt(_ b: UInt64) -> String { ByteCountFormatter.string(fromByteCount: Int64(b), countStyle: .file) }

struct JobRow: View {
    let item: JobItem
    var showFolder = false
    /// Files of this run found inside a dropped folder.
    var childCount = 0
    /// Of those, how many were put back with Undo.
    var undoneChildren = 0
    @State private var thumb: NSImage?

    var body: some View {
        HStack {
            thumbnail
            labels
            Spacer()
            sizes
            trailingButtons
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(accessibilityTitle)
        .accessibilityValue(accessibilityValueText)
        // The combined row hides its buttons from VoiceOver: offer them as actions (VO-Command-Space).
        .accessibilityActions {
            if item.status == .done, !item.restored, let entry = item.undoEntry, JobStore.shared.undoStore.blocker(entry) == nil {
                Button("Undo Compression") { JobStore.shared.undo(item.id) }
            }
            if item.status == .done {
                Button("Reveal in Finder") { NSWorkspace.shared.activateFileViewerSelecting([item.revealURL]) }
            }
        }
        .task(id: item.path) {
            guard !item.isDirectory else { return }
            let req = QLThumbnailGenerator.Request(fileAt: item.url, size: CGSize(width: 36, height: 36), scale: 2, representationTypes: .thumbnail)
            thumb = try? await QLThumbnailGenerator.shared.generateBestRepresentation(for: req).nsImage
        }
    }

    private var thumbnail: some View {
        Group {
            if let thumb { Image(nsImage: thumb).resizable().scaledToFit() }
            else { Image(systemName: item.isDirectory ? "folder" : "photo").foregroundStyle(.secondary) }
        }.frame(width: 36, height: 36)
    }

    private var labels: some View {
        VStack(alignment: .leading) {
            Text(item.name).lineLimit(1)
            if showFolder { Text(parentFolder).font(.caption).foregroundStyle(.tertiary).lineLimit(1).truncationMode(.head) }
            Text(statusText).font(.caption).foregroundStyle(statusColor).lineLimit(1)
        }
    }

    @ViewBuilder private var sizes: some View {
        if let i = item.inBytes, let o = item.outBytes, !item.restored {
            let saved: Double = item.savings ?? 0
            let savedText: String = item.savings.map { String(format: "%.1f%%", $0 * 100) } ?? ""
            Text("\(fmt(i)) → \(fmt(o))").monospacedDigit().foregroundStyle(.secondary)
            Text(savedText).monospacedDigit().foregroundStyle(saved > 0 ? Color.green : Color.secondary).frame(width: 56, alignment: .trailing)
        }
    }

    @ViewBuilder private var trailingButtons: some View {
        if case .running = item.status { ProgressView().controlSize(.small) }
        if item.status == .done, !item.restored, let entry = item.undoEntry {
            let blocked = JobStore.shared.undoStore.blocker(entry)
            let tip: String = blocked.map { "Can't undo: \($0.lowercased())" } ?? "Undo: put the original back"
            Button { JobStore.shared.undo(item.id) } label: { Image(systemName: "arrow.uturn.backward.circle") }
                .buttonStyle(.borderless).disabled(blocked != nil).help(tip)
                .focusable(false)   // never the target of Space or Return: undo only on a deliberate click
        }
        if item.status == .done {
            Button { NSWorkspace.shared.activateFileViewerSelecting([item.revealURL]) } label: { Image(systemName: "magnifyingglass.circle") }
                .buttonStyle(.borderless).help("Reveal in Finder").focusable(false)
        }
    }

    private var accessibilityTitle: String {
        let where_ = showFolder ? ", in \(parentFolder)" : ""
        return "\(item.name)\(where_), \(statusText)"
    }

    private var accessibilityValueText: String {
        guard !item.restored, let i = item.inBytes, let o = item.outBytes else { return "" }
        var text = "\(fmt(i)) to \(fmt(o))"
        if let saved = item.savings, saved > 0 { text += ", \(Int((saved * 100).rounded())) percent smaller" }
        return text
    }

    /// Parent folder with the home directory abbreviated, to tell same-named files apart.
    private var parentFolder: String { displayFolder(of: item.path) }

    private var statusText: String {
        switch item.status {
        case .queued: "Queued"
        case .running(let st): Self.stageTitle(st)
        case .done:
            if item.restored { "Restored the original" }
            else if item.isDirectory {
                if childCount > 0, undoneChildren == childCount { "Restored the originals" }
                else if childCount > 0 { "Done: \(childCount) \(childCount == 1 ? "image" : "images")" + (undoneChildren > 0 ? ", \(undoneChildren) undone" : "") }
                else { "Done" }
            }
            else if let out = item.output, (out as NSString).lastPathComponent != item.name { "Saved as \((out as NSString).lastPathComponent)" }
            else { "Done" }
        case .skipped(let r): Self.skipTitle(r)
        case .failed(let e): "Failed: \(e)"
        }
    }
    /// The engine's skip reasons in plain words; anything unknown is shown as "Skipped: <reason>".
    static func skipTitle(_ r: String) -> String {
        switch r {
        case "not smaller": "Kept the original: the result was not smaller"
        case "unchanged since last run (history)": "Already compressed with these settings"
        case "empty file": "Skipped: the file is empty"
        case "below min size": "Skipped: smaller than the minimum size"
        default: "Skipped: \(r)"
        }
    }

    /// The engine's stage ids ("decode", "quality-search q=80 …") as words.
    static func stageTitle(_ s: String) -> String {
        let id = s.split(separator: " ").first.map(String.init) ?? s
        switch id {
        case "Starting": return "Starting…"
        case "decode": return "Reading…"
        case "resize": return "Resizing…"
        case "encode": return "Encoding…"
        case "jxl-transcode": return "Converting to JPEG XL losslessly…"
        case "jpegtran": return "Repacking the JPEG…"
        case "quantize": return "Reducing the palette…"
        case "optimize": return "Optimizing…"
        case "gifsicle": return "Optimizing the animation…"
        case "svg": return "Optimizing the SVG…"
        case "quality-search": return "Finding the best quality…"
        default: return s
        }
    }

    private var statusColor: Color {
        switch item.status { case .failed: .red; case .done: item.restored || (childCount > 0 && undoneChildren == childCount) ? .secondary : .green; default: .secondary }
    }
}

struct DropZone: View {
    var targeted = false
    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "photo.on.rectangle.angled").font(.system(size: 48)).foregroundStyle(targeted ? Color.accentColor : .secondary).symbolEffect(.bounce, value: targeted).scaleEffect(targeted ? 1.1 : 1)
            Text("Drop images or folders here").font(.title2)
            Text("or press ⌘O to add files").foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Drop images or folders here, or press Command O to add files")
        .background(RoundedRectangle(cornerRadius: 12).fill(Color.accentColor.opacity(targeted ? 0.08 : 0)).padding(20))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(style: StrokeStyle(lineWidth: 2, dash: [8])).foregroundStyle(targeted ? AnyShapeStyle(Color.accentColor) : AnyShapeStyle(.quaternary)).padding(20))
    }
}

/// Everything from the last day that can still be put back, including files from earlier launches.
struct UndoHistoryView: View {
    let undo: UndoStore
    @Environment(\.dismiss) private var dismiss
    @State private var message: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Recent Compressions").font(.title3.bold())
            Text("Originals are kept for 24 hours (up to 1 GB) in a temporary folder, so restarting your Mac clears them. Undo puts the original back and refuses if the file was changed since.")
                .font(.caption).foregroundStyle(.secondary)
            if undo.entries.isEmpty {
                Text("Nothing to undo.").foregroundStyle(.secondary).frame(maxWidth: .infinity, minHeight: 120)
            } else {
                List(undo.entries) { e in
                    HStack {
                        let blocked = undo.blocker(e)
                        VStack(alignment: .leading) {
                            Text(e.name).lineLimit(1)
                            Text(displayFolder(of: e.undo.output))
                                .font(.caption).foregroundStyle(.tertiary).lineLimit(1).truncationMode(.head)
                            Text(blocked ?? "\(fmt(e.inBytes)) → \(fmt(e.outBytes)) · \(e.date.formatted(.relative(presentation: .named)))")
                                .font(.caption).foregroundStyle(.secondary)
                        }
                        Spacer()
                        Button { NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: e.undo.output)]) } label: { Image(systemName: "magnifyingglass.circle") }
                            .buttonStyle(.borderless).help("Reveal in Finder").disabled(!FileManager.default.fileExists(atPath: e.undo.output))
                        Button("Undo") {
                            Task {
                                do { try await undo.undo(e.id); message = nil; JobStore.shared.markRestored(undoID: e.id) }
                                catch { message = "\(e.name): \(error.localizedDescription)" }
                            }
                        }
                        .disabled(blocked != nil).help(blocked ?? "Put the original back")
                    }
                }
                .frame(minHeight: 200)
            }
            if let message { Text(message).foregroundStyle(.red).font(.caption) }
            HStack { Spacer(); Button("Done") { dismiss() }.keyboardShortcut(.cancelAction) }
        }
        .padding()
        .frame(minWidth: 560, idealWidth: 620, minHeight: 460, idealHeight: 560)
    }
}
