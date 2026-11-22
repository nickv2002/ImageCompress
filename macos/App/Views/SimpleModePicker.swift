import SwiftUI

/// The two questions that matter, as two toolbar controls: which format ("Preserve format" or
/// "Convert to X") and how much loss is acceptable. Their choices are the distinct tags among the favorited
/// presets; picking a pair applies that preset's options as one-run overrides, so they compose with presets.
@MainActor struct GoalModel {
    let store: JobStore
    let inherited: Options

    private var effective: Options { store.overrides.merged(over: inherited) }
    var axisSet: AxisSet { store.presets.axisSet }
    /// The favorite the effective options correspond to; nil when customised beyond all of them.
    var match: AxisPreset? { axisSet.matching(effective, prefer: store.axisChoice) }

    private func choose(format: String?, quality: String?, touched: AxisSet.Axis) {
        let f = format ?? match?.format ?? store.axisChoice?.format ?? axisSet.formats.first
        let q = quality ?? match?.quality ?? store.axisChoice?.quality ?? axisSet.qualities.first
        guard let f, let q, let p = axisSet.resolve(format: f, quality: q, touched: touched) else { return }
        withAnimation(.snappy) { store.choose(p) }
    }

    /// nil means the effective format is not one of the offered tags (shown as "Custom").
    var format: Binding<String?> {
        Binding(
            get: {
                if let m = match { return m.format }
                let f = effective.string("format") ?? AxisTags.keep
                return axisSet.formats.contains(f) ? f : nil
            },
            set: { if let f = $0 { choose(format: f, quality: nil, touched: .format) } })
    }
    /// nil means the options were customised beyond the favorites (Advanced or a saved preset).
    var quality: Binding<String?> {
        Binding(get: { match?.quality }, set: { if let q = $0 { choose(format: nil, quality: q, touched: .quality) } })
    }
}

struct FormatMenu: View {
    let goal: GoalModel
    var body: some View {
        let formats = goal.axisSet.formats
        Picker("Format", selection: goal.format) {
            ForEach(Array(formats.enumerated()), id: \.element) { i, f in
                if i > 0, AxisTags.formatGroup(f) != AxisTags.formatGroup(formats[i - 1]) { Divider() }
                Text(AxisTags.formatTitle(f)).tag(Optional(f))
            }
            if goal.format.wrappedValue == nil { Divider(); Text("Custom").tag(Optional<String>.none) }
        }
        .pickerStyle(.menu)
        .accessibilityLabel("Output format")
        .help(goal.match.map { AxisSet.outcome(format: $0.format, quality: $0.quality) } ?? "Output format")
    }
}

struct QualityPicker: View {
    let goal: GoalModel
    var body: some View {
        Picker("Quality", selection: goal.quality) {
            ForEach(goal.axisSet.qualities, id: \.self) { Text(AxisTags.qualityTitle($0)).tag(Optional($0)) }
            if goal.quality.wrappedValue == nil { Text("Custom").tag(Optional<String>.none) }
        }
        .pickerStyle(.segmented)
        .accessibilityLabel("Quality")
        .help(goal.match.map { (AxisTags.qualityHint($0.quality).map { $0 + " " } ?? "") + AxisSet.outcome(format: $0.format, quality: $0.quality) } ?? "Customised in Advanced options.")
    }
}
