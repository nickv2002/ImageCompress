import SwiftUI

/// Renders the options in `specs` that exist in the schema. `edited` is the layer being edited
/// (a preset's partial options, the base options, or one-run overrides); `inherited` supplies the
/// values shown when a key is not set in `edited`.
struct OptionsForm: View {
    let schema: OptionsSchema
    let specs: [OptionSpec]
    let inherited: Options
    @Binding var edited: Options
    var allowReset = true
    /// What `edited` holds when untouched (the toolbar pair's values); a row only counts as changed when it differs.
    var baseline = Options()

    private var visible: [OptionSpec] { specs.filter { schema.has($0.key) } }

    var body: some View {
        ForEach(visible) { spec in
            OptionRow(spec: spec, schema: schema, inherited: inherited, edited: $edited, allowReset: allowReset, baseline: baseline)
        }
    }
}

struct OptionRow: View {
    let spec: OptionSpec
    let schema: OptionsSchema
    let inherited: Options
    @Binding var edited: Options
    var allowReset: Bool
    var baseline = Options()

    private var value: JSONValue { edited[spec.key] ?? inherited[spec.key] ?? schema.defaultValue(spec.key) ?? .null }
    private func set(_ v: JSONValue) { edited[spec.key] = v }
    private var isSet: Bool { edited[spec.key] != baseline[spec.key] }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(alignment: .firstTextBaseline) {
                Text(spec.title).fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 12)
                if !wideControl { control }
                if allowReset {
                    Button { edited[spec.key] = baseline[spec.key] } label: { Image(systemName: "arrow.uturn.backward") }
                        .buttonStyle(.borderless).help("Use the inherited value")
                        .opacity(isSet ? 1 : 0).disabled(!isSet)
                }
            }
            if wideControl { control.frame(maxWidth: .infinity, alignment: .leading) }
            Text(spec.hint).font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
        .padding(.vertical, 2)
    }

    /// Slider-based controls get their own line so they never squeeze the title.
    private var wideControl: Bool {
        switch spec.control {
        case .int, .optionalInt, .range: return true
        default: return false
        }
    }

    @ViewBuilder private var control: some View {
        switch spec.control {
        case .toggle:
            Toggle("", isOn: Binding(get: { value.boolValue ?? false }, set: { set(.bool($0)) })).labelsHidden()
        case .int(let r):
            IntField(value: Binding(get: { value.intValue ?? r.lowerBound }, set: { set(.int($0)) }), range: r)
        case .optionalInt(let r, let nullLabel, let whenOn):
            HStack(spacing: 8) {
                Toggle("", isOn: Binding(get: { !value.isNull }, set: { set($0 ? .int(whenOn) : .null) }))
                    .toggleStyle(.switch).labelsHidden().controlSize(.small)
                if value.isNull {
                    Text(nullLabel).foregroundStyle(.secondary)
                } else {
                    IntField(value: Binding(get: { value.intValue ?? whenOn }, set: { set(.int($0)) }), range: r)
                }
                Spacer(minLength: 0)
            }
        case .range(let r, let nullLabel):
            let pair: [Int] = { if case .array(let a) = value, a.count == 2 { return a.compactMap(\.intValue) } else { return [] } }()
            HStack(spacing: 8) {
                Toggle("", isOn: Binding(get: { !value.isNull }, set: { set($0 ? .array([.int(65), .int(85)]) : .null) }))
                    .toggleStyle(.switch).labelsHidden().controlSize(.small)
                if pair.count == 2 {
                    IntField(value: Binding(get: { pair[0] }, set: { set(.array([.int(min($0, pair[1])), .int(pair[1])])) }), range: r, width: 100)
                    Text("to")
                    IntField(value: Binding(get: { pair[1] }, set: { set(.array([.int(pair[0]), .int(max($0, pair[0]))])) }), range: r, width: 100)
                } else {
                    Text(nullLabel).foregroundStyle(.secondary)
                }
                Spacer(minLength: 0)
            }
        case .choice:
            Picker("", selection: Binding(get: { value.stringValue ?? "" }, set: { set(.string($0)) })) {
                ForEach(schema.properties[spec.key]?.enumValues ?? [], id: \.self) { Text(OptionCatalog.choiceTitle(spec.key, $0)).tag($0) }
            }.labelsHidden().fixedSize()
        case .intChoice(let options):
            Picker("", selection: Binding(get: { value.intValue ?? options[0] }, set: { set(.int($0)) })) {
                ForEach(options, id: \.self) { Text("\($0)").tag($0) }
            }.labelsHidden().pickerStyle(.segmented).fixedSize()
        case .destination:
            DestinationControl(value: Binding(get: { value.stringValue ?? "in-place" }, set: { set(.string($0)) }))
        case .text:
            TextField("", text: Binding(get: { value.stringValue ?? "" }, set: { set(.string($0)) }))
                .textFieldStyle(.roundedBorder).frame(minWidth: 90, idealWidth: 160, maxWidth: 220)
        case .kilobytes:
            IntField(value: Binding(get: { (value.intValue ?? 0) / 1024 }, set: { set(.int($0 * 1024)) }), range: 0...1_000_000, slider: false)
        }
    }
}

/// A slider paired with a numeric field.
struct IntField: View {
    @Binding var value: Int
    let range: ClosedRange<Int>
    var width: CGFloat = 220
    var slider = true

    var body: some View {
        HStack(spacing: 6) {
            if slider {
                Slider(value: Binding(get: { Double(value) }, set: { value = Int($0.rounded()) }),
                       in: Double(range.lowerBound)...Double(range.upperBound), step: 1)
                    .frame(minWidth: 60, idealWidth: min(width, 120), maxWidth: width)
            }
            TextField("", value: Binding(get: { value }, set: { value = min(max($0, range.lowerBound), range.upperBound) }), format: .number)
                .textFieldStyle(.roundedBorder).frame(width: 64).multilineTextAlignment(.trailing)
        }
    }
}

/// In place / Alongside / a chosen folder. The folder comes from an open panel, never typed.
struct DestinationControl: View {
    @Binding var value: String

    private enum Mode: Hashable { case inPlace, alongside, folder }
    private var mode: Mode { value == "in-place" ? .inPlace : value == "alongside" ? .alongside : .folder }

    var body: some View {
        HStack(spacing: 8) {
            Picker("", selection: Binding(get: { mode }, set: { select($0) })) {
                Text("Replace in place").tag(Mode.inPlace)
                Text("Alongside the original").tag(Mode.alongside)
                Text(mode == .folder ? "Folder: \((value as NSString).lastPathComponent)" : "Folder…").tag(Mode.folder)
            }.labelsHidden().fixedSize()
            if mode == .folder { Button("Choose…") { chooseFolder() } }
            Spacer(minLength: 0)
        }
    }

    private func select(_ m: Mode) {
        switch m {
        case .inPlace: value = "in-place"
        case .alongside: value = "alongside"
        case .folder: chooseFolder()
        }
    }

    private func chooseFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true; panel.canChooseFiles = false
        panel.canCreateDirectories = true; panel.allowsMultipleSelection = false
        panel.prompt = "Choose"; panel.message = "Choose where the results are saved."
        if mode == .folder { panel.directoryURL = URL(fileURLWithPath: value) }
        if panel.runModal() == .OK, let url = panel.url { value = url.path }
    }
}
