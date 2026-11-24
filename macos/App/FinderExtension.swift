import AppKit

/// The Finder menu extension: whether it is on, why Finder might ignore it, and the fixes the app can make itself.
/// macOS keeps a new Finder extension off, and some macOS 15 builds have no switch for it in System Settings,
/// so the app turns it on with `pluginkit` (what that switch does) and restarts Finder to load it.
@MainActor
enum FinderExtension {
    static let id = "com.nickvance.imagecompress.FinderMenu"
    private static let promptedKey = "finderExtensionPrompted"

    struct Status: Equatable {
        var enabled: Bool
        /// Other copies of ImageCompress on this Mac. With more than one, Finder can silently skip the extension.
        var otherCopies: [URL]
    }

    /// Reads the state from `pluginkit`, which is what Finder goes by. `FIFinderSyncController.isExtensionEnabled`
    /// can report "off" for an extension that is on, especially while more than one copy is registered.
    static func status() async -> Status {
        let me = Bundle.main.bundleURL.resolvingSymlinksInPath().standardizedFileURL
        var enabled = false, mine = false, apps: [URL] = []
        let out = await output("/usr/bin/pluginkit", ["-m", "-A", "-D", "-v", "-i", id])
        for line in out.split(separator: "\n") where line.contains(id) {
            guard let path = line.split(separator: "\t").last.map(String.init), path.hasPrefix("/") else { continue }
            let appex = URL(fileURLWithPath: path)
            let app = appex.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
                .resolvingSymlinksInPath().standardizedFileURL
            if app == me {
                mine = true
                enabled = line.hasPrefix("+")
            } else {
                if !mine && line.hasPrefix("+") { enabled = true }   // our own copy may not be registered yet
                apps.append(app)
            }
        }
        if let appID = Bundle.main.bundleIdentifier {
            apps += NSWorkspace.shared.urlsForApplications(withBundleIdentifier: appID).map { $0.resolvingSymlinksInPath().standardizedFileURL }
        }
        var seen = Set<URL>()
        let others = apps.filter { $0 != me && FileManager.default.fileExists(atPath: $0.path) && seen.insert($0).inserted }
        return Status(enabled: enabled, otherCopies: others)
    }

    /// Turns the extension on and restarts Finder. Returns whether macOS now reports it as on.
    @discardableResult
    static func enable() async -> Bool {
        await run("/usr/bin/pluginkit", ["-e", "use", "-i", id])
        restartFinder()
        return await status().enabled
    }

    /// Finder reopens by itself, with its windows, and loads extensions again.
    static func restartFinder() {
        Task { await run("/usr/bin/killall", ["Finder"]) }
    }

    /// Asks once, on a launch where the extension is off, whether to turn it on.
    static func offerOnce() async {
        let d = UserDefaults.standard
        guard !d.bool(forKey: promptedKey), await !status().enabled else { return }
        d.set(true, forKey: promptedKey)
        let alert = NSAlert()
        alert.messageText = "Turn on the Finder menu?"
        alert.informativeText = """
            ImageCompress can add a submenu to Finder's right-click menu, so you can compress images without opening \
            the app. macOS keeps it off until you turn it on. Finder restarts briefly to load it.

            You can change this later in Settings → Finder.
            """
        alert.addButton(withTitle: "Turn On")
        alert.addButton(withTitle: "Not Now")
        guard alert.runModal() == .alertFirstButtonReturn, await !enable() else { return }
        let failed = NSAlert()
        failed.messageText = "The Finder menu is still off"
        failed.informativeText = "Open Settings → Finder in ImageCompress to see what is stopping it. On a Mac managed by your employer or school, extensions can be blocked."
        failed.runModal()
    }

    private static func run(_ tool: String, _ args: [String]) async { _ = await output(tool, args) }

    private static func output(_ tool: String, _ args: [String]) async -> String {
        await withCheckedContinuation { cont in
            DispatchQueue.global().async {
                let p = Process(), pipe = Pipe()
                p.executableURL = URL(fileURLWithPath: tool)
                p.arguments = args
                p.standardOutput = pipe
                p.standardError = FileHandle.nullDevice
                guard (try? p.run()) != nil else { return cont.resume(returning: "") }
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                p.waitUntilExit()
                cont.resume(returning: String(decoding: data, as: UTF8.self))
            }
        }
    }
}
