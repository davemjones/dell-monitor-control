import ServiceManagement

/// Launch at login, as a standard macOS Login Item (System Settings › General ›
/// Login Items), so it can be managed from either place.
enum LoginItem {
    enum State: Equatable {
        case on
        case off
        /// Registered, but switched off in System Settings.
        case needsApproval
    }

    static var state: State {
        switch SMAppService.mainApp.status {
        case .enabled: .on
        case .requiresApproval: .needsApproval
        default: .off
        }
    }

    static func set(_ on: Bool) throws {
        if on {
            try SMAppService.mainApp.register()
        } else {
            try SMAppService.mainApp.unregister()
        }
    }

    static func openSettings() {
        SMAppService.openSystemSettingsLoginItems()
    }
}
