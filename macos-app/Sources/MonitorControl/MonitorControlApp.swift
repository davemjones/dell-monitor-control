import SwiftUI

@main
struct MonitorControlApp: App {
    // A plain `let`, not `@State`: SwiftUI's macros need full Xcode to build,
    // and the App is only created once anyway.
    private let model = StatusModel()

    var body: some Scene {
        MenuBarExtra("Monitor Control", systemImage: "display") {
            PanelView(model: model)
        }
        .menuBarExtraStyle(.window)
    }
}
