import SwiftUI
import FinderSync

enum SettingsPane: Hashable {
    case general, presets, format(String), tools, finder
}

/// Lets other windows (the toolbar's "Manage Presets…") choose the Settings pane before opening it.
@MainActor @Observable final class SettingsRouter {
    static let shared = SettingsRouter()
    var pane: SettingsPane = .general
}

struct SettingsView: View {
    @State private var router = SettingsRouter.shared
    private var selection: Binding<SettingsPane> { Binding(get: { router.pane }, set: { router.pane = $0 }) }
    @State private var schemaStore = SchemaStore.shared
    @AppStorage("settingsShowAdvancedDefaults") private var showAdvancedDefaults = false

    private var panels: [PanelSpec] {
        OptionCatalog.panels.filter { p in p.specs.contains { schemaStore.schema.has($0.key) } }
    }

    var body: some View {
        NavigationSplitView {
            List(selection: selection) {
                // Ordered by flow: defaults first, then presets built on them, then where they surface, then engine.
                Section("Everyday") {
                    Label("General", systemImage: "gearshape").tag(SettingsPane.general)
                    Label("Presets", systemImage: "slider.horizontal.3").tag(SettingsPane.presets)
                    Label("Finder", systemImage: "folder").tag(SettingsPane.finder)
                }
                Section("Advanced") {
                    Label("Tools", systemImage: "wrench.and.screwdriver").tag(SettingsPane.tools)
                    // Defaults are tuned already; the per-format panes stay out of the way until asked for.
                    DisclosureGroup(isExpanded: $showAdvancedDefaults) {
                        ForEach(panels) { p in Label(p.title, systemImage: p.symbol).tag(SettingsPane.format(p.id)) }
                    } label: {
                        // the whole label toggles it (a sidebar disclosure only reacted to its chevron)
                        Text("Default Options").frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
                            .onTapGesture { withAnimation { showAdvancedDefaults.toggle() } }
                    }
                }
            }
            // A fixed-width sidebar with no toggle: Settings panes are always one click away.
            .navigationSplitViewColumnWidth(190)
            .toolbar(removing: .sidebarToggle)
        } detail: {
            detail.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .frame(width: 820, height: 560)
        .task { await schemaStore.load() }
    }

    @ViewBuilder private var detail: some View {
        switch router.pane {
        case .general: GeneralSettings()
        case .presets: PresetsSettings()
        case .format(let id):
            if let p = OptionCatalog.panels.first(where: { $0.id == id }) { BasePanel(panel: p) }
        case .tools: ToolsSettings()
        case .finder: FinderSettingsView()
        }
    }
}

/// A defaults panel: edits the base options that sit under every preset.
struct BasePanel: View {
    let panel: PanelSpec
    @State private var settings = AppSettings.shared
    @State private var schemaStore = SchemaStore.shared

    var body: some View {
        Form {
            Section {
                OptionsForm(schema: schemaStore.schema, specs: panel.specs, inherited: schemaStore.schema.defaults, edited: $settings.base)
            } header: {
                Text("\(panel.title) defaults")
            } footer: {
                Text("Applied to every preset that does not set the option itself. Presets can override these in the Presets pane.")
            }
        }
        .formStyle(.grouped)
        .navigationTitle(panel.title)
    }
}

struct GeneralSettings: View {
    @State private var settings = AppSettings.shared
    @State private var store = PresetStore.shared
    @State private var schemaStore = SchemaStore.shared
    @State private var confirmForget = false

    /// Output-related base options (originals, destination, metadata, size cap) that exist in this engine build.
    private var outputSpecs: [OptionSpec] {
        let byKey = Dictionary(uniqueKeysWithValues: OptionCatalog.panels.flatMap(\.specs).map { ($0.key, $0) })
        return ["originals", "destination", "max-dimension", "metadata"].compactMap { byKey[$0] }.filter { schemaStore.schema.has($0.key) }
    }

    var body: some View {
        Form {
            Section {
                Picker("Default preset", selection: $settings.defaultPreset) {
                    Text("None (Format and Quality)").tag("")
                    ForEach(store.plainPresets) { Text($0.name).tag($0.name) }
                    if !settings.defaultPreset.isEmpty, store.preset(named: settings.defaultPreset) == nil {
                        Text("\(settings.defaultPreset) (deleted)").tag(settings.defaultPreset)
                    }
                }
            } header: { Text("Starting point") } footer: {
                Text("What the window starts with. None means just the toolbar's Format and Quality. Your own presets (without format and quality tags) can be chosen here.")
            }
            Section {
                Toggle("Start compressing as soon as files are added", isOn: $settings.autoProcess)
                Toggle("Keep undo copies of replaced originals", isOn: $settings.keepUndo)
            } header: { Text("Behavior") } footer: {
                Text("Undo copies are kept for 24 hours, up to 1 GB, in a temporary folder. Restarting your Mac clears them, and macOS deletes any left after about three days, even if ImageCompress isn't opened.")
            }
            Section {
                OptionsForm(schema: schemaStore.schema, specs: outputSpecs,
                            inherited: schemaStore.schema.defaults, edited: $settings.base)
            } header: { Text("Output and originals") } footer: {
                Text("Defaults for every preset. A preset can override each of these in Presets.")
            }
            Section {
                OptionsForm(schema: schemaStore.schema, specs: OptionCatalog.general.filter { OptionCatalog.generalCommonKeys.contains($0.key) },
                            inherited: schemaStore.schema.defaults, edited: $settings.base)
                DisclosureGroup("More Processing Options") {
                    OptionsForm(schema: schemaStore.schema, specs: OptionCatalog.general.filter { !OptionCatalog.generalCommonKeys.contains($0.key) },
                                inherited: schemaStore.schema.defaults, edited: $settings.base)
                }
            } header: { Text("Which files are processed") } footer: {
                Text("Applies to the app, the Finder menu and the command line tool.")
            }
            Section {
                Toggle("Ask before moving originals to the Trash", isOn: $settings.confirmTrash)
                Text("Permanently deleting originals always asks.").font(.caption).foregroundStyle(.secondary)
            } header: { Text("Confirmations") } footer: {
                Text("Only matters for presets whose originals handling is Trash or Replace.")
            }
            Section {
                Toggle("Notify when a run finishes in the background", isOn: $settings.notifyOnCompletion)
                Toggle("Play a sound", isOn: $settings.playSound)
            } header: { Text("Notifications") } footer: {
                Text("Shown when ImageCompress is in the background, such as after a Finder menu run.")
            }
            if schemaStore.schema.has("history") {
                Section {
                    Button("Forget history…") { confirmForget = true }
                } header: { Text("History") } footer: {
                    Text("Clears the list behind “Remember processed files”.")
                }
            }
        }
        .formStyle(.grouped)
        .alert("Forget processed-file history?", isPresented: $confirmForget) {
            Button("Forget", role: .destructive) { Task { _ = await CLI.exec(["history", "clear"]) } }
            Button("Cancel", role: .cancel) {}
        } message: { Text("Files that were already optimized will be processed again.") }
    }
}

struct PresetsSettings: View {
    @State private var store = PresetStore.shared
    @State private var selection: String?
    @State private var confirmRestore = false

    var body: some View {
        VStack(spacing: 0) {
            Label("Presets build on the defaults in General. Favorites (the star) with a format and quality fill the toolbar's Format and Quality controls; “Show in Finder menu” adds a preset to the Finder menu.", systemImage: "info.circle")
                .font(.caption).foregroundStyle(.secondary).frame(maxWidth: .infinity, alignment: .leading).padding(10)
            Divider()
        HSplitView {
            VStack(spacing: 0) {
                List(selection: $selection) {
                    ForEach(store.presets) { p in
                        let a = store.axes(for: p)
                        HStack {
                            Button { var n = a; n.favorite.toggle(); store.setAxes(n, for: p) } label: {
                                Image(systemName: a.favorite ? "star.fill" : "star").foregroundStyle(a.favorite ? Color.yellow : .secondary)
                            }
                            .buttonStyle(.borderless)
                            .help(a.favorite ? "Favorite: shown in the toolbar's Format and Quality controls" : "Add to the toolbar's Format and Quality controls")
                            .accessibilityLabel(a.favorite ? "Unfavorite \(p.name)" : "Favorite \(p.name)")
                            VStack(alignment: .leading, spacing: 0) {
                                Text(p.name)
                                if let f = a.format, let q = a.quality { Text("\(AxisTags.formatShort(f)), \(AxisTags.qualityTitle(q))").font(.caption2).foregroundStyle(.secondary) }
                            }
                            Spacer()
                            if p.isDestructive { Image(systemName: "trash").foregroundStyle(.secondary).help("Replaces originals") }
                        }.tag(p.name)
                    }
                    .onMove { store.move(from: $0, to: $1) }
                }
                Divider()
                HStack(spacing: 6) {
                    Button { selection = store.add().name } label: { Image(systemName: "plus") }.help("Add preset")
                    Button { if let s = selection { store.delete(s); selection = store.presets.first?.name } } label: { Image(systemName: "minus") }
                        .disabled(selection == nil).help("Delete preset")
                    Button { if let s = selection { selection = store.duplicate(s)?.name } } label: { Image(systemName: "plus.square.on.square") }
                        .disabled(selection == nil).help("Duplicate preset")
                    Spacer()
                    Menu {
                        Button("Show Preset File in Finder") { revealSelected() }
                            .disabled(selection.flatMap { store.preset(named: $0) }.flatMap(store.fileURL(for:)) == nil)
                        Button("Open Presets Folder") { openFolder() }
                        Divider()
                        Button("Restore Default Presets…") { confirmRestore = true }
                    } label: { Image(systemName: "ellipsis.circle") }
                    .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                    .help("Preset files, and restoring the defaults")
                }
                .buttonStyle(.borderless).padding(8)
            }
            .frame(minWidth: 180, idealWidth: 200, maxWidth: 260)

            Group {
                if let name = selection, let i = store.index(of: name) {
                    PresetDetail(preset: $store.presets[i],
                                 axes: Binding(get: { store.preset(named: name).map { store.axes(for: $0) } ?? PresetAxes() }, set: { a in if let p = store.preset(named: name) { store.setAxes(a, for: p) } }),
                                 customFormats: store.customFormatTags, customQualities: store.customQualityTags, onChange: { store.commit() }, onRename: { selection = store.rename(name, to: $0) })
                        .id(name)
                } else {
                    Text("Select a preset").foregroundStyle(.secondary).frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(minWidth: 360)
        }
        }
        .onAppear { store.reload(); if selection == nil { selection = store.presets.first?.name } }
        .alert("Restore the default presets?", isPresented: $confirmRestore) {
            Button("Restore", role: .destructive) { store.restoreDefaults(); selection = store.presets.first?.name }
            Button("Cancel", role: .cancel) {}
        } message: { Text("This deletes every file in the Presets folder, including your own presets, and puts back the 21 built-in Format and Quality presets.") }
        .overlay(alignment: .bottom) {
            if let e = store.loadError ?? unknownKeysWarning {
                HStack {
                    Text(e).font(.caption).foregroundStyle(.red).textSelection(.enabled)
                    Button("Show File") { revealProblem() }.controlSize(.small)
                }.padding(6).background(.bar, in: .rect(cornerRadius: 6)).padding(.bottom, 44)
            }
        }
    }

    /// A misspelt option key in a hand-edited preset: the command line skips that preset until it is fixed.
    private var unknownKeysWarning: String? {
        let schema = SchemaStore.shared.schema
        guard !schema.properties.isEmpty else { return nil }
        let bad = store.presets.flatMap { p in p.options.values.keys.filter { !schema.has($0) }.sorted().map { "“\($0)” in \(p.name)" } }
        guard let first = bad.first else { return nil }
        return "Unknown option \(first)\(bad.count > 1 ? " and \(bad.count - 1) more" : ""). The command line skips that preset until it is fixed."
    }

    // Preset files are plain JSON shared with the CLI; edits and dropped-in files are picked up when the app
    // comes back to the front.
    private func openFolder() {
        store.ensureFile()
        NSWorkspace.shared.open(store.directory)
    }
    private func revealSelected() {
        store.ensureFile()
        if let p = selection.flatMap(store.preset(named:)), let u = store.fileURL(for: p) { NSWorkspace.shared.activateFileViewerSelecting([u]) }
    }
    /// The skipped file, or the selected preset's file for an unknown-option warning.
    private func revealProblem() {
        if let f = store.problems.first?.file {
            NSWorkspace.shared.activateFileViewerSelecting([store.directory.appendingPathComponent(f)])
        } else {
            let schema = SchemaStore.shared.schema
            let bad = store.presets.first { p in p.options.values.keys.contains { !schema.has($0) } }
            if let u = bad.flatMap(store.fileURL(for:)) { NSWorkspace.shared.activateFileViewerSelecting([u]) } else { openFolder() }
        }
    }
}

struct PresetDetail: View {
    @Binding var preset: Preset
    @Binding var axes: PresetAxes
    var customFormats: [String] = []
    var customQualities: [String] = []
    var onChange: () -> Void
    var onRename: (String) -> Void
    @State private var name = ""
    @AppStorage("presetShowAdvanced") private var showAdvanced = false
    @State private var settings = AppSettings.shared
    @State private var schemaStore = SchemaStore.shared

    private var inherited: Options { settings.base.merged(over: schemaStore.schema.defaults) }

    var body: some View {
        Form {
            Section {
                TextField("Name", text: $name).onSubmit { onRename(name) }
                Toggle("Show in Finder menu", isOn: Binding(get: { preset.showInFinder }, set: { preset.showInFinder = $0; onChange() }))
                if preset.isDestructive {
                    Label("This preset replaces original files (\(preset.options.originalsMode)).", systemImage: "exclamationmark.triangle")
                        .font(.caption).foregroundStyle(.orange)
                }
            }
            Section {
                Toggle("Favorite", isOn: $axes.favorite)
                TagPicker(title: "Format", tag: $axes.format, known: AxisTags.knownFormats, custom: customFormats, label: AxisTags.formatTitle)
                TagPicker(title: "Quality", tag: $axes.quality, known: AxisTags.knownQualities, custom: customQualities, label: AxisTags.qualityTitle)
                if axes.favorite && !axes.isTagged {
                    Text("Choose both a format and a quality for this preset to appear in the toolbar.").font(.caption).foregroundStyle(.orange)
                }
            } header: { Text("Toolbar") } footer: {
                Text("Favorite presets with a format and quality fill the toolbar's Format and Quality controls. Presets without tags are listed under Presets and in the Process menu instead.")
            }
            let edited = Binding(get: { preset.options }, set: { preset.options = $0; onChange() })
            Section("Output") {
                OptionsForm(schema: schemaStore.schema, specs: OptionCatalog.presetCommon, inherited: inherited, edited: edited)
            }
            Section {
                DisclosureGroup("Advanced", isExpanded: $showAdvanced) {
                    ForEach(OptionCatalog.presetAdvancedPanels) { panel in
                        if panel.specs.contains(where: { schemaStore.schema.has($0.key) }) {
                            Text(panel.title).font(.headline).padding(.top, 8)
                            OptionsForm(schema: schemaStore.schema, specs: panel.specs, inherited: inherited, edited: edited)
                        }
                    }
                }
            }
        }
        .formStyle(.grouped)
        .onAppear { name = preset.name }
        .onDisappear { if name != preset.name { onRename(name) } }
    }
}

/// A format or quality tag: the well-known ones, tags already used by other presets, None, and "Other…" for a new label.
struct TagPicker: View {
    let title: String
    @Binding var tag: String?
    let known: [String]
    let custom: [String]
    let label: (String) -> String
    @State private var other = ""
    @State private var entering = false
    private static let otherTag = "\u{1}other"

    var body: some View {
        Picker(title, selection: Binding(get: { tag }, set: { if $0 == Self.otherTag { other = ""; entering = true } else { tag = $0; entering = false } })) {
            Text("None").tag(String?.none)
            Divider()
            ForEach(known + custom, id: \.self) { Text(label($0)).tag(Optional($0)) }
            Divider()
            Text("Other…").tag(Optional(Self.otherTag))
        }
        if entering {
            TextField("\(title) label", text: $other)
                .onSubmit { commit() }
            HStack { Spacer(); Button("Cancel") { entering = false }; Button("Use Label") { commit() }.disabled(other.trimmingCharacters(in: .whitespaces).isEmpty) }
        }
    }
    private func commit() {
        let t = other.trimmingCharacters(in: .whitespaces)
        if !t.isEmpty { tag = t }
        entering = false
    }
}

struct ToolsSettings: View {
    @State private var output = "Loading…"
    @State private var showLicenses = false
    @State private var licenses = ""

    var body: some View {
        Form {
            Section {
              DisclosureGroup("Details") {
                Text(output).font(.system(.body, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                Text(CLI.executable().path).font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
              }
            } header: { Text("Bundled engine") } footer: {
                Text("The helper tools that do the compressing. Nothing to configure here.")
            }
            Section {
                Button("Licenses…") {
                    Task { let r = await CLI.exec(["licenses"]); licenses = r.ok ? r.text : "License notices are not available in this build."; showLicenses = true }
                }
                Button("Install Command Line Tool…") { Task { await CLIInstaller.run() } }
            } footer: {
                Text("The command line tool shares your presets and defaults, so scripts behave like the app.")
            }
            Section("Updates") {
                Toggle("Check for updates automatically", isOn: Binding(get: { Updater.shared.automaticChecks }, set: { Updater.shared.automaticChecks = $0 }))
                Button("Check Now") { Updater.shared.checkForUpdates() }
            }
        }
        .formStyle(.grouped)
        .task {
            let r = await CLI.exec(["tools"])
            output = r.ok ? r.text.trimmingCharacters(in: .whitespacesAndNewlines) : "imgcompress could not be run: \(r.stderr)"
        }
        .sheet(isPresented: $showLicenses) {
            VStack {
                ScrollView { Text(licenses).font(.system(.caption, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }
                Button("Done") { showLicenses = false }.keyboardShortcut(.defaultAction)
            }.padding().frame(width: 640, height: 480)
        }
    }
}

struct FinderSettingsView: View {
    @State private var enabled = FIFinderSyncController.isExtensionEnabled
    @State private var settings = FinderSettings.read()

    var body: some View {
        Form {
            Section {
                LabeledContent("Status") {
                    Label(enabled ? "Enabled" : "Not enabled", systemImage: enabled ? "checkmark.circle.fill" : "exclamationmark.circle")
                        .foregroundStyle(enabled ? .green : .orange)
                }
                Button("Open Extensions Settings…") { FIFinderSyncController.showExtensionManagementInterface() }
                Text("Enable “ImageCompress Finder Menu” under Finder Extensions. Managed Macs can block extensions silently.")
                    .font(.caption).foregroundStyle(.secondary)
            } header: { Text("Extension") } footer: {
                Text("The Finder menu runs the presets you mark “Show in Finder menu” in Presets.")
            }
            Section("Menu") {
                TextField("Menu title", text: Binding(get: { settings.menuTitle }, set: { settings.menuTitle = $0; save() }))
                Picker("Choosing a preset", selection: Binding(get: { settings.mode }, set: { settings.mode = $0; save() })) {
                    Text("Compress immediately with that preset").tag("presets")
                    Text("Open in ImageCompress…").tag("open")
                }
                Text("Which presets appear is set per preset with “Show in Finder menu”.").font(.caption).foregroundStyle(.secondary)
            }
        }
        .formStyle(.grouped)
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in
            enabled = FIFinderSyncController.isExtensionEnabled
        }
    }

    private func save() { try? settings.write() }
}
