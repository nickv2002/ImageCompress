import AppKit

/// "Install Command Line Tool…": says so if `imgcompress` is already installed (by Homebrew, or linked to this app
/// earlier); otherwise links /usr/local/bin/imgcompress to the copy inside the app, asking for an administrator
/// password when the folder is not writable.
@MainActor
enum CLIInstaller {
    static let dest = "/usr/local/bin/imgcompress"

    /// Where `imgcompress` is usually found. A GUI app does not get the shell's PATH, so look in the usual places.
    static var searchDirs: [String] {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return ["/opt/homebrew/bin", "/usr/local/bin", "\(home)/.local/bin", "\(home)/bin"]
    }

    struct Found {
        var path: String
        var version: String
        /// The command is this app's own bundled copy (through a symlink, as the Homebrew cask and this menu make it).
        var isThisApp: Bool
    }

    static func existing() async -> [Found] {
        let mine = CLI.executable().resolvingSymlinksInPath().standardizedFileURL
        var found: [Found] = []
        for dir in searchDirs {
            let path = "\(dir)/imgcompress"
            guard FileManager.default.isExecutableFile(atPath: path) else { continue }
            let url = URL(fileURLWithPath: path)
            let r = await CLI.exec(["--version"], executable: url)
            let version = r.text.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "imgcompress ", with: "")
            found.append(Found(path: path, version: version.isEmpty ? "unknown version" : version,
                               isThisApp: url.resolvingSymlinksInPath().standardizedFileURL == mine))
        }
        return found
    }

    static func run() async {
        let found = await existing()
        let alert = NSAlert()
        alert.messageText = "Command Line Tool"
        if let f = found.first {
            alert.informativeText = describe(found) + "\n\nOpen Terminal and run imgcompress --help to get started."
            if !found.contains(where: \.isThisApp) && f.path != dest {
                alert.addButton(withTitle: "OK")
                alert.addButton(withTitle: "Also Link This App’s Copy…")
                if alert.runModal() == .alertSecondButtonReturn { report(await install()) }
                return
            }
            alert.runModal()
            return
        }
        report(await install())
    }

    private static func describe(_ found: [Found]) -> String {
        let lines = found.map { f in
            let how = f.isThisApp ? "this app’s copy" : (f.path.hasPrefix("/opt/homebrew") || isHomebrew(f.path) ? "from Homebrew" : "another copy")
            return "\(f.path): \(f.version), \(how)"
        }
        let head = found.count == 1 ? "imgcompress is already installed:" : "imgcompress is already installed in more than one place. The first one on your PATH is used:"
        return ([head] + lines).joined(separator: "\n")
    }

    private static func isHomebrew(_ path: String) -> Bool {
        URL(fileURLWithPath: path).resolvingSymlinksInPath().path.contains("/Cellar/")
    }

    private static func report(_ message: String) {
        let alert = NSAlert()
        alert.messageText = "Command Line Tool"
        alert.informativeText = message
        alert.runModal()
    }

    /// Links /usr/local/bin/imgcompress to the bundled CLI, with an administrator prompt if needed.
    private static func install() async -> String {
        let exe = CLI.executable()
        let r = await CLI.exec(["install"])
        if r.ok {
            let message = r.text.trimmingCharacters(in: .whitespacesAndNewlines)
            return message.isEmpty ? "Installed imgcompress." : message
        }
        return symlink(to: exe)   // no `install` command in this engine, or the folder needs an administrator
    }

    private static func symlink(to exe: URL) -> String {
        let fm = FileManager.default
        do {
            try? fm.createDirectory(atPath: "/usr/local/bin", withIntermediateDirectories: true)
            try? fm.removeItem(atPath: dest)
            try fm.createSymbolicLink(atPath: dest, withDestinationPath: exe.path)
            return "Linked \(dest) to the bundled imgcompress."
        } catch {}
        let script = "do shell script \"mkdir -p /usr/local/bin && ln -sf '\(exe.path)' '\(dest)'\" with prompt \"ImageCompress wants to link the imgcompress command into /usr/local/bin.\" with administrator privileges"
        var err: NSDictionary?
        NSAppleScript(source: script)?.executeAndReturnError(&err)
        if let err {
            if (err[NSAppleScript.errorNumber] as? Int) == -128 { return "Not installed: the administrator prompt was cancelled." }
            return "Could not install: \(err[NSAppleScript.errorMessage] ?? "unknown error")"
        }
        return "Linked \(dest) to the bundled imgcompress."
    }
}
