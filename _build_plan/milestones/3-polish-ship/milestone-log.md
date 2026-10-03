## What's new in the app

- **Launch at login:** a checkbox at the bottom of the panel adds Monitor Control to System Settings › General › Login Items. The checkbox always matches what System Settings says. If you switched it off there, an "Allow in Login Items…" link takes you straight to that page.
- **The app version** (the same as `dellctl`'s) is shown at the bottom of the panel.
- **The README has a "Menu bar app (macOS)" section** covering how to build, install, and first-launch the app, plus its caveats.
- **Clearer messages:** errors from `dellctl` start with a capital letter, and a failed status read now says "Couldn't read the monitor".
- The build script tells you what to do with the finished app.
- **An app icon:** a white monitor on a blue tile, shown in Finder, Spotlight, and System Settings › Login Items. (Added after review, at the user's request.)

---

## What was built

- `macos-app/Sources/MonitorControl/LoginItem.swift` (new): a wrapper over `SMAppService.mainApp` with `state` (`on` / `off` / `needsApproval`), `set(_:)`, and `openSettings()`.
- `StatusModel.swift`:
  - `loginItem` is re-read on every panel open, in `refresh()`. It doesn't use dellctl or the serial queue.
  - New `loginItemError` and `setLaunchAtLogin(_:)`.
- `PanelView.swift`: a new shared `Footer`, shown in the loading, ready, and problem states. It has the Launch at login checkbox, the approval link, the login-item error, Quit, and "Version x.y.z" read from `CFBundleShortVersionString`.
- `MonitorControlCore`:
  - `errorMessage` now sentence-cases dellctl's messages.
  - `Problem.failed` is titled "Couldn't read the monitor".
  - Tests were updated to match.
- `macos-app/Package.swift`: passes `-plugin-path …/CommandLineTools/usr/lib/swift/host/plugins/testing` to the test target, only when that folder exists.
- `macos-app/build.sh`: prints the install hint.
- App icon:
  - `macos-app/Icon/make-icon.swift` draws the icon as vectors at every iconset size.
  - `build.sh` turns it into `target/AppIcon.icns` with `iconutil`, redrawing only when the script is newer (drawing takes ~13s). It bundles the icon as `Contents/Resources/AppIcon.icns` with `CFBundleIconFile`.
  - The icon is deliberately **not** the SF Symbol used in the menu bar, because Apple's SF Symbols license forbids symbols in app icons. It's a hand-drawn monitor in the same outline style.
- `README.md`: an intro mention, the new "Menu bar app (macOS)" section, and a Known-limitations bullet.
- Installed to `/Applications/Monitor Control.app` (with the user's OK) and launched from there.

## Decisions not pre-specified in the PRD

- **`requiresApproval`** (registered, but disabled in System Settings) shows as unchecked plus an "Allow in Login Items…" link, rather than as checked.
- **Version source:** the version comes from the app's Info.plist, which `build.sh` fills from Cargo.toml. That's the same number as the bundled dellctl, without running it.
- **Switch-away caveat:** the README points to the monitor's buttons for switching back. The existing Known-limitations entry covers the tested setup, where the Mac can still reach the monitor after switching away.

## Anything a later phase needs to know

- **Swift Testing under the Command Line Tools:** a clean `swift test` couldn't find the Testing macros (`plugin for module 'TestingMacros' not found`), while incremental runs worked. The `Package.swift` flag fixes it. It may be unnecessary, though harmless, with full Xcode.
- **No SwiftUI macros:** `@State`, `#Preview`, and other SwiftUI macros still don't build with only the Command Line Tools. Keep view state on the `@Observable` model.
- **Rebuilds change the signature:** each rebuild changes the ad-hoc signature. If Launch at login stops working after reinstalling, toggle it off and on again.

## Deviations from the PRD

- None.

## Verification status

- Automated:
  - A clean `swift test` passes (14 tests), and `cargo test` passes (54).
  - `build.sh` produces the app, and the copy in `/Applications` is running.
- Manual: the user confirmed everything works, including the icon:
  - Footer contents.
  - Launch at login and its System Settings round trip.
  - Logging out and back in.
  - Light and dark appearance.
