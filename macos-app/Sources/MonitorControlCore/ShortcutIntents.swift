import AppIntents
import Foundation

/// A failed monitorctl run, reported to Shortcuts with monitorctl's own message.
public struct MonitorctlFailure: Error, CustomLocalizedStringResourceConvertible {
    public let message: String

    public var localizedStringResource: LocalizedStringResource { "\(message)" }
}

/// The inputs from `monitorctl input list --json`, or why they couldn't be read.
public func parseInputList(_ result: RunResult) throws -> [InputEntry] {
    if let message = failureMessage(of: result) {
        throw MonitorctlFailure(message: message)
    }
    guard case .finished(_, let stdout, _) = result,
        let inputs = try? JSONDecoder().decode([InputEntry].self, from: stdout)
    else {
        throw MonitorctlFailure(message: "monitorctl gave an answer the app couldn't read.")
    }
    return inputs
}

/// One of the monitor's inputs, as Shortcuts shows it: the user's name for it,
/// with the built-in name underneath, or just the built-in name.
public struct MonitorInputEntity: AppEntity {
    public static let typeDisplayRepresentation: TypeDisplayRepresentation = "Monitor Input"
    public static let defaultQuery = MonitorInputQuery()

    /// The input code, e.g. "0x11", which is what `input set` is given.
    public let id: String
    public let name: String
    public let builtInName: String

    public init(_ input: InputEntry) {
        id = input.code
        name = input.displayName
        builtInName = input.builtInLabel
    }

    public var displayRepresentation: DisplayRepresentation {
        name == builtInName
            ? DisplayRepresentation(title: "\(name)")
            : DisplayRepresentation(title: "\(name)", subtitle: "\(builtInName)")
    }
}

/// Lists the monitor's inputs for Shortcuts, read live through the app.
public struct MonitorInputQuery: EntityQuery {
    public static var allowedExecutionTargets: IntentExecutionTargets { .main }

    public init() {}

    public func entities(for identifiers: [String]) async throws -> [MonitorInputEntity] {
        let inputs = try await suggestedEntities()
        return identifiers.compactMap { id in inputs.first { $0.id == id } }
    }

    public func suggestedEntities() async throws -> [MonitorInputEntity] {
        try parseInputList(await ControlActions.run(["input", "list", "--json"]))
            .map(MonitorInputEntity.init)
    }
}

public struct SwitchMonitorInputIntent: AppIntent {
    public static let title: LocalizedStringResource = "Switch Monitor Input"
    public static let description = IntentDescription("Switches your external monitor to another input.")
    public static var allowedExecutionTargets: IntentExecutionTargets { .main }
    public static var parameterSummary: some ParameterSummary {
        Summary("Switch monitor to \(\.$input)")
    }

    @Parameter(title: "Input")
    public var input: MonitorInputEntity

    public init() {}

    public func perform() async throws -> some IntentResult {
        let result = await ControlActions.run(["input", "set", input.id, "--json"])
        if let message = failureMessage(of: result) {
            throw MonitorctlFailure(message: message)
        }
        return .result()
    }
}

public struct SetMonitorBrightnessIntent: AppIntent {
    public static let title: LocalizedStringResource = "Set Monitor Brightness"
    public static let description = IntentDescription("Sets your external monitor's brightness, from 0 to 100.")
    public static var allowedExecutionTargets: IntentExecutionTargets { .main }
    public static var parameterSummary: some ParameterSummary {
        Summary("Set monitor brightness to \(\.$brightness)")
    }

    @Parameter(title: "Brightness", inclusiveRange: (0, 100))
    public var brightness: Int

    public init() {}

    public func perform() async throws -> some IntentResult {
        let result = await ControlActions.run(["brightness", "\(brightness)", "--json"])
        if let message = failureMessage(of: result) {
            throw MonitorctlFailure(message: message)
        }
        return .result()
    }
}
