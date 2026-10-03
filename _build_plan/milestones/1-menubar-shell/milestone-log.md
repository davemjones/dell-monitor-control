## What's new in the app

- **Monitor Control.app** now exists. One command (`./macos-app/build.sh`) builds it, with `dellctl` bundled inside.
- A monitor icon sits in the menu bar. The app has no Dock icon and no window.
- Clicking the icon opens a panel showing the monitor's model, its inputs with a checkmark on the active one, and its brightness. The values are read fresh every time the panel opens.
- While it reads, the panel shows a spinner and greyed-out placeholders instead of old values.
- When something's wrong, the panel says so plainly and offers a **Retry** button. That covers a missing `dellctl`, no monitor, several monitors, an unreachable monitor (DDC/CI off), a timeout, or any other error.
- **Quit Monitor Control** is at the bottom of the panel.
- For the CLI: `dellctl status --json` now also lists the supported inputs, and `--json` errors come back as JSON with a `kind`.

---

## What was built

### CLI (Rust)
- `src/failure.rs` (new):
  - `Failure { kind, message }` error type, with `FailureKind` = `no_monitor | multiple_monitors | unsupported | failed`.
  - `kind_of()` looks through anyhow context for the kind.
  - `json()` renders `{"error":{"kind","message"}}`.
- `src/main.rs`: with `--json`, errors print as that JSON on stderr (exit 1). Text errors are unchanged.
- `src/select.rs`: tags the no-monitor error as `no_monitor` and the "several monitors… choose one" error as `multiple_monitors`.
- `src/commands.rs`:
  - New `read_inputs()` helper, shared by `input list` and `status`. It tags "doesn't report its inputs" as `unsupported`.
  - `mute_unsupported` is now `unsupported`.
  - `read_status(m, with_inputs)` reads inputs only for `--json`.
- `src/output.rs`: `Status.inputs: Option<Vec<InputEntry>>`, which is `null` when capabilities are unreadable or don't list inputs.
- Tests:
  - The status JSON test now includes `inputs`.
  - New `status_json_inputs_are_null_without_caps`.
  - New `errors_carry_a_kind`.
  - Total: 54 tests, all passing.
- `README.md`: the JSON section documents `inputs` and the error shape.

### App (Swift, `macos-app/`)
- `Package.swift`: swift-tools 6.0, macOS 14, three targets (`MonitorControlCore`, `MonitorControl`, `MonitorControlCoreTests`).
- `Sources/MonitorControlCore/`:
  - `Models.swift`: Codable `MonitorStatus`, `InputEntry`, `InputRef`, and `ErrorPayload`.
  - `Dellctl.swift`:
    - `Dellctl.bundled()` points at `Contents/Helpers/dellctl`.
    - `run(args, timeout: 10)` runs it off the main thread, drains both pipes, and terminates it on timeout.
    - Returns `RunResult` (`finished` / `launchFailed` / `timedOut`).
  - `PanelState.swift`:
    - `PanelState` is `loading | ready(MonitorStatus) | problem(Problem)`, built by `PanelState(statusResult:)`.
    - `Problem` holds all user-facing problem wording (title plus optional detail).
- `Sources/MonitorControl/`:
  - `MonitorControlApp.swift`: `MenuBarExtra(systemImage: "display")` with `.menuBarExtraStyle(.window)`.
  - `StatusModel.swift`: `@MainActor @Observable`; `refresh()` allows only one dellctl call in flight at a time.
  - `PanelView.swift`:
    - The panel layout: header, Input section, Brightness section, `ProblemView`, and the Quit footer.
    - Loading placeholders use `.redacted(reason: .placeholder)`.
    - `PanelOpenObserver` triggers a refresh whenever the panel window becomes key.
- `Tests/MonitorControlCoreTests/PanelStateTests.swift`: 7 Swift Testing tests covering decoding, every problem mapping, and real process runs (echo, missing binary, timeout).
- `macos-app/build.sh`:
  1. `cargo build --release` and `swift build -c release`.
  2. Assembles `target/Monitor Control.app`: `Contents/MacOS/MonitorControl`, `Contents/Helpers/dellctl`, and a generated `Info.plist` (version from Cargo.toml, `LSUIElement`, bundle id `com.davemjones.monitor-control`, min macOS 14).
  3. Ad-hoc codesigns the bundle.
- `.gitignore`: `/macos-app/.build/`.

## Decisions not pre-specified in the PRD

- **The CLI extension shape** was chosen with the user: `inputs` inside `status --json`, plus JSON errors with a `kind`. Each panel open is one `dellctl status --json` call, about 1.3–1.4s on the U3425WE because of the capabilities read.
- **Detecting several monitors:** a JSON array from `status` (it lists every monitor when several are connected) also maps to "Multiple monitors found — not supported yet", even if only one is a Dell.
- **"Can't reach <name>"** plus the DDC/CI hint is shown when the monitor is found but input, inputs, and brightness are all unreadable. This is how DDC/CI being off shows up on macOS.
- **The `failed` problem** shows the title "Something went wrong" with dellctl's message underneath.
- **Placement and signing:** the bundled CLI lives in `Contents/Helpers/`. The output goes to `target/` (already gitignored). Ad-hoc signing is needed for Apple Silicon to run the app; it is not distribution signing.
- **Build workaround:** SwiftUI's `@State` (and other SwiftUI macros) can't be used, because this SDK makes `@State` a macro whose plugin ships only with full Xcode. The App holds the model in a plain `let`. Observation's `@Observable` macro does work with the Command Line Tools.

## Notes for milestone 2

- `StatusModel.refresh()` is the single entry point and guards against overlapping dellctl calls. Input switching and brightness writes should go through the same `Dellctl.run`, keeping DDC access serialized and never running two dellctl processes at once.
- Useful JSON shapes:
  - `input set <name> --json` → `{"before","after","changed"}`.
  - `brightness <n> --json` → `{"brightness":{"before","after"}}`.
  - Errors → `{"error":{"kind","message"}}` on stderr.
- Input rows and the slider are display-only (a `.constant` binding on a disabled `Slider`). Rows show dellctl's raw names; readable labels are M2 scope.
- `PanelView` uses no `@State` (see above). For local view state such as the slider's drag value, keep it on the `@Observable` model, or verify that an alternative compiles with the Command Line Tools first.
- `swift build` prints harmless `ld: warning: search path … not found` lines under the Command Line Tools.

## Deviations from the PRD

- None in scope. The CLI's text output is unchanged; `inputs` is read only under `--json`, so plain `dellctl status` stays fast.
