import AppIntents
import MonitorControlCore

/// Ready-made Shortcuts and Spotlight actions, with no setup needed.
struct MonitorControlShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: SwitchMonitorInputIntent(),
            phrases: [
                "Switch \(.applicationName) input",
                "Switch monitor input with \(.applicationName)",
            ],
            shortTitle: "Switch Monitor Input",
            systemImageName: "rectangle.2.swap"
        )
        AppShortcut(
            intent: SetMonitorBrightnessIntent(),
            phrases: [
                "Set \(.applicationName) brightness",
                "Set monitor brightness with \(.applicationName)",
            ],
            shortTitle: "Set Monitor Brightness",
            systemImageName: "sun.max"
        )
    }
}
