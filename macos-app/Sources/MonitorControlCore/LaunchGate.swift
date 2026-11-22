/// Whether launching the app should open Settings.
///
/// With the menu bar icon off, Settings is the only sign the app opened, so a
/// person opening it (Finder, Spotlight, Launchpad) should see it. The system
/// starting it at login, or to run a Control Center action, shouldn't. A
/// person's launch and an action's launch look the same to the app, so it
/// waits `grace` and opens Settings only if no action arrived meanwhile.
public struct LaunchGate: Sendable {
    public static let grace: Duration = .milliseconds(500)

    /// macOS started the app because it's a login item.
    public let isLoginItem: Bool
    /// A control or Shortcuts action arrived since launch.
    public private(set) var actionArrived = false

    public init(isLoginItem: Bool) {
        self.isLoginItem = isLoginItem
    }

    public mutating func noteAction() {
        actionArrived = true
    }

    /// Checked once the grace period has passed.
    public var showsSettings: Bool {
        !isLoginItem && !actionArrived
    }
}
