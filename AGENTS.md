# Agent guide: monitorctl and Monitor Control

Read this before changing anything in this repository, then read `README.md` (the user-facing source of truth for behavior and output). Platform-specific bring-up guides live in `LINUX_PROMPT.md` and `WINDOWS_PROMPT.md`.

## What this project is

Two products in one repo:

1. **`monitorctl`** (Rust, repo root): a cross-platform CLI that controls a monitor over **DDC/CI**. It lists monitors, reads and switches inputs, and reads and sets brightness, volume, and speaker mute. Every command accepts `--json`. No daemon; the only file it keeps is the user's input names (`names.json`, see `src/names.rs`). Each run reads the monitor live, does one thing, and exits.
2. **Monitor Control** (Swift/SwiftUI, `macos-app/`): a macOS 27+ app with Control Center controls (brightness, input, settings), Shortcuts actions, a Settings window, and an optional menu bar panel. It is a **front end only**. It runs a bundled copy of `monitorctl` and reads its JSON. It never talks DDC/CI itself.

Targets: macOS (Apple Silicon), Windows 11 x64, Linux x64 (Arch is the reference distro).

**Reference hardware:** Dell U3425WE. Inputs `dp` (0x0f), `hdmi` (0x11), `thunderbolt` (0x19). It does **not** support speaker mute over DDC/CI (reads as `null` / `n/a`), and it keeps a separate brightness per input.

**Verification status:** macOS is verified end to end on real hardware. Windows and Linux build and pass tests against a simulated monitor but have not been run on real hardware.

## Repository map

| Path | Purpose |
|---|---|
| `src/main.rs` | Entry point. Prints output; errors go to stderr as `error: …`, or as JSON under `--json`. Exit 1 on failure (clap uses 2 for bad arguments) |
| `src/cli.rs` | clap definitions (`Cli`, `Command`, `InputAction`, `Level`, `SpeakerState`) |
| `src/commands.rs` | All command logic, written against the `Backend`/`Monitor` and `NameStore` traits. Platform-neutral. Most tests live here |
| `src/output.rs` | Output structs (`Status`, `InputEntry`, `ListEntry`, …) and text/JSON rendering |
| `src/select.rs` | Monitor selection (`--monitor N\|MODEL`, auto-pick when only one is connected) and the shared `DDC_HINT` text |
| `src/names.rs` | Custom input names: the `names.json` file (`FileStore`, path from `$MONITORCTL_NAMES_FILE` or the OS config dir), name validation, and `MemoryStore` for tests |
| `src/failure.rs` | `Failure { kind, message }`. Kinds: `no_monitor`, `multiple_monitors`, `unsupported`, `failed` |
| `src/vcp.rs` | VCP codes (0x10 brightness, 0x60 input, 0x62 volume, 0x8D mute), input names and aliases, percent scaling |
| `src/backend/mod.rs` | `Backend` / `Monitor` traits, `Caps` parsing (via `mccs-caps`) |
| `src/backend/ddc.rs` | The real backend on `ddc-hi` 0.4. Retries each DDC transaction with a growing pause. Platform hooks are `#[cfg]`-gated here |
| `src/backend/fake.rs` | `FakeMonitor` / `FakeBackend` for tests, including the real U3425WE capabilities string (`U3425WE_CAPS`) |
| `src/backend/macos_serial.rs` | macOS serial-number lookup (only used by `list`) |
| `src/backend/windows_edid.rs` | Windows: reads the registry EDID for real model names and serials. `parse_device_id` is pure and tested on every OS |
| `src/backend/linux_i2c.rs` | Linux: when no monitors are found, probes `/dev/i2c-*` to explain missing `i2c-dev` or permissions. `diagnose` is pure and tested on every OS |
| `macos-app/Sources/MonitorControlCore/` | Testable app logic: `Monitorctl.swift` (process runner with timeout), `Models.swift` (JSON shapes), `PanelState.swift` (status → panel state / problems), `Actions.swift` (`BrightnessWriter`, error messages), `InputLabels.swift` |
| `macos-app/Sources/MonitorControlCore/LaunchGate.swift` | Decides whether a launch opens Settings (not for login-item launches or control/Shortcuts actions) |
| `macos-app/Sources/MonitorControlCore/ControlIntents.swift` | The controls' App Intents (`allowedExecutionTargets = .main`), the `ControlActionHandler` protocol the app implements, and `ControlActions.run` (monitorctl through the app's queue) |
| `macos-app/Sources/MonitorControlCore/ShortcutIntents.swift` | Shortcuts actions *Switch Monitor Input* (`MonitorInputEntity`, listed from `input list --json`) and *Set Monitor Brightness* |
| `macos-app/Sources/MonitorControlCore/PanelPlacement.swift` | `panelOrigin`: where a control's panel opens (top right of the visible frame) |
| `macos-app/App/` | The app shell: `MonitorControlApp.swift` (app delegate: optional menu bar icon, launch handling, `ControlActionHandler`), `StatusModel.swift` (state + serialized `monitorctl` runs), `PanelView.swift` (menu bar panel and the shared input/brightness sections), `ControlPanel.swift` (the panels the controls open), `SettingsView.swift` (Settings window), `Shortcuts.swift` (App Shortcuts), `LoginItem.swift`, `Assets.xcassets` |
| `macos-app/Controls/` | The `ControlsExtension` widget extension (sandboxed): the three Control Center controls, which only name intents from the framework |
| `macos-app/MonitorControl.xcodeproj/` | Xcode project in the JSON `project.xcproj` format: the app, the `MonitorControlCore` framework (built from `Sources/MonitorControlCore`, embedded in the app, linked by the extension), and `ControlsExtension`. A copy phase bundles `../target/release/monitorctl` at `Contents/Helpers/` and signs it |
| `macos-app/Config/` | `MonitorControl.xcconfig` (shared), which includes the git-ignored `Signing.local.xcconfig` (Team ID; see the `.example`) and `Version.xcconfig` (written by `build.sh`) |
| `macos-app/Package.swift` | SwiftPM package for `MonitorControlCore` and its tests only, so `swift test` runs without building the app |
| `macos-app/Tests/MonitorControlCoreTests/` | Swift Testing tests for the core module |
| `macos-app/Icon/make-icon.swift` | Draws the app icon (not an SF Symbol: Apple's license forbids those in app icons) |
| `macos-app/build.sh` | Builds `monitorctl`, draws the icon into the asset catalog, and runs `xcodebuild`; leaves a signed `target/Monitor Control.app` |
| `.cargo/config.toml` | Windows-only static CRT link flag. Keep it |
| `scripts/release-macos.sh` | Builds a release's macOS assets (CLI archive, app zip, `.sha256` files) from the tagged source; `--draft NOTES.md` also creates the draft GitHub release. See [Releases](#releases) |
| `specs/` | Feature specs. A **separate, private git repository** (`davemjones/dell-monitor-control-specs`), ignored by this project. See [Features across both repositories](#features-across-both-repositories) |

## Build and test

Rust (any platform; Linux also needs `libudev` headers and `pkg-config`):

```sh
cargo build --release                       # target/release/monitorctl
cargo test                                  # simulated monitor, no hardware needed
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

macOS app (needs Rust plus Xcode 27.2+, and `macos-app/Config/Signing.local.xcconfig` with your Team ID):

```sh
swift test --package-path macos-app         # core-module tests
./macos-app/build.sh                        # → target/Monitor Control.app
```

All of the above must pass before committing. A `nom v3.2.1` "future incompatibility" warning comes from a dependency and is expected. `Package.swift` passes the Command Line Tools' Swift Testing plugin path explicitly so `swift test` also works with only the Command Line Tools selected; don't remove that.

The app version comes from `Cargo.toml`'s `version`, which `build.sh` writes into `Config/Version.xcconfig` for Xcode. Release tags (`v<version>`) must match it; `scripts/release-macos.sh` checks. Every new feature bumps it (see [Features across both repositories](#features-across-both-repositories)).

## Architecture rules

- **Platform code stays at the edge.** `src/commands.rs`, `output.rs`, `select.rs`, and `vcp.rs` must stay platform-neutral. Platform fixes go in `src/backend/` behind `#[cfg(target_os = …)]`.
- **Platform-specific dependencies** go in a `[target.'cfg(…)'.dependencies]` table in `Cargo.toml`. Keep `cargo test` compiling on every OS.
- **Don't enable `ddc-hi`'s `nvapi` feature.** On NVIDIA GPUs it lists every monitor twice (see the comment in `Cargo.toml`).
- **Keep pure logic pure.** Platform diagnostics and parsers (`linux_i2c::diagnose`, `windows_edid::parse_device_id`) are pure functions so they're tested on every OS. Follow that pattern.
- **Reading capabilities is slow (about 1s).** Text `status` skips it; only `status --json`, `input list`, and `list --verbose` read them. Don't add capability reads to fast paths.
- **The app never duplicates CLI logic.** If the app needs data the CLI doesn't provide, extend the CLI and its JSON, then consume it in the app.

## The JSON contract (don't break it)

The menu bar app (and any future tray app) depends on these shapes. Keep them stable, or update every consumer in the same change.

- `status --json`: one object for a single monitor; an array when several are shown.
  ```json
  {"monitor":1,"name":"DELL U3425WE","input":{"name":"thunderbolt","code":"0x19","label":"Work Mac"},
   "inputs":[{"name":"dp","code":"0x0f","label":null,"active":false}, …],
   "brightness":78,"volume":88,"speakers":null}
  ```
  Unreadable values are `null`. `inputs` is `null` if the monitor doesn't report its inputs. Every input object carries `label`, the user's name for it or `null`.
- `list` and `input list` always return arrays.
- `input set` returns `{"before","after","changed"}`.
- `input name` returns `{"input"}` when showing, and `{"input","label":{"before","after"}}` when setting or clearing.
- Plain reads return a single key: `brightness --json` → `{"brightness":78}` (the app's slider snap-back parses this), likewise `volume` and `speakers`.
- `brightness <n>` / `volume <n>` return `{"brightness":{"before","after"}}` / `{"volume":{…}}`; volume changes also include `"speakers"`.
- `speaker` changes return `{"speakers":{"before","after"}}`.
- Errors under `--json` go to **stderr** as `{"error":{"kind":…,"message":…}}`, exit code 1. To give an error a kind, raise `Failure::new(FailureKind::…, msg)`; anything else reports as `failed`.

The app's decoding lives in `macos-app/Sources/MonitorControlCore/Models.swift` and `PanelState.swift`.

## Mac app behavior (keep when changing it)

- An agent app (`LSUIElement`). The menu bar icon is optional and **off by default** (`showMenuBarIcon` in `UserDefaults`, toggled in Settings).
- **Control Center controls** (Monitor Brightness, Monitor Input, Monitor Control Settings) are buttons: Control Center has no sliders. Their intents run in the app (`.main`, needs macOS 27), which opens a borderless panel at the top right of the screen under the pointer (`ControlPanel`), closed by Esc or clicking away. The input panel switches inputs but never renames. With no monitor, panels disable their control and show only "No monitor detected"; other problems show the `Problem` with Retry.
- **Shortcuts actions** *Switch Monitor Input* and *Set Monitor Brightness* run in the app through `StatusModel.runQueued`, then refresh. Siri/Spotlight phrases are out of scope.
- Opening the app shows **Settings**: on launch after `LaunchGate.grace` unless it was started as a login item or to run a control or Shortcuts action, and on every reopen while running. Settings has the input names, Show menu bar icon, Launch at login, a Control Center guide, and Quit. Names save through `input name` like the panel's Rename…, with errors shown under the field.
- Reads `status --json` at launch, then again in the background each time the panel or Settings opens. The panel shows the last readings immediately and updates in place. The loading state appears only at startup and during Retry (which keeps the problem visible with a spinner).
- **Only one `monitorctl` runs at a time** (`StatusModel.serial`), so DDC/CI commands never overlap. Each run has a 10s timeout.
- Switching input runs `input set <code> --json` and closes the panel (menu bar or control) on success.
- Right-click an input → Rename… edits the row in place (Return or losing focus saves, Esc cancels); Reset Name clears it. Saving runs `input name <code> <name> --json` (or `--clear`), shows the new name immediately, and restores the old one on failure. The app never writes `names.json` itself.
- The brightness slider writes `brightness <n> --json` while dragging through `BrightnessWriter`: one write in flight, overtaken values skipped, last value always written. On failure the slider snaps back to `brightness --json`. A refresh that lands during a write keeps the slider's position.
- Problem states come from the error `kind`. A monitor that's found with every value `null` means DDC/CI is off or blocked (`Problem.unreachable`). An array from `status` means multiple monitors (not supported in the app yet).
- The panel window is resized downward from the menu bar when its content height changes; keep content pinned to the top.
- No volume controls in the app yet.

## Releases

- **Never use GitHub workflows.** Every build, including release builds, is made locally. There is no CI.
- Each release has `specs/releases/<version>/tasks.md` (see `specs/releases/README.md`): a checklist written so an agent on another machine can build its platform's assets (Linux x64 in an Ubuntu 22.04 container, Windows x64) and upload them with `gh`. Keep it after the release and record who built what, when, and the checksums.
- This Mac tags `v<version>` (matching `Cargo.toml`), runs `scripts/release-macos.sh --draft NOTES.md` to build the macOS assets and create a **draft** release, and the other machines add theirs. Publish only when every asset and its `.sha256` file is attached.
- The app is signed with the developer's team (Personal Team today) but not notarized, so downloaded copies need Privacy & Security › Open Anyway. Developer ID signing and notarization come with the paid Developer Program.

## Features across both repositories

Feature work spans two repositories: this one (code, public) and `specs/` (specs, private, its own `.git`, listed in this project's `.gitignore`). It is deliberately **not** a git submodule, because a private submodule would break clones of the public project. Keep the two in step by convention:

1. **Same branch name in both.** Start every feature on a matching branch, e.g. `feature/volume-slider`, created from an up-to-date `main` in each:
   ```sh
   git switch main && git pull && git switch -c feature/<name>
   git -C specs switch main && git -C specs pull && git -C specs switch -c feature/<name>
   ```
   Before working, check that both repositories are on the same branch (`git branch --show-current; git -C specs branch --show-current`). If they differ, stop and ask.
2. **Spec first.** Write or update the spec in `specs/` before changing code, and keep it accurate as the implementation changes. The spec PR merges before, or together with, the code PR, never after.
3. **Bump the version.** Every new feature raises the minor version in `Cargo.toml` (`0.1.0` → `0.2.0`, `1.2.3` → `1.3.0`) at the start of the work, so builds from the branch are distinguishable from `main`. Run `cargo build` so `Cargo.lock` picks it up, and include both files in the feature's code commit. Check that no other merged feature has already taken that number; if `main` moved on, bump again from its version.
4. **Check and commit each repository separately.** `git status` here does not show changes inside `specs/`. Always check both (`git status && git -C specs status`), and commit spec changes in `specs/` and code changes here, each with its own message. The commit rules in [Conventions](#conventions) apply to both.
5. **Link the pull requests.** In the code PR, add `Spec: davemjones/dell-monitor-control-specs#<n>`; in the spec PR, link the code PR. Don't paste private spec content into the public code PR beyond a short summary.
6. **Record the spec version on merge.** Put the spec commit the code was built against in the code PR description or merge commit, e.g. `Spec: dell-monitor-control-specs@<short-sha>`.
7. **Finish together.** After both PRs merge, switch both repositories back to `main`, pull, and delete the feature branch in each (locally and on GitHub).

Git worktrees of this project don't include `specs/` (it's ignored). If you work in a worktree, create a matching worktree of the specs repository at `<worktree>/specs` on the same branch.

## Conventions

- **Tests for every behavior change.** Rust: use the `FakeBackend` style in `src/commands.rs` (`run_with(monitors, &["args"])`, `FakeMonitor::u3425we()`). Swift: add tests to `MonitorControlCoreTests` and keep logic in `MonitorControlCore` so it stays testable.
- **CLI messages:** short, lowercase, and specific, with a `hint:` line that says exactly what to do. Text output must match the examples in `README.md`.
- **App messages:** sentence case (the app capitalizes `monitorctl`'s messages for display).
- **Comments:** brief doc comments that explain *why*; match the density of surrounding code.
- **Update `README.md`** when behavior, output, setup steps, Known limitations, or Troubleshooting change. Update `LINUX_PROMPT.md` / `WINDOWS_PROMPT.md` when what they describe changes.
- **Commits:** only when asked. One logical change per commit. Short imperative subject line, with a body explaining why (see `git log`).
- **Never commit** build output (`target/`, `macos-app/.build/`), signing identities, certificates, or tokens. Signing uses the developer's own team via the git-ignored `Signing.local.xcconfig`; never commit a Team ID.

## Known limitations to keep in mind

- Docks, adapters, and KVMs can block DDC/CI.
- After switching input away from this computer, some setups lose DDC/CI. macOS over Thunderbolt to the U3425WE can still switch back.
- Input names are keyed by monitor model, so identical monitors share them.
- Only input, brightness, volume, and mute are supported (no contrast, color presets, KVM, or PbP/PiP).
- The CLI binaries are unsigned; the app is signed with the developer's team but not notarized. The app needs macOS 27+ and supports one monitor. No Intel Mac, Windows on ARM, or Linux on ARM support.
