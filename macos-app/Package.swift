// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "MonitorControl",
    platforms: [.macOS(.v14)],
    targets: [
        // Talks to the bundled dellctl and turns its answers into panel state.
        .target(name: "MonitorControlCore"),
        // The menu bar app itself.
        .executableTarget(name: "MonitorControl", dependencies: ["MonitorControlCore"]),
        .testTarget(name: "MonitorControlCoreTests", dependencies: ["MonitorControlCore"]),
    ]
)
