import AppIntents
import MonitorControlCore
import SwiftUI
import WidgetKit

/// Makes the control intents in the MonitorControlCore framework known to the extension.
struct ControlsIntentsPackages: AppIntentsPackage {
    static var includedPackages: [any AppIntentsPackage.Type] { [MonitorControlIntents.self] }
}

/// Monitor Control's Control Center controls. Each is a button whose action
/// runs in the app (see `ControlIntents.swift`): Control Center only offers
/// buttons and toggles, so brightness and inputs open small panels from the app.
@main
struct ControlsBundle: WidgetBundle {
    var body: some Widget {
        BrightnessControl()
        InputControl()
        SettingsControl()
    }
}

struct BrightnessControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "com.davemjones.monitor-control.brightness") {
            ControlWidgetButton(action: ShowBrightnessPanelIntent()) {
                Label("Monitor Brightness", systemImage: "sun.max")
            }
        }
        .displayName("Monitor Brightness")
        .description("Opens a brightness slider for your external monitor.")
    }
}

struct InputControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "com.davemjones.monitor-control.input") {
            ControlWidgetButton(action: ShowInputPanelIntent()) {
                Label("Monitor Input", systemImage: "rectangle.2.swap")
            }
        }
        .displayName("Monitor Input")
        .description("Opens a list of your monitor's inputs to switch between.")
    }
}

struct SettingsControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "com.davemjones.monitor-control.settings") {
            ControlWidgetButton(action: OpenMonitorControlSettingsIntent()) {
                Label("Monitor Control Settings", systemImage: "gearshape")
            }
        }
        .displayName("Monitor Control Settings")
        .description("Opens Monitor Control's settings, where you name your monitor's inputs.")
    }
}
