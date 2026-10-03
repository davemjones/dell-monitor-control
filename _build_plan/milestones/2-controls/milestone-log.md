## What's new in the app

- **Click an input to switch to it.** The monitor changes source, and the panel closes. A small spinner shows on the row while it works. Clicking the input that's already checked does nothing.
- **Inputs have readable names:** "Thunderbolt / USB-C", "DisplayPort", "HDMI".
- **The brightness slider works.** The monitor dims and brightens as you drag and always ends on the number where you let go. The panel stays open while you adjust.
- **When a change fails, a short red message appears in the panel.** If a brightness change fails, the slider snaps back to the monitor's real level.

---

## What was built

### Core (`macos-app/Sources/MonitorControlCore/`)
- `InputLabels.swift`: `InputEntry.label`, the readable names for known dellctl input names, falling back to dellctl's name.
- `Actions.swift`:
  - `failureMessage(of:)` returns nil on success; otherwise dellctl's JSON error message, raw stderr, or the timeout / can't-run wording from `Problem`.
  - `parseBrightness(_:)` reads `brightness --json`.
  - `errorMessage(_:)` is shared with `PanelState`.
  - `BrightnessWriter` coalesces slider values: at most one write in flight, values overtaken during a write are skipped, the last value requested is always written, and nothing is rewritten if it already matches the last write.
- `Models.swift`:
  - `MonitorStatus.switched(to:)` moves the checkmark after a successful switch.
  - `ErrorPayload.decode(_:)`.
- `Dellctl.swift`: fixed a Swift 6 warning by keeping stderr on the `Job` instead of in a captured `var`.
- Tests: new `ActionTests.swift` covering labels, failure messages, brightness parsing, the switched status, and three `BrightnessWriter` cases. 14 Swift tests total, all passing; the 54 Rust tests are unchanged and passing.

### App (`macos-app/Sources/MonitorControl/`)
- `StatusModel.swift`:
  - Every dellctl call (refresh, `input set <code> --json`, `brightness <n> --json`, and the snap-back read `brightness --json`) goes through `serial(_:)`, which chains Tasks so only one dellctl talks to the monitor at a time.
  - New `brightness`, `switchingTo`, `actionError`, and `hoveredInput` properties, plus `switchInput(to:)` and `setBrightness(_:)`.
  - `panelWindow` is captured on open and closed after a successful switch.
- `PanelView.swift`:
  - Input rows are plain full-width buttons with a hover highlight, a checkmark, and a per-row spinner.
  - The slider is bound to `model.brightness` through `Binding(get:set:)`.
  - Inline red `actionError` line.
  - `PanelOpenObserver` now passes the window.

## Decisions not pre-specified in the PRD

- **When the panel closes:** it closes once the switch succeeds (~1.2s, with the row spinner), not on the click itself, so a failure can still be shown in the panel as Feature 3 requires.
- **How switching targets an input:** by its raw code (`input set 0x0f`), not its name, so the app never relies on name aliases.
- **Hover state without `@State`:** the hover highlight lives on the model (`hoveredInput`), because SwiftUI's `@State` macro doesn't build with the Command Line Tools (see the M1 log).
- **Where errors appear:** an action error is cleared on the next action or refresh, and it never replaces the panel with a problem screen.
- **Snap-back fallback:** if the snap-back read also fails, the slider returns to the last level the monitor confirmed.

## Notes for milestone 3

- The footer currently holds only Quit. The Launch at login checkbox goes next to it. Any state behind it (e.g. the `SMAppService.mainApp.status`) should live on the model, still with no `@State`.
- The version can come from `Bundle.main` `CFBundleShortVersionString`, which `build.sh` already sets from Cargo.toml.
- Login items need the app to stay at a stable path. Building to `target/` and then copying it into `/Applications` is the expected flow for the README.

## Deviations from the PRD

- None.

## Verification status

- Automated: Swift and Rust tests pass. `build.sh` produces the app, and the new build launches.
- Manual: the user confirmed on the U3425WE that input switching and the live brightness slider work.
