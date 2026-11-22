import Foundation

/// Locations shared between the app and the sandboxed Finder extension.
enum AppGroup {
    /// Team-ID-prefixed so macOS 15 does not show the "access data from other apps" prompt.
    static let identifier = "3D9SMH4RWM.com.nickvance.imagecompress"
    static let urlScheme = "imagecompress"

    static var containerURL: URL {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: identifier)
            ?? FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent("Library/Group Containers/\(identifier)")
    }
    /// `IMGCOMPRESS_CONFIG` (a directory) overrides the location, like the CLI.
    static var configDirectory: URL {
        if let p = ProcessInfo.processInfo.environment["IMGCOMPRESS_CONFIG"], !p.isEmpty { return URL(fileURLWithPath: p, isDirectory: true) }
        return containerURL
    }
    /// One JSON file per preset.
    static var presetsDirectory: URL { configDirectory.appendingPathComponent("Presets", isDirectory: true) }
    static var presetOrderURL: URL { configDirectory.appendingPathComponent("preset-order.json") }
    static var jobsDirectory: URL { configDirectory.appendingPathComponent("jobs") }
}

/// Finder-menu preferences shared with the sandboxed extension (a small JSON file in the App Group container).
struct FinderSettings: Codable, Equatable, Sendable {
    var menuTitle = "ImageCompress"
    /// "presets" = one-click presets; "open" = only "Open in ImageCompress…".
    var mode = "presets"

    static var url: URL { AppGroup.configDirectory.appendingPathComponent("finder.json") }
    static func read(from url: URL = url) -> FinderSettings {
        (try? JSONDecoder().decode(FinderSettings.self, from: Data(contentsOf: url))) ?? FinderSettings()
    }
    func write(to url: URL = url) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try JSONEncoder().encode(self).write(to: url, options: .atomic)
    }
}
