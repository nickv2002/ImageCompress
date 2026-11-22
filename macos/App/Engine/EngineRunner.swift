import Foundation

/// Splits a byte stream into complete lines.
struct LineSplitter {
    private var buffer = Data()
    mutating func feed(_ data: Data) -> [String] {
        buffer.append(data)
        var lines: [String] = []
        while let i = buffer.firstIndex(of: 0x0A) {
            lines.append(String(decoding: buffer[buffer.startIndex..<i], as: UTF8.self))
            buffer.removeSubrange(buffer.startIndex...i)
        }
        return lines
    }
    mutating func flush() -> String? {
        defer { buffer.removeAll() }
        return buffer.isEmpty ? nil : String(decoding: buffer, as: UTF8.self)
    }
}

enum EngineError: Error, LocalizedError {
    case exited(Int32, String)
    case launch(String)
    var errorDescription: String? {
        switch self {
        case .exited(let code, let err): return "imgcompress exited with status \(code)\(err.isEmpty ? "" : ": \(err)")"
        case .launch(let m): return "Could not launch imgcompress: \(m)"
        }
    }
}

/// Runs `imgcompress run --json --options-file F -- PATHS` and streams parsed events.
final class EngineRunner: @unchecked Sendable {
    let executable: URL
    private let lock = NSLock()
    private var process: Process?
    private var stopRequested = false

    init(executable: URL) { self.executable = executable }

    static func bundledExecutable() -> URL { CLI.executable() }

    func run(paths: [String], options: Options, backupDir: URL? = nil) -> AsyncThrowingStream<JobEvent, Error> {
        AsyncThrowingStream { continuation in
            let optionsFile = FileManager.default.temporaryDirectory
                .appendingPathComponent("imgcompress-\(UUID().uuidString).json")
            do {
                try JSONEncoder().encode(options).write(to: optionsFile)
            } catch {
                continuation.finish(throwing: EngineError.launch("\(error)"))
                return
            }
            let p = Process()
            p.executableURL = executable
            p.arguments = ["run", "--json", "--options-file", optionsFile.path] + (backupDir.map { ["--backup-dir", $0.path] } ?? []) + ["--"] + paths
            let out = Pipe(), err = Pipe()
            p.standardOutput = out
            p.standardError = err
            p.standardInput = FileHandle.nullDevice

            let state = StreamState()
            out.fileHandleForReading.readabilityHandler = { h in
                let d = h.availableData
                guard !d.isEmpty else { return }
                for l in state.feedOut(d) { if let e = JobEvent.parse(line: l) { continuation.yield(e) } }
            }
            err.fileHandleForReading.readabilityHandler = { h in
                let d = h.availableData
                if !d.isEmpty { state.appendErr(d) }
            }
            p.terminationHandler = { proc in
                out.fileHandleForReading.readabilityHandler = nil
                err.fileHandleForReading.readabilityHandler = nil
                let rest = (try? out.fileHandleForReading.readToEnd()) ?? Data()
                for l in state.feedOut(rest) + state.flushOut() { if let e = JobEvent.parse(line: l) { continuation.yield(e) } }
                if let e = try? err.fileHandleForReading.readToEnd() { state.appendErr(e) }
                try? FileManager.default.removeItem(at: optionsFile)
                self.lock.withLock { self.process = nil }
                // Exit 1 = some files failed (reported as events); other codes are errors.
                // A user-requested Stop ends in SIGINT's 130 after the engine cleans up: not an error.
                let stoppedByUser = self.lock.withLock { self.stopRequested }
                if proc.terminationReason == .exit, proc.terminationStatus > 1, !(stoppedByUser && proc.terminationStatus == 130) {
                    continuation.finish(throwing: EngineError.exited(proc.terminationStatus, state.errText()))
                } else {
                    continuation.finish()
                }
            }
            continuation.onTermination = { [weak self] reason in
                if case .cancelled = reason { self?.stop() }
            }
            do {
                try p.run()
                lock.withLock { process = p }
            } catch {
                continuation.finish(throwing: EngineError.launch("\(error)"))
            }
        }
    }

    /// SIGINT: the engine finishes the in-flight work and cleans up temp files.
    func stop() {
        lock.withLock { stopRequested = true }
        lock.withLock { process }?.interrupt()
    }
}

private final class StreamState: @unchecked Sendable {
    private let lock = NSLock()
    private var splitter = LineSplitter()
    private var err = Data()
    func feedOut(_ d: Data) -> [String] { lock.withLock { splitter.feed(d) } }
    func flushOut() -> [String] { lock.withLock { splitter.flush().map { [$0] } ?? [] } }
    func appendErr(_ d: Data) { lock.withLock { err.append(d) } }
    func errText() -> String {
        lock.withLock { String(decoding: err, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines) }
    }
}
