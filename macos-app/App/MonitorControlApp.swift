import AppKit
import MonitorControlCore
import SwiftUI

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
final class AppDelegate: NSObject, NSApplicationDelegate {
    let model = StatusModel()
    private lazy var settings = SettingsWindow(model: model)
    private var gate = LaunchGate(isLoginItem: false)

    func applicationDidFinishLaunching(_ notification: Notification) {
        model.openSettings = { [weak self] in self?.settings.show() }
        gate = LaunchGate(isLoginItem: launchedAsLoginItem())
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

    /// Whether macOS started the app as a login item, read from its launch event.
    private func launchedAsLoginItem() -> Bool {
        guard let event = NSAppleEventManager.shared().currentAppleEvent,
            event.eventID == kAEOpenApplication
        else { return false }
        return event.paramDescriptor(forKeyword: keyAEPropData)?.enumCodeValue
            == keyAELaunchedAsLogInItem
    }
}
