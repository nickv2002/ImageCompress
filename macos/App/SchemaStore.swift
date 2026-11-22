import Foundation
import Observation

/// The engine's Options schema; the Settings UI renders only the options that exist in it.
@MainActor @Observable
final class SchemaStore {
    static let shared = SchemaStore()
    var schema = OptionsSchema()
    var error: String?

    func load() async {
        let r = await CLI.exec(["options", "schema"])
        if r.ok, let s = OptionsSchema.parse(r.stdout) { schema = s; error = nil }
        else { error = r.stderr.isEmpty ? "Could not read the option schema from imgcompress." : r.stderr }
    }
}
