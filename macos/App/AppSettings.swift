import Foundation
import Observation

/// App-level settings. `base` holds engine options applied under every preset
/// (the Settings panels edit it); presets and one-run overrides layer on top.
@MainActor @Observable
final class AppSettings {
    static let shared = AppSettings()

    private enum Key {
        static let base = "baseOptions"
        static let defaultPreset = "defaultPreset"
        static let confirmTrash = "confirmTrash"
        static let notify = "notifyOnCompletion"
        static let sound = "playSound"
        static let finderMode = "finderMode"
        static let autoProcess = "autoProcess"
        static let keepUndo = "keepUndo"
    }
    private let d = UserDefaults.standard

    var base: Options { didSet { if let data = try? JSONEncoder().encode(base) { d.set(data, forKey: Key.base) } } }
    var defaultPreset: String { didSet { d.set(defaultPreset, forKey: Key.defaultPreset) } }
    /// Ask before moving originals to the Trash (deleting always asks).
    var confirmTrash: Bool { didSet { d.set(confirmTrash, forKey: Key.confirmTrash) } }
    var notifyOnCompletion: Bool { didSet { d.set(notifyOnCompletion, forKey: Key.notify) } }
    /// Start compressing as soon as files are dropped, added or opened.
    var autoProcess: Bool { didSet { d.set(autoProcess, forKey: Key.autoProcess) } }
    /// Keep a copy of every replaced or removed original for a day so a file can be restored.
    var keepUndo: Bool { didSet { d.set(keepUndo, forKey: Key.keepUndo) } }
    var playSound: Bool { didSet { d.set(playSound, forKey: Key.sound) } }

    init() {
        if let data = UserDefaults.standard.data(forKey: Key.base), let o = try? JSONDecoder().decode(Options.self, from: data) { base = o } else { base = Options() }
        defaultPreset = UserDefaults.standard.string(forKey: Key.defaultPreset) ?? ""   // "": no preset, the toolbar alone
        confirmTrash = UserDefaults.standard.bool(forKey: Key.confirmTrash)
        notifyOnCompletion = UserDefaults.standard.object(forKey: Key.notify) as? Bool ?? true
        playSound = UserDefaults.standard.bool(forKey: Key.sound)
        autoProcess = UserDefaults.standard.object(forKey: Key.autoProcess) as? Bool ?? true
        keepUndo = UserDefaults.standard.object(forKey: Key.keepUndo) as? Bool ?? true
    }

    /// base < preset < one-run overrides.
    static func effective(base: Options, preset: Preset?, overrides: Options) -> Options {
        overrides.merged(over: (preset?.options ?? Options()).merged(over: base))
    }
}
