import SwiftUI
import UserNotifications

@MainActor final class AppDelegate: NSObject, NSApplicationDelegate {
    private let notifications = NotificationDelegate()
    private let services = ServicesProvider()

    private func installURLHandler() {
        // SwiftUI swallows URL-scheme events for a Window scene, so take them directly.
        NSAppleEventManager.shared().setEventHandler(self, andSelector: #selector(handleURLEvent(_:reply:)),
                                                     forEventClass: AEEventClass(kInternetEventClass), andEventID: AEEventID(kAEGetURL))
    }

    func applicationWillFinishLaunching(_ notification: Notification) { installURLHandler() }

    @objc func handleURLEvent(_ event: NSAppleEventDescriptor, reply: NSAppleEventDescriptor) {
        guard let s = event.paramDescriptor(forKeyword: keyDirectObject)?.stringValue, let url = URL(string: s) else { return }
        JobStore.shared.handleOpen([url])
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        installURLHandler()  // SwiftUI installs its own handler during launch; take over afterwards
        UNUserNotificationCenter.current().delegate = notifications
        Feedback.setUp()
        PresetStore.shared.ensureFile()   // the Finder menu reads it, and it is there to open and edit
        NSApp.servicesProvider = services
        NSUpdateDynamicServices()
    }

    func applicationWillTerminate(_ notification: Notification) { UndoStore.shared.flush() }

    // Dock drops, Open With, and imagecompress://job/<uuid> all arrive here
    // (avoids onOpenURL, which spawns extra windows).
    func application(_ application: NSApplication, open urls: [URL]) {
        JobStore.shared.handleOpen(urls)
    }
}

@main
struct ImageCompressApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    private let store = JobStore.shared
    private let presets = PresetStore.shared

    @AppStorage("showInspector") private var showInspector = false

    var body: some Scene {
        Window("ImageCompress", id: "main") {
            ContentView(store: store)
        }
        .windowToolbarStyle(.unified(showsTitle: false))
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Add Files…") { store.addViaPanel() }.keyboardShortcut("o")
            }
            CommandGroup(after: .appInfo) {
                Button("Check for Updates…") { Updater.shared.checkForUpdates() }
                Button("Install Command Line Tool…") { Task { await CLIInstaller.run() } }
            }
            ToolbarCommands()
            CommandGroup(after: .toolbar) {
                Toggle("Show Options", isOn: $showInspector).keyboardShortcut("i", modifiers: [.command, .option])
            }
            CommandGroup(replacing: .help) {
                Button("ImageCompress Help") { NSWorkspace.shared.open(Links.help) }
                    .keyboardShortcut("?", modifiers: .command)
                Button("Command Line Reference") { NSWorkspace.shared.open(Links.cli) }
                Divider()
                Button("Release Notes") { NSWorkspace.shared.open(Links.releases) }
                Button("Report an Issue…") { NSWorkspace.shared.open(Links.issues) }
                Divider()
                Button("Acknowledgements") { NSWorkspace.shared.open(Links.acknowledgements) }
            }
            CommandMenu("Process") {
                Button("Recent Compressions & Undo…") { store.showUndoHistory = true }
                    .keyboardShortcut("z", modifiers: [.command, .option])
                Divider()
                Button("Compress Waiting Images") { store.process() }
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(store.isRunning || !store.hasQueued)
                Button("Stop") { store.stop() }
                    .keyboardShortcut(".", modifiers: .command)
                    .disabled(!store.isRunning)
                if !presets.plainPresets.isEmpty { Divider() }
                ForEach(Array(presets.plainPresets.enumerated()), id: \.element.id) { i, p in
                    if i < 9 {
                        Button(p.name) { store.choosePreset(p.name) }
                            .keyboardShortcut(KeyEquivalent(Character("\(i + 1)")), modifiers: [.command, .control])
                    } else {
                        Button(p.name) { store.choosePreset(p.name) }
                    }
                }
            }
        }
        Settings { SettingsView() }
    }
}
