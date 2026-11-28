import Foundation

struct PreviewResult: Sendable {
    var output: URL
    var inBytes: UInt64
    var outBytes: UInt64
}

enum PreviewError: Error, LocalizedError {
    case failed(String)
    var errorDescription: String? { if case .failed(let m) = self { m } else { nil } }
}

/// Encodes one file with the effective options into a scratch location for the before/after view.
enum Preview {
    static func size(_ url: URL) -> UInt64 {
        ((try? FileManager.default.attributesOfItem(atPath: url.path)[.size]) as? NSNumber)?.uint64Value ?? 0
    }

    /// Uses `imgcompress preview FILE --out PATH` and falls back to compressing a scratch copy
    /// with `run` when the installed engine has no `preview` command.
    static func render(file: URL, options: Options, executable: URL = CLI.executable()) async throws -> PreviewResult {
        let scratch = FileManager.default.temporaryDirectory.appendingPathComponent("imgcompress-preview-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: true)
        let optsFile = scratch.appendingPathComponent("options.json")
        var o = options
        o["originals"] = .string("keep"); o["destination"] = .string("in-place"); o["history"] = .bool(false)
        o["replace-if"] = .string("always")
        try JSONEncoder().encode(o).write(to: optsFile)

        let ext = file.pathExtension
        let out = scratch.appendingPathComponent("preview" + (ext.isEmpty ? "" : ".\(ext)"))
        let r = await CLI.exec(["preview", file.path, "--out", out.path, "--options-file", optsFile.path], executable: executable)
        if r.ok, FileManager.default.fileExists(atPath: out.path) {
            return PreviewResult(output: out, inBytes: size(file), outBytes: size(out))
        }
        // Fallback: process a copy in place and read the finished event.
        let copy = scratch.appendingPathComponent(file.lastPathComponent)
        try FileManager.default.copyItem(at: file, to: copy)
        let run = await CLI.exec(["run", "--json", "--options-file", optsFile.path, "--", copy.path], executable: executable)
        for line in run.text.split(separator: "\n") {
            if case .finished(_, let output, let i, let o, _, _, _, _)? = JobEvent.parse(line: String(line)) {
                return PreviewResult(output: URL(fileURLWithPath: output), inBytes: i, outBytes: o)
            }
            if case .skipped(_, let reason)? = JobEvent.parse(line: String(line)) {
                throw PreviewError.failed("Nothing to preview: \(reason)")
            }
            if case .failed(_, let e)? = JobEvent.parse(line: String(line)) { throw PreviewError.failed(e) }
        }
        throw PreviewError.failed(r.stderr.isEmpty ? run.stderr : r.stderr)
    }
}
