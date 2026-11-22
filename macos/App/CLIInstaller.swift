import AppKit

/// "Install Command Line Tool…": `imgcompress install` when the engine has it, else a symlink.
@MainActor
enum CLIInstaller {
    static func run() async {
        let exe = CLI.executable()
        let r = await CLI.exec(["install"])
        var message: String
        if r.ok {
            message = r.text.trimmingCharacters(in: .whitespacesAndNewlines)
            if message.isEmpty { message = "Installed imgcompress." }
        } else if r.status == 2 {  // this engine has no `install` command
            message = symlink(to: exe)
        } else {
            message = "Install failed: \(r.stderr)"
        }
        let alert = NSAlert()
        alert.messageText = "Command Line Tool"
        alert.informativeText = message
        alert.runModal()
    }

    private static func symlink(to exe: URL) -> String {
        let dest = "/usr/local/bin/imgcompress"
        let fm = FileManager.default
        do {
            try? fm.createDirectory(atPath: "/usr/local/bin", withIntermediateDirectories: true)
            try? fm.removeItem(atPath: dest)
            try fm.createSymbolicLink(atPath: dest, withDestinationPath: exe.path)
            return "Linked \(dest) to the bundled imgcompress."
        } catch {}
        let script = "do shell script \"mkdir -p /usr/local/bin && ln -sf '\(exe.path)' '\(dest)'\" with administrator privileges"
        var err: NSDictionary?
        NSAppleScript(source: script)?.executeAndReturnError(&err)
        if let err { return "Could not install: \(err[NSAppleScript.errorMessage] ?? "unknown error")" }
        return "Linked \(dest) to the bundled imgcompress."
    }
}
