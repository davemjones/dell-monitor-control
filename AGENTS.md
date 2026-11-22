# Agent guide: monitorctl and Monitor Control

Read this before changing anything in this repository, then read `README.md` (the user-facing source of truth for behavior and output). Platform-specific bring-up guides live in `LINUX_PROMPT.md` and `WINDOWS_PROMPT.md`.

## What this project is

Two products in one repo:

1. **`monitorctl`** (Rust, repo root): a cross-platform CLI that controls a monitor over **DDC/CI**. It lists monitors, reads and switches inputs, and reads and sets brightness, volume, and speaker mute. Every command accepts `--json`. No daemon, no config file: each run reads the monitor live, does one thing, and exits.
2. **Monitor Control** (Swift/SwiftUI, `macos-app/`): a macOS menu bar app. It is a **front end only**. It runs a bundled copy of `monitorctl` and reads its JSON. It never talks DDC/CI itself.

Targets: macOS (Apple Silicon), Windows 11 x64, Linux x64 (Arch is the reference distro).

**Reference hardware:** Dell U3425WE. Inputs `dp` (0x0f), `hdmi` (0x11), `thunderbolt` (0x19). It does **not** support speaker mute over DDC/CI (reads as `null` / `n/a`), and it keeps a separate brightness per input.

**Verification status:** macOS is verified end to end on real hardware. Windows and Linux build and pass tests against a simulated monitor but have not been run on real hardware.

## Repository map

| Path | Purpose |
|---|---|
| `src/main.rs` | Entry point. Prints output; errors go to stderr as `error: …`, or as JSON under `--json`. Exit 1 on failure (clap uses 2 for bad arguments) |
| `src/cli.rs` | clap definitions (`Cli`, `Command`, `InputAction`, `Level`, `SpeakerState`) |
| `src/commands.rs` | All command logic, written against the `Backend`/`Monitor` traits. Platform-neutral. Most tests live here |
| `src/output.rs` | Output structs (`Status`, `InputEntry`, `ListEntry`, …) and text/JSON rendering |
| `src/select.rs` | Monitor selection (`--monitor N\|MODEL`, auto-pick when only one is connected) and the shared `DDC_HINT` text |
| `src/failure.rs` | `Failure { kind, message }`. Kinds: `no_monitor`, `multiple_monitors`, `unsupported`, `failed` |
| `src/vcp.rs` | VCP codes (0x10 brightness, 0x60 input, 0x62 volume, 0x8D mute), input names and aliases, percent scaling |
| `src/backend/mod.rs` | `Backend` / `Monitor` traits, `Caps` parsing (via `mccs-caps`) |
| `src/backend/ddc.rs` | The real backend on `ddc-hi` 0.4. Retries each DDC transaction with a growing pause. Platform hooks are `#[cfg]`-gated here |
| `src/backend/fake.rs` | `FakeMonitor` / `FakeBackend` for tests, including the real U3425WE capabilities string (`U3425WE_CAPS`) |
| `src/backend/macos_serial.rs` | macOS serial-number lookup (only used by `list`) |
| `src/backend/windows_edid.rs` | Windows: reads the registry EDID for real model names and serials. `parse_device_id` is pure and tested on every OS |
| `src/backend/linux_i2c.rs` | Linux: when no monitors are found, probes `/dev/i2c-*` to explain missing `i2c-dev` or permissions. `diagnose` is pure and tested on every OS |
| `macos-app/Sources/MonitorControlCore/` | Testable app logic: `Monitorctl.swift` (process runner with timeout), `Models.swift` (JSON shapes), `PanelState.swift` (status → panel state / problems), `Actions.swift` (`BrightnessWriter`, error messages), `InputLabels.swift` |
| `macos-app/Sources/MonitorControl/` | The app shell: `MonitorControlApp.swift`, `StatusModel.swift` (state + serialized `monitorctl` runs), `PanelView.swift` (SwiftUI panel), `LoginItem.swift` |
| `macos-app/Tests/MonitorControlCoreTests/` | Swift Testing tests for the core module |
| `macos-app/Icon/make-icon.swift` | Draws the app icon (not an SF Symbol: Apple's license forbids those in app icons) |
| `macos-app/build.sh` | Builds `target/Monitor Control.app` with `monitorctl` at `Contents/Helpers/monitorctl`, ad-hoc signed |
| `.cargo/config.toml` | Windows-only static CRT link flag. Keep it |
| `.github/workflows-disabled/` | CI and release workflows, **parked**. Do not move them to `.github/workflows/` unless asked |

## Build and test

Rust (any platform; Linux also needs `libudev` headers and `pkg-config`):

```sh
cargo build --release                       # target/release/monitorctl
cargo test                                  # simulated monitor, no hardware needed
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

macOS app (needs Rust plus Apple Command Line Tools; full Xcode is not required):

```sh
swift test --package-path macos-app         # core-module tests
./macos-app/build.sh                        # → target/Monitor Control.app
```

All of the above must pass before committing. A `nom v3.2.1` "future incompatibility" warning comes from a dependency and is expected. `Package.swift` passes the Command Line Tools' Swift Testing plugin path explicitly so `swift test` works without Xcode; don't remove that.

The app version comes from `Cargo.toml`'s `version`, which `build.sh` writes into `Info.plist`. The release workflow requires the git tag to match it.

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
  {"monitor":1,"name":"DELL U3425WE","input":{"name":"thunderbolt","code":"0x19"},
   "inputs":[{"name":"dp","code":"0x0f","active":false}, …],
   "brightness":78,"volume":88,"speakers":null}
  ```
  Unreadable values are `null`. `inputs` is `null` if the monitor doesn't report its inputs.
- `list` and `input list` always return arrays.
- `input set` returns `{"before","after","changed"}`.
- Plain reads return a single key: `brightness --json` → `{"brightness":78}` (the app's slider snap-back parses this), likewise `volume` and `speakers`.
- `brightness <n>` / `volume <n>` return `{"brightness":{"before","after"}}` / `{"volume":{…}}`; volume changes also include `"speakers"`.
- `speaker` changes return `{"speakers":{"before","after"}}`.
- Errors under `--json` go to **stderr** as `{"error":{"kind":…,"message":…}}`, exit code 1. To give an error a kind, raise `Failure::new(FailureKind::…, msg)`; anything else reports as `failed`.

The app's decoding lives in `macos-app/Sources/MonitorControlCore/Models.swift` and `PanelState.swift`.

## Menu bar app behavior (keep when changing it)

- Reads `status --json` at launch, then again in the background each time the panel opens. The panel shows the last readings immediately and updates in place. The loading state appears only at startup and during Retry (which keeps the problem visible with a spinner).
- **Only one `monitorctl` runs at a time** (`StatusModel.serial`), so DDC/CI commands never overlap. Each run has a 10s timeout.
- Switching input runs `input set <code> --json` and closes the panel on success.
- The brightness slider writes `brightness <n> --json` while dragging through `BrightnessWriter`: one write in flight, overtaken values skipped, last value always written. On failure the slider snaps back to `brightness --json`. A refresh that lands during a write keeps the slider's position.
- Problem states come from the error `kind`. A monitor that's found with every value `null` means DDC/CI is off or blocked (`Problem.unreachable`). An array from `status` means multiple monitors (not supported in the app yet).
- The panel window is resized downward from the menu bar when its content height changes; keep content pinned to the top.
- No volume controls in the app yet.

## Conventions

- **Tests for every behavior change.** Rust: use the `FakeBackend` style in `src/commands.rs` (`run_with(monitors, &["args"])`, `FakeMonitor::u3425we()`). Swift: add tests to `MonitorControlCoreTests` and keep logic in `MonitorControlCore` so it stays testable.
- **CLI messages:** short, lowercase, and specific, with a `hint:` line that says exactly what to do. Text output must match the examples in `README.md`.
- **App messages:** sentence case (the app capitalizes `monitorctl`'s messages for display).
- **Comments:** brief doc comments that explain *why*; match the density of surrounding code.
- **Update `README.md`** when behavior, output, setup steps, Known limitations, or Troubleshooting change. Update `LINUX_PROMPT.md` / `WINDOWS_PROMPT.md` when what they describe changes.
- **Commits:** only when asked. One logical change per commit. Short imperative subject line, with a body explaining why (see `git log`).
- **Never commit** build output (`target/`, `macos-app/.build/`), signing identities, certificates, or tokens. The app uses ad-hoc signing only.

## Known limitations to keep in mind

- Docks, adapters, and KVMs can block DDC/CI.
- After switching input away from this computer, some setups lose DDC/CI. macOS over Thunderbolt to the U3425WE can still switch back.
- Only input, brightness, volume, and mute are supported (no contrast, color presets, KVM, or PbP/PiP).
- Binaries and the app are unsigned/not notarized. No Intel Mac, Windows on ARM, or Linux on ARM support.
