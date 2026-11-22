import Cocoa
import FinderSync

/// Top-level "ImageCompress" submenu in Finder's context menu.
/// Presets come from the Presets folder (those with show_in_finder). Choosing one writes a job file
/// into the App Group container and opens imagecompress://job/<uuid>; the host app does the work.
final class FinderSync: FIFinderSync {
    private static let openInAppTag = -1

    override init() {
        super.init()
        FIFinderSyncController.default().directoryURLs = [URL(fileURLWithPath: "/")]
    }

    override var toolbarItemName: String { "ImageCompress" }
    override var toolbarItemToolTip: String { "Compress the selected images" }
    override var toolbarItemImage: NSImage { NSImage(systemSymbolName: "photo.on.rectangle", accessibilityDescription: "ImageCompress") ?? NSImage() }

    private func finderPresets() -> [Preset] {
        (PresetFolder.load()?.presets ?? []).filter(\.showInFinder)   // the app writes the folder at launch
    }

    override func menu(for menuKind: FIMenuKind) -> NSMenu? {
        guard menuKind == .contextualMenuForItems || menuKind == .contextualMenuForContainer
                || menuKind == .toolbarItemMenu else { return nil }
        let settings = FinderSettings.read()
        let title = settings.menuTitle.isEmpty ? "ImageCompress" : settings.menuTitle
        let sub = NSMenu(title: title)
        for (i, p) in (settings.mode == "open" ? [] : finderPresets()).enumerated() {
            let item = NSMenuItem(title: p.name, action: #selector(runPreset(_:)), keyEquivalent: "")
            item.tag = i
            sub.addItem(item)
        }
        if sub.items.isEmpty == false { sub.addItem(.separator()) }
        let open = NSMenuItem(title: "Open in ImageCompress…", action: #selector(runPreset(_:)), keyEquivalent: "")
        open.tag = Self.openInAppTag
        sub.addItem(open)

        let menu = NSMenu(title: "")
        let top = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        top.image = NSImage(systemSymbolName: "photo.on.rectangle", accessibilityDescription: nil)
        top.submenu = sub
        menu.addItem(top)
        return menu
    }

    /// Action methods must live on the FIFinderSync subclass: Finder copies the menu items.
    @objc func runPreset(_ sender: Any?) {
        guard let item = sender as? NSMenuItem else { return }
        let controller = FIFinderSyncController.default()
        var urls = controller.selectedItemURLs() ?? []
        if urls.isEmpty, let target = controller.targetedURL() { urls = [target] }
        guard !urls.isEmpty else { return }

        var job = JobFile(preset: nil, overrides: nil, paths: urls.map(\.path), start: false)
        if item.tag != Self.openInAppTag {
            let list = finderPresets()
            guard list.indices.contains(item.tag), list[item.tag].name == item.title else { return }
            job.preset = list[item.tag].name
            job.start = true
        }
        do {
            let url = try job.write()
            NSWorkspace.shared.open(url)
        } catch {
            NSLog("ImageCompress FinderMenu: could not hand off job: \(error)")
        }
    }
}
