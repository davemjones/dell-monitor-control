import Foundation

/// monitorctl's own message for a failed run, in sentence case for the panel: the
/// JSON error's message, or its raw stderr if that isn't JSON.
func errorMessage(_ stderr: Data) -> String {
    let text =
        ErrorPayload.decode(stderr)?.message
        ?? String(decoding: stderr, as: UTF8.self)
        .trimmingCharacters(in: .whitespacesAndNewlines)
    guard let first = text.first else { return "monitorctl stopped without saying why." }
    return first.uppercased() + text.dropFirst()
}

/// Why a change (switching input, setting brightness) didn't happen, or nil
/// if it succeeded.
public func failureMessage(of result: RunResult) -> String? {
    switch result {
    case .finished(exitCode: 0, _, _): nil
    case .finished(_, _, let stderr): errorMessage(stderr)
    case .launchFailed: Problem.cantRunMonitorctl.title
    case .timedOut: Problem.timedOut.title
    }
}

/// The input as renamed by `monitorctl input name … --json`.
public func parseRename(_ result: RunResult) -> InputRef? {
    struct Renamed: Decodable { let input: InputRef }
    guard case .finished(exitCode: 0, let stdout, _) = result else { return nil }
    return (try? JSONDecoder().decode(Renamed.self, from: stdout))?.input
}

/// The level from `monitorctl brightness --json`.
public func parseBrightness(_ result: RunResult) -> Int? {
    struct Reading: Decodable { let brightness: Int }
    guard case .finished(exitCode: 0, let stdout, _) = result else { return nil }
    return (try? JSONDecoder().decode(Reading.self, from: stdout))?.brightness
}

/// Turns a stream of slider values into as few brightness writes as possible:
/// one write at a time, skipping values that were overtaken while it ran, but
/// always ending on the last value asked for.
public struct BrightnessWriter: Sendable {
    private var writing = false
    private var pending: Int?
    private var lastWritten: Int?

    /// `current` is the monitor's level, so asking for it again writes nothing.
    public init(current: Int? = nil) {
        lastWritten = current
    }

    /// A write is in flight, so the slider is ahead of the monitor.
    public var isWriting: Bool { writing }

    /// The slider moved to `value`. Returns a level to write now, if any.
    public mutating func want(_ value: Int) -> Int? {
        if writing {
            pending = value
            return nil
        }
        guard value != lastWritten else { return nil }
        writing = true
        lastWritten = value
        return value
    }

    /// The write in flight succeeded. Returns the next level to write, if any.
    public mutating func finished() -> Int? {
        writing = false
        guard let next = pending else { return nil }
        pending = nil
        return want(next)
    }

    /// The write in flight failed; drop anything queued behind it.
    public mutating func failed(current: Int?) {
        self = BrightnessWriter(current: current)
    }
}
