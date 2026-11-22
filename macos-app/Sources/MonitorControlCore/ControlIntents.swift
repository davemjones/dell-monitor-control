import AppIntents

/// The intents the Control Center controls run. They live in this framework so
/// the app and the controls extension share the same types, and they always run
/// in the app (`.main`), which isn't sandboxed and can run monitorctl.
public struct MonitorControlIntents: AppIntentsPackage {}

/// What the app does when a control is clicked; installed by the app at launch.
@MainActor
public protocol ControlActionHandler: AnyObject {
    func showBrightnessPanel()
    func showInputPanel()
    func showSettings()
}

@MainActor
public enum ControlActions {
    public static weak var handler: (any ControlActionHandler)?
}

public struct ShowBrightnessPanelIntent: AppIntent {
    public static let title: LocalizedStringResource = "Show Monitor Brightness"
    public static let description = IntentDescription("Opens a brightness slider for the external monitor.")
    public static let isDiscoverable = false
    public static var allowedExecutionTargets: IntentExecutionTargets { .main }

    public init() {}

    public func perform() async throws -> some IntentResult {
        await MainActor.run { ControlActions.handler?.showBrightnessPanel() }
        return .result()
    }
}

public struct ShowInputPanelIntent: AppIntent {
    public static let title: LocalizedStringResource = "Show Monitor Inputs"
    public static let description = IntentDescription("Opens the list of the external monitor's inputs.")
    public static let isDiscoverable = false
    public static var allowedExecutionTargets: IntentExecutionTargets { .main }

    public init() {}

    public func perform() async throws -> some IntentResult {
        await MainActor.run { ControlActions.handler?.showInputPanel() }
        return .result()
    }
}

public struct OpenMonitorControlSettingsIntent: AppIntent {
    public static let title: LocalizedStringResource = "Open Monitor Control Settings"
    public static let description = IntentDescription("Opens Monitor Control's settings, where you name the monitor's inputs.")
    public static let isDiscoverable = false
    public static var allowedExecutionTargets: IntentExecutionTargets { .main }

    public init() {}

    public func perform() async throws -> some IntentResult {
        await MainActor.run { ControlActions.handler?.showSettings() }
        return .result()
    }
}
