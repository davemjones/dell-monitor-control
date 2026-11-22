import AppIntents
import AppKit
import MonitorControlCore
import SwiftUI

/// Makes the control intents in the MonitorControlCore framework known to the app.
struct AppIntentsPackages: AppIntentsPackage {
    static var includedPackages: [any AppIntentsPackage.Type] { [MonitorControlIntents.self] }
}

@main
struct MonitorControlApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    /// Off by default: Control Center controls cover most uses.
    @AppStorage(Preferences.showMenuBarIcon) private var showMenuBarIcon = false

    var body: some Scene {
        MenuBarExtra("Monitor Control", systemImage: "display", isInserted: $showMenuBarIcon) {
            PanelView(model: delegate.model)
        }
        .menuBarExtraStyle(.window)
    }
}

enum Preferences {
    static let showMenuBarIcon = "showMenuBarIcon"
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, ControlActionHandler {
    let model = StatusModel()
    private lazy var settings = SettingsWindow(model: model)
    private lazy var controlPanel = ControlPanel(model: model)
    private var gate = LaunchGate(isLoginItem: false)

    override init() {
        super.init()
        // Before launch finishes, so an action that started the app finds it.
        ControlActions.handler = self
        model.openSettings = { [weak self] in self?.settings.show() }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let actionArrived = gate.actionArrived
        gate = LaunchGate(isLoginItem: launchedAsLoginItem())
        if actionArrived { gate.noteAction() }
        Task {
            try? await Task.sleep(for: LaunchGate.grace)
            if gate.showsSettings { settings.show() }
        }
    }

    /// Opening the app while it runs (Finder, Spotlight, Launchpad) shows Settings.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        settings.show()
        return false
    }

    // MARK: Control Center actions

    func showBrightnessPanel() {
        gate.noteAction()
        controlPanel.show(.brightness)
    }

    func showInputPanel() {
        gate.noteAction()
        controlPanel.show(.inputs)
    }

    func showSettings() {
        gate.noteAction()
        controlPanel.close()
        settings.show()
    }

    /// Whether macOS started the app as a login item, read from its launch event.
    private func launchedAsLoginItem() -> Bool {
        guard let event = NSAppleEventManager.shared().currentAppleEvent,
            event.eventID == kAEOpenApplication
        else { return false }
        return event.paramDescriptor(forKeyword: keyAEPropData)?.enumCodeValue
            == keyAELaunchedAsLogInItem
    }
}
