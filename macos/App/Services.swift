import AppKit

/// NSServices entries (Finder > Services) that hand files to the app.
@MainActor final class ServicesProvider: NSObject {
    private func files(_ pboard: NSPasteboard) -> [URL] {
        (pboard.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]) ?? []
    }

    /// Adds the files and, with `start`, compresses them with the window's current settings.
    private func run(_ pboard: NSPasteboard, start: Bool) {
        let store = JobStore.shared
        store.presets.reload()
        store.add(files(pboard))
        NSApp.activate()
        if start { store.process() }
    }

    @objc func compressService(_ pboard: NSPasteboard, userData: String, error: AutoreleasingUnsafeMutablePointer<NSString>) {
        run(pboard, start: true)
    }
    /// Runs "Preserve · Smaller" the way a Finder menu choice does: the window goes back to its own settings afterwards.
    @objc func compressSmallerService(_ pboard: NSPasteboard, userData: String, error: AutoreleasingUnsafeMutablePointer<NSString>) {
        let paths = files(pboard).map(\.path)
        guard !paths.isEmpty, let url = try? JobFile(preset: "keep-smaller", paths: paths, start: true).write() else { return }
        NSApp.activate()
        JobStore.shared.handleOpen([url])
    }
    @objc func openService(_ pboard: NSPasteboard, userData: String, error: AutoreleasingUnsafeMutablePointer<NSString>) {
        run(pboard, start: false)
    }
}
