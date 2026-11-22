import SwiftUI

/// Before/after split slider. The "after" image is produced by `imgcompress preview`.
struct CompareView: View {
    let item: JobItem
    let options: Options
    @Environment(\.dismiss) private var dismiss
    @State private var before: NSImage?
    @State private var after: NSImage?
    @State private var result: PreviewResult?
    @State private var error: String?
    @State private var split: CGFloat = 0.5
    /// A finished in-place run overwrote the source, so "before" is no longer the original file.
    private var replacedInPlace: Bool { item.status == .done && (item.output == nil || item.output == item.path) }

    var body: some View {
        VStack(spacing: 8) {
            ZStack {
                if let before, let after {
                    GeometryReader { geo in
                        ZStack {
                            Image(nsImage: after).resizable().scaledToFit().frame(width: geo.size.width, height: geo.size.height)
                            Image(nsImage: before).resizable().scaledToFit().frame(width: geo.size.width, height: geo.size.height)
                                .mask(alignment: .leading) { Rectangle().frame(width: geo.size.width * split) }
                            Rectangle().fill(.white).frame(width: 2).shadow(radius: 2).position(x: geo.size.width * split, y: geo.size.height / 2)
                        }
                        .contentShape(Rectangle())
                        .gesture(DragGesture(minimumDistance: 0).onChanged { split = min(max($0.location.x / geo.size.width, 0), 1) })
                    }
                } else if let error {
                    Text(error).foregroundStyle(.red).padding()
                } else {
                    ProgressView("Encoding preview…")
                }
            }
            .frame(minWidth: 640, minHeight: 420)
            if replacedInPlace {
                Text("This file was already compressed in place, so the original is gone. The preview re-encodes the current file with today's settings.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            HStack {
                Text("\(replacedInPlace ? "Current file" : "Original") \(fmt(result?.inBytes ?? Preview.size(item.url)))").foregroundStyle(.secondary)
                Spacer()
                Text(item.name).lineLimit(1)
                Spacer()
                if let r = result {
                    let pct = r.inBytes > 0 ? (1 - Double(r.outBytes) / Double(r.inBytes)) * 100 : 0
                    Text("Result \(fmt(r.outBytes)) (\(String(format: "%.1f", pct))% smaller)").foregroundStyle(.secondary)
                }
                Button("Done") { dismiss() }.keyboardShortcut(.cancelAction)
            }
        }
        .padding()
        .task {
            before = NSImage(contentsOf: item.url)
            do {
                let r = try await Preview.render(file: item.url, options: options)
                result = r; after = NSImage(contentsOf: r.output)
                if after == nil { error = "The preview could not be displayed." }
            } catch { self.error = error.localizedDescription }
        }
    }
}
