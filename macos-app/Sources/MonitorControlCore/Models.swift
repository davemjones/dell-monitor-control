import Foundation

/// The shapes of `monitorctl --json` output that the app reads.

public struct InputRef: Decodable, Equatable, Sendable {
    public let name: String
    public let code: String
    /// The user's own name for the input; nil if it has none.
    public let label: String?
}

public struct InputEntry: Decodable, Equatable, Sendable {
    public let name: String
    public let code: String
    /// The user's own name for the input; nil if it has none.
    public let label: String?
    public let active: Bool

    public init(name: String, code: String, label: String? = nil, active: Bool) {
        self.name = name
        self.code = code
        self.label = label
        self.active = active
    }
}

/// `monitorctl status --json` for a single monitor. Unreadable settings are nil.
public struct MonitorStatus: Decodable, Equatable, Sendable {
    public let name: String
    public private(set) var input: InputRef?
    public private(set) var inputs: [InputEntry]?
    public let brightness: Int?

    /// This status after a successful switch to the input with `code`.
    public func switched(to code: String) -> MonitorStatus {
        var status = self
        status.inputs = inputs?.map {
            InputEntry(name: $0.name, code: $0.code, label: $0.label, active: $0.code == code)
        }
        if let target = inputs?.first(where: { $0.code == code }) {
            status.input = InputRef(name: target.name, code: target.code, label: target.label)
        }
        return status
    }

    /// This status after the input with `code` was given `label` (nil clears it).
    public func renamed(code: String, to label: String?) -> MonitorStatus {
        var status = self
        status.inputs = inputs?.map {
            $0.code == code
                ? InputEntry(name: $0.name, code: $0.code, label: label, active: $0.active)
                : $0
        }
        if let input, input.code == code {
            status.input = InputRef(name: input.name, code: input.code, label: label)
        }
        return status
    }
}

/// `{"error": {"kind": …, "message": …}}`, printed to stderr on failure.
struct ErrorPayload: Decodable {
    struct Body: Decodable {
        let kind: String
        let message: String
    }

    let error: Body

    static func decode(_ stderr: Data) -> Body? {
        (try? JSONDecoder().decode(ErrorPayload.self, from: stderr))?.error
    }
}
