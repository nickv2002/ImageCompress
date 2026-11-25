import Foundation
import Testing

struct ScreenshotDropTests {
    private func tempDir() throws -> URL {
        let d = FileManager.default.temporaryDirectory.appendingPathComponent("ic-ss-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
        return d
    }

    @Test func recognisesThumbnailTempFiles() {
        #expect(ScreenshotDrop.isThumbnailTempFile(URL(fileURLWithPath: "/var/folders/7n/x/T/TemporaryItems/NSIRD_screencaptureui_A1s3ZQ/Screenshot 2026-11-24 at 5.35.37\u{202F}PM.png")))
        #expect(ScreenshotDrop.isThumbnailTempFile(URL(fileURLWithPath: "/Users/a/Library/Containers/com.apple.screencaptureui/Data/tmp/x/Screenshot.png")))
        #expect(!ScreenshotDrop.isThumbnailTempFile(URL(fileURLWithPath: "/Users/a/Desktop/Screenshot 2026-11-24 at 5.35.37\u{202F}PM.png")))
        #expect(!ScreenshotDrop.isThumbnailTempFile(URL(fileURLWithPath: "/var/folders/7n/x/T/TemporaryItems/NSIRD_xcodebuild_1/a.png")))
        #expect(!ScreenshotDrop.isThumbnailTempFile(URL(string: "https://example.com/NSIRD_screencaptureui_1/a.png")!))
    }

    @Test func saveLocationFallsBackToDesktop() throws {
        let home = try tempDir()
        let desktop = home.appendingPathComponent("Desktop", isDirectory: true)
        #expect(ScreenshotDrop.saveLocation(configured: nil, home: home).path == desktop.path)
        #expect(ScreenshotDrop.saveLocation(configured: "~/Nope", home: home).path == desktop.path)
        let shots = home.appendingPathComponent("Shots")
        try FileManager.default.createDirectory(at: shots, withIntermediateDirectories: true)
        #expect(ScreenshotDrop.saveLocation(configured: "~/Shots", home: home).path == shots.path)
        #expect(ScreenshotDrop.saveLocation(configured: shots.path, home: home).path == shots.path)
    }

    @Test func adoptMovesWithoutOverwriting() throws {
        let root = try tempDir()
        let tmp = root.appendingPathComponent("NSIRD_screencaptureui_test")
        let dest = root.appendingPathComponent("dest")
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: dest, withIntermediateDirectories: true)
        try Data("old".utf8).write(to: dest.appendingPathComponent("Shot.png"))
        let src = tmp.appendingPathComponent("Shot.png")
        try Data("new".utf8).write(to: src)

        let moved = ScreenshotDrop.adopt(src, location: dest)
        #expect(moved.lastPathComponent == "Shot 2.png")
        #expect(try String(contentsOf: moved, encoding: .utf8) == "new")
        #expect(try String(contentsOf: dest.appendingPathComponent("Shot.png"), encoding: .utf8) == "old")
        #expect(!FileManager.default.fileExists(atPath: src.path))

        let other = dest.appendingPathComponent("Shot.png")
        #expect(ScreenshotDrop.adopt(other, location: root) == other)   // ordinary files are left alone
    }
}
