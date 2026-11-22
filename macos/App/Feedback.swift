import AppKit
import UserNotifications

/// Dock tile progress + badge, and the completion notification (with a Reveal action).
@MainActor
enum Feedback {
    nonisolated static let revealAction = "REVEAL"
    nonisolated static let category = "JOB_DONE"

    private final class DockView: NSView {
        var fraction: Double = 0
        override func draw(_ dirtyRect: NSRect) {
            NSApp.applicationIconImage.draw(in: bounds)
            let bar = NSRect(x: bounds.width * 0.1, y: bounds.height * 0.08, width: bounds.width * 0.8, height: bounds.height * 0.1)
            NSColor.black.withAlphaComponent(0.55).setFill()
            NSBezierPath(roundedRect: bar, xRadius: bar.height / 2, yRadius: bar.height / 2).fill()
            NSColor.systemBlue.setFill()
            var fill = bar.insetBy(dx: 1.5, dy: 1.5); fill.size.width *= fraction
            NSBezierPath(roundedRect: fill, xRadius: fill.height / 2, yRadius: fill.height / 2).fill()
        }
    }
    private static let dockView = DockView()

    static func setUp() {
        let center = UNUserNotificationCenter.current()
        let reveal = UNNotificationAction(identifier: revealAction, title: "Reveal in Finder", options: [.foreground])
        center.setNotificationCategories([UNNotificationCategory(identifier: category, actions: [reveal], intentIdentifiers: [])])
        center.requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    static func update(done: Int, total: Int) {
        let tile = NSApp.dockTile
        guard total > 0 else { return }
        dockView.frame = NSRect(x: 0, y: 0, width: 128, height: 128)
        dockView.fraction = min(1, Double(done) / Double(total))
        tile.contentView = dockView
        tile.badgeLabel = done < total ? "\(total - done)" : nil
        tile.display()
    }

    /// `revealing`: this run's result files; the notification's button selects them in Finder.
    static func finish(totals: Totals?, error: String?, revealing: [URL]) {
        let tile = NSApp.dockTile
        tile.contentView = nil
        tile.badgeLabel = nil
        tile.display()
        guard AppSettings.shared.notifyOnCompletion, !NSApp.isActive else { return }
        let content = UNMutableNotificationContent()
        content.title = error == nil ? "Compression finished" : "Compression stopped"
        if let t = totals {
            let saved = t.inBytes > t.outBytes ? t.inBytes - t.outBytes : 0
            let f = ByteCountFormatter.string(fromByteCount: Int64(saved), countStyle: .file)
            content.body = "\(t.processed) processed, \(t.skipped) skipped, \(t.failed) failed. Saved \(f)."
        } else { content.body = error ?? "" }
        content.categoryIdentifier = category
        if AppSettings.shared.playSound { content.sound = .default }
        if !revealing.isEmpty { content.userInfo = ["paths": revealing.prefix(50).map(\.path)] }
        UNUserNotificationCenter.current().add(UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil))
    }
}

final class NotificationDelegate: NSObject, UNUserNotificationCenterDelegate, @unchecked Sendable {
    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        guard response.actionIdentifier == Feedback.revealAction || response.actionIdentifier == UNNotificationDefaultActionIdentifier,
              let paths = response.notification.request.content.userInfo["paths"] as? [String], !paths.isEmpty else { return }
        await MainActor.run { NSWorkspace.shared.activateFileViewerSelecting(paths.map { URL(fileURLWithPath: $0) }) }
    }
}
