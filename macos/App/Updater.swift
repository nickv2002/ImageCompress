import Sparkle
import Observation

/// Sparkle wrapper: "Check for Updates…" menu item and the automatic-check preference.
@MainActor @Observable
final class Updater {
    static let shared = Updater()
    private let controller = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)

    var automaticChecks: Bool {
        get { controller.updater.automaticallyChecksForUpdates }
        set { controller.updater.automaticallyChecksForUpdates = newValue }
    }
    func checkForUpdates() { controller.checkForUpdates(nil) }
}
