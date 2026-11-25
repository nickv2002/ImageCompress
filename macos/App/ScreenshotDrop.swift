import Foundation

/// The floating thumbnail macOS shows after ⇧⌘3/4/5 can be dragged into ImageCompress. The drag hands over a
/// file URL inside a temporary folder (…/T/TemporaryItems/NSIRD_screencaptureui_XXXX/Screenshot ….png), and
/// once the thumbnail has been dragged out macOS no longer saves it to the Desktop, so that temporary file is the
/// only copy. `adopt` moves such a file to where screenshots normally go, so the compressed result lives there.
/// (The drag also carries a file promise, but screencaptureui cancels it for other apps; the URL is what works.)
enum ScreenshotDrop {
    /// True for a file inside the screenshot tool's temporary folders.
    static func isThumbnailTempFile(_ url: URL) -> Bool {
        guard url.isFileURL else { return false }
        let comps = url.standardizedFileURL.pathComponents
        if comps.contains(where: { $0.hasPrefix("NSIRD_screencaptureui_") }) { return true }
        if let i = comps.firstIndex(of: "Containers"), i + 1 < comps.count, comps[i + 1] == "com.apple.screencaptureui" { return true }
        return false
    }

    /// The folder screenshots are saved to: `defaults read com.apple.screencapture location`, else ~/Desktop.
    static func saveLocation(configured: String? = CFPreferencesCopyAppValue("location" as CFString, "com.apple.screencapture" as CFString) as? String,
                             home: URL = FileManager.default.homeDirectoryForCurrentUser) -> URL {
        let desktop = home.appendingPathComponent("Desktop", isDirectory: true)
        guard var path = configured?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty else { return desktop }
        if path == "~" { path = home.path } else if path.hasPrefix("~/") { path = home.path + path.dropFirst(1) }
        var isDir: ObjCBool = false
        guard path.hasPrefix("/"), FileManager.default.fileExists(atPath: path, isDirectory: &isDir), isDir.boolValue else { return desktop }
        return URL(fileURLWithPath: path, isDirectory: true)
    }

    /// `name` in `folder`, or "name 2.ext", "name 3.ext", … if that is taken.
    static func freeDestination(for name: String, in folder: URL) -> URL {
        let fm = FileManager.default
        var candidate = folder.appendingPathComponent(name)
        let base = (name as NSString).deletingPathExtension, ext = (name as NSString).pathExtension
        var n = 2
        while fm.fileExists(atPath: candidate.path) {
            candidate = folder.appendingPathComponent(ext.isEmpty ? "\(base) \(n)" : "\(base) \(n).\(ext)")
            n += 1
        }
        return candidate
    }

    /// A screenshot-thumbnail file moved into the screenshot location; any other URL unchanged. Runs
    /// synchronously, while the drop is still being handled, so the file is safe before the drag session ends.
    /// If the move fails it tries a copy, and if that fails too the original URL is used as is.
    static func adopt(_ url: URL, location: URL = saveLocation()) -> URL {
        guard isThumbnailTempFile(url), FileManager.default.fileExists(atPath: url.path) else { return url }
        let fm = FileManager.default
        let dest = freeDestination(for: url.lastPathComponent, in: location)
        do {
            try fm.moveItem(at: url, to: dest)
            return dest
        } catch {
            if (try? fm.copyItem(at: url, to: dest)) != nil { return dest }
            return url
        }
    }
}
