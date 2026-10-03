import Foundation

/// Something that stops the panel from showing the monitor.
public enum Problem: Equatable, Sendable {
    case cantRunDellctl
    case noMonitor
    case multipleMonitors
    case unreachable(name: String)
    case timedOut
    case failed(message: String)

    private static let ddcHint =
        "Check that DDC/CI is on in the monitor's menu (Others → DDC/CI) and that your cable or dock supports it."

    public var title: String {
        switch self {
        case .cantRunDellctl: "Couldn't run dellctl. Try reinstalling the app."
        case .noMonitor: "No Dell monitor detected"
        case .multipleMonitors: "Multiple monitors found — not supported yet."
        case .unreachable(let name): "Can't reach \(name)"
        case .timedOut: "The monitor didn't respond."
        case .failed: "Something went wrong"
        }
    }

    public var detail: String? {
        switch self {
        case .noMonitor, .unreachable: Self.ddcHint
        case .failed(let message): message
        case .cantRunDellctl, .multipleMonitors, .timedOut: nil
        }
    }
}

public enum PanelState: Equatable, Sendable {
    case loading
    case ready(MonitorStatus)
    case problem(Problem)

    /// The panel state for the result of `dellctl status --json`.
    public init(statusResult result: RunResult) {
        switch result {
        case .launchFailed:
            self = .problem(.cantRunDellctl)
        case .timedOut:
            self = .problem(.timedOut)
        case .finished(exitCode: 0, let stdout, _):
            self = Self.fromStatus(stdout)
        case .finished(_, _, let stderr):
            self = .problem(Self.problem(fromError: stderr))
        }
    }

    private static func fromStatus(_ json: Data) -> PanelState {
        // With several monitors connected, `status` lists them all as an array.
        if (try? JSONSerialization.jsonObject(with: json)) is [Any] {
            return .problem(.multipleMonitors)
        }
        guard let status = try? JSONDecoder().decode(MonitorStatus.self, from: json) else {
            return .problem(.failed(message: "dellctl gave an answer the app couldn't read."))
        }
        // Found, but nothing readable: DDC/CI is off or blocked.
        if status.input == nil && status.inputs == nil && status.brightness == nil {
            return .problem(.unreachable(name: status.name))
        }
        return .ready(status)
    }

    private static func problem(fromError stderr: Data) -> Problem {
        switch ErrorPayload.decode(stderr)?.kind {
        case "no_monitor": .noMonitor
        case "multiple_monitors": .multipleMonitors
        default: .failed(message: errorMessage(stderr))
        }
    }
}
