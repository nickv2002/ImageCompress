import Foundation

struct TraceEntry: Decodable, Equatable, Sendable {
    var stage: String
    var bytes: UInt64
    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        stage = try c.decode(String.self)
        bytes = try c.decode(UInt64.self)
    }
    init(stage: String, bytes: UInt64) { self.stage = stage; self.bytes = bytes }
}

struct Totals: Equatable, Sendable {
    var processed = 0, skipped = 0, failed = 0
    var inBytes: UInt64 = 0, outBytes: UInt64 = 0
}

/// Mirrors `compress_core::events::JobEvent` (serde tag "event").
enum JobEvent: Decodable, Equatable, Sendable {
    case queued(files: Int)
    case started(path: String)
    case stage(path: String, stage: String)
    case finished(path: String, output: String, inBytes: UInt64, outBytes: UInt64, encoder: String, trace: [TraceEntry], undo: UndoInfo? = nil, note: String? = nil)
    case skipped(path: String, reason: String)
    case failed(path: String, error: String)
    case totals(Totals)
    case unknown(String)

    private enum K: String, CodingKey {
        case event, files, path, stage, output, encoder, trace, reason, error, undo, note
        case inBytes = "in_bytes", outBytes = "out_bytes"
        case processed, skipped, failed
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        let kind = try c.decode(String.self, forKey: .event)
        switch kind {
        case "queued": self = .queued(files: try c.decode(Int.self, forKey: .files))
        case "started": self = .started(path: try c.decode(String.self, forKey: .path))
        case "stage": self = .stage(path: try c.decode(String.self, forKey: .path), stage: try c.decode(String.self, forKey: .stage))
        case "finished":
            self = .finished(
                path: try c.decode(String.self, forKey: .path),
                output: try c.decode(String.self, forKey: .output),
                inBytes: try c.decode(UInt64.self, forKey: .inBytes),
                outBytes: try c.decode(UInt64.self, forKey: .outBytes),
                encoder: (try? c.decode(String.self, forKey: .encoder)) ?? "",
                trace: (try? c.decode([TraceEntry].self, forKey: .trace)) ?? [],
                undo: try? c.decode(UndoInfo.self, forKey: .undo),
                note: try? c.decode(String.self, forKey: .note))
        case "skipped": self = .skipped(path: try c.decode(String.self, forKey: .path), reason: try c.decode(String.self, forKey: .reason))
        case "failed": self = .failed(path: try c.decode(String.self, forKey: .path), error: try c.decode(String.self, forKey: .error))
        case "totals":
            self = .totals(Totals(
                processed: try c.decode(Int.self, forKey: .processed),
                skipped: try c.decode(Int.self, forKey: .skipped),
                failed: try c.decode(Int.self, forKey: .failed),
                inBytes: try c.decode(UInt64.self, forKey: .inBytes),
                outBytes: try c.decode(UInt64.self, forKey: .outBytes)))
        default: self = .unknown(kind)
        }
    }

    static func parse(line: String) -> JobEvent? {
        let t = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty, let data = t.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(JobEvent.self, from: data)
    }
}

/// Mirrors `compress_core::undo::Undo`.
struct UndoInfo: Codable, Equatable, Sendable {
    struct Restore: Codable, Equatable, Sendable { var path: String; var backup: String }
    var output: String
    var outSize: UInt64
    var outHash: String
    var restore: [Restore]
    private enum CodingKeys: String, CodingKey { case output, outSize = "out_size", outHash = "out_hash", restore }
}
