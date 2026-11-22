// swift-tools-version: 6.0

import Foundation
import PackageDescription

// With only the Command Line Tools installed, a clean `swift test` can't find
// the Swift Testing macros unless their plugin folder is passed explicitly.
let cltTestingPlugins = "/Library/Developer/CommandLineTools/usr/lib/swift/host/plugins/testing"
let testSettings: [SwiftSetting] =
    FileManager.default.fileExists(atPath: cltTestingPlugins)
    ? [.unsafeFlags(["-plugin-path", cltTestingPlugins])] : []

// Only the app's testable core lives here, so `swift test` runs without Xcode.
// The app itself (App/), and a framework built from these same sources, are
// built by MonitorControl.xcodeproj.
let package = Package(
    name: "MonitorControl",
    platforms: [.macOS("27.0")],
    targets: [
        // Talks to the bundled monitorctl and turns its answers into panel state.
        .target(name: "MonitorControlCore"),
        .testTarget(
            name: "MonitorControlCoreTests",
            dependencies: ["MonitorControlCore"],
            swiftSettings: testSettings
        ),
    ]
)
