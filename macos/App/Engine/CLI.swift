import Foundation

struct CLIResult: Sendable {
    var status: Int32
    var stdout: Data
    var stderr: String
    var ok: Bool { status == 0 }
    var text: String { String(decoding: stdout, as: UTF8.self) }
}

/// One-shot invocations of the bundled `imgcompress` binary.
enum CLI {
    /// The CLI bundled next to the app executable (override with IMGCOMPRESS_BIN).
    static func executable() -> URL {
        if let p = ProcessInfo.processInfo.environment["IMGCOMPRESS_BIN"] { return URL(fileURLWithPath: p) }
        let dir = Bundle.main.executableURL?.deletingLastPathComponent()
        return (dir ?? URL(fileURLWithPath: "/usr/local/bin")).appendingPathComponent("imgcompress")
    }

    static func exec(_ args: [String], executable: URL = CLI.executable(), environment: [String: String] = [:]) async -> CLIResult {
        await Task.detached {
            let p = Process()
            p.executableURL = executable
            p.arguments = args
            if !environment.isEmpty { p.environment = ProcessInfo.processInfo.environment.merging(environment) { _, new in new } }
            let out = Pipe(), err = Pipe()
            p.standardOutput = out; p.standardError = err; p.standardInput = FileHandle.nullDevice
            do { try p.run() } catch {
                return CLIResult(status: 127, stdout: Data(), stderr: "\(error)")
            }
            let o = (try? out.fileHandleForReading.readToEnd()) ?? Data()
            let e = (try? err.fileHandleForReading.readToEnd()) ?? Data()
            p.waitUntilExit()
            return CLIResult(status: p.terminationStatus, stdout: o, stderr: String(decoding: e, as: UTF8.self))
        }.value
    }
}

/// One property of the engine's Options JSON Schema (`imgcompress options schema`).
struct SchemaProperty: Equatable, Sendable {
    var key: String
    var defaultValue: JSONValue
    var description: String?
    var enumValues: [String]?
}

struct OptionsSchema: Equatable, Sendable {
    var properties: [String: SchemaProperty] = [:]
    var isEmpty: Bool { properties.isEmpty }
    func has(_ key: String) -> Bool { properties[key] != nil }
    func defaultValue(_ key: String) -> JSONValue? { properties[key]?.defaultValue }
    var defaults: Options { Options(properties.mapValues(\.defaultValue)) }

    static func parse(_ data: Data) -> OptionsSchema? {
        guard let root = try? JSONDecoder().decode([String: JSONValue].self, from: data),
              case .object(let props)? = root["properties"] else { return nil }
        var defs: [String: JSONValue] = [:]
        if case .object(let d)? = root["$defs"] { defs = d }
        var result = OptionsSchema()
        for (key, v) in props {
            guard case .object(let o) = v else { continue }
            var enumValues: [String]?
            var target = o
            if case .string(let ref)? = o["$ref"], let name = ref.split(separator: "/").last, case .object(let d)? = defs[String(name)] {
                target = d
            }
            if case .array(let e)? = target["enum"] { enumValues = e.compactMap(\.stringValue) }
            else if case .array(let one)? = target["oneOf"] { enumValues = one.compactMap { if case .object(let x) = $0 { x["const"]?.stringValue } else { nil } } }
            var desc: String?
            if case .string(let s)? = o["description"] { desc = s }
            result.properties[key] = SchemaProperty(key: key, defaultValue: o["default"] ?? .null, description: desc, enumValues: enumValues)
        }
        return result
    }
}
