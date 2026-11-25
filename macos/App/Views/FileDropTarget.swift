import SwiftUI
import AppKit

/// Accepts dropped files by reading their file URLs straight off the drag pasteboard. SwiftUI's own drop handling
/// (`dropDestination(for: URL.self)` and `onDrop`) prefers the file promise when a drag carries one as well, and
/// the macOS screenshot thumbnail cancels its promise for other apps, so those drops silently failed. Used as a
/// transparent overlay: it never takes clicks or scrolling, only drags.
struct FileDropTarget: NSViewRepresentable {
    @Binding var isTargeted: Bool
    var onDrop: ([URL]) -> Void

    func makeNSView(context: Context) -> DropView { DropView() }

    func updateNSView(_ view: DropView, context: Context) {
        view.onDrop = onDrop
        view.onTargeted = { targeted in if isTargeted != targeted { isTargeted = targeted } }
    }

    final class DropView: NSView {
        var onDrop: ([URL]) -> Void = { _ in }
        var onTargeted: (Bool) -> Void = { _ in }

        override init(frame: NSRect) {
            super.init(frame: frame)
            registerForDraggedTypes([.fileURL])
        }
        required init?(coder: NSCoder) { fatalError("not used") }

        override func hitTest(_ point: NSPoint) -> NSView? { nil }   // clicks and scrolling go to the content below

        private func fileURLs(_ info: NSDraggingInfo) -> [URL] {
            info.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL] ?? []
        }

        override func draggingEntered(_ info: NSDraggingInfo) -> NSDragOperation {
            guard !fileURLs(info).isEmpty else { return [] }
            onTargeted(true)
            return .copy
        }
        override func draggingUpdated(_ info: NSDraggingInfo) -> NSDragOperation { fileURLs(info).isEmpty ? [] : .copy }
        override func draggingExited(_ info: NSDraggingInfo?) { onTargeted(false) }
        override func draggingEnded(_ info: NSDraggingInfo) { onTargeted(false) }

        override func performDragOperation(_ info: NSDraggingInfo) -> Bool {
            let urls = fileURLs(info)
            onTargeted(false)
            guard !urls.isEmpty else { return false }
            onDrop(urls)   // synchronous, so a screenshot thumbnail's temp file is moved before the drag session ends
            return true
        }
    }
}
