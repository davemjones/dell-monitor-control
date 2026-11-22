# Working on monitorctl from Windows

You are working on this repository from a **Windows 11 (x64)** machine. This file has what you need to build, verify on real hardware, and improve the project on Windows. Read it fully, then read `README.md`, before changing anything.

## What this project is

`monitorctl` is a Rust command-line tool for controlling a monitor over **DDC/CI** (a control channel that runs over the video cable). It can:
- List and switch inputs.
- Read and set brightness.
- Read and set speaker volume.
- Mute and unmute the speakers.

Every read command takes `--json`. One codebase builds for macOS (Apple Silicon), Windows 11 (x64), and Linux (x64).

- **Test monitor:** Dell **U3425WE** (34" ultrawide). It has inputs `dp` (0x0f), `hdmi` (0x11), and `thunderbolt` (0x19). It does **not** support speaker mute over DDC/CI (`speakers` reads `null` / `n/a`), and it keeps a **separate brightness per input**.
- **Status per platform:**
  - **macOS:** fully verified on the real monitor.
  - **Windows:** builds and passes tests (the tests use a simulated monitor) but **has never been run against real monitor hardware.** Proving and refining that is the main job here.
- **Menu bar app:** `macos-app/` holds **Monitor Control**, a native macOS menu bar app (Swift/SwiftUI). It's macOS-only; ignore it on Windows unless you're building a Windows counterpart (see "Building a Windows tray app" below).

## Repository map

| Path | What it is |
|---|---|
| `src/main.rs` | Entry point. Prints output, or errors (`error: …` as text, or JSON with `--json`) |
| `src/cli.rs` | clap argument definitions (`Cli`, `Command`, `InputAction`, `Level`) |
| `src/commands.rs` | All command logic, written against the `Backend`/`Monitor` traits. Most tests live here |
| `src/output.rs` | Output structs (`Status`, `InputEntry`, …) and text/JSON rendering |
| `src/select.rs` | Choosing the monitor (auto-picks the monitor when only one is connected), plus the no-monitor hint text `DDC_HINT`. On Windows, the no-monitor message adds "update your graphics driver" |
| `src/failure.rs` | `Failure { kind, message }`. Kinds: `no_monitor`, `multiple_monitors`, `unsupported`, `failed` |
| `src/vcp.rs` | VCP codes (0x10 brightness, 0x60 input, 0x62 volume, 0x8D mute), input names and aliases, percent scaling |
| `src/backend/mod.rs` | The `Backend` / `Monitor` traits and capability parsing |
| `src/backend/ddc.rs` | The real backend, built on the `ddc-hi` crate (0.4). Retries each DDC transaction up to 4 times with a growing pause |
| `src/backend/windows_edid.rs` | **Windows-specific.** The Windows monitor API names every monitor "Generic PnP Monitor", so this reads the EDID copy in the registry (`HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\…\Device Parameters\EDID`) for the real model name, manufacturer, and serial. `parse_device_id` is pure and tested on every OS; the `win32` module uses `windows-sys` |
| `src/backend/fake.rs` | `FakeMonitor` / `FakeBackend` for tests. Includes the real U3425WE capabilities string |
| `src/backend/linux_i2c.rs`, `macos_serial.rs` | Other platforms |
| `.cargo/config.toml` | **Windows:** links the C runtime statically (`+crt-static`) so `monitorctl.exe` runs without the Visual C++ Redistributable. Keep it |
| `.github/workflows-disabled/` | CI (`ci.yml`) and release (`release.yml`) workflows, **parked** (not active). Windows CI uses `windows-latest`; the release packages `monitorctl-vX.Y.Z-windows-x64.zip` |
| `macos-app/` | macOS menu bar app; not built on Windows |

How it works on Windows:
1. `ddc-hi` uses its `ddc-winapi` feature (Windows' built-in monitor configuration API, Dxva2).
2. The `nvapi` feature is **deliberately off**: on NVIDIA GPUs it lists every monitor a second time. See the comment in `Cargo.toml`.
3. `DdcHiBackend::enumerate` (`src/backend/ddc.rs`) pairs each `ddc-hi` display with `windows_edid::lookup()` results in order, and **only when both report the same number of monitors**. Otherwise the names stay as Windows reports them. Pairing by order is the riskiest Windows logic, so verify it carefully with more than one monitor attached.

## Set up the machine

1. Install Rust with [rustup](https://rustup.rs) (stable, `x86_64-pc-windows-msvc`), then run `rustup component add clippy rustfmt`.
2. Install the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the **"Desktop development with C++"** workload, which provides the MSVC linker.
3. Install Git for Windows. In a fresh clone, run `git config core.autocrlf false` so line endings match the repo, or rely on `cargo fmt` to keep them consistent.
4. Make sure DDC/CI is on in the monitor's menu (**Others → DDC/CI → On**). Use a direct cable if possible, since docks, adapters, and KVMs can block DDC/CI. Keep the graphics driver up to date.

**Optional cross-check tool:** NirSoft's **ControlMyMonitor** reads and writes VCP codes through the same Windows API. Use it to tell whether a problem is in `monitorctl` or in the driver or connection.

## Build and test (PowerShell)

```powershell
cargo build                 # debug
cargo build --release       # → target\release\monitorctl.exe
cargo test                  # simulated monitor, no hardware needed
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

All of these must pass before anything is committed. These are the same checks the parked CI runs. (A `nom v3.2.1` "future incompatibility" note during builds comes from a dependency and is expected.)

Check the static runtime with `dumpbin /dependents target\release\monitorctl.exe` (from a Developer PowerShell). It should **not** list `VCRUNTIME140.dll`.

## Verify on the real monitor

Work through this with the U3425WE connected. Note what Windows actually reports for each step.

1. `.\target\release\monitorctl.exe list` and `list --verbose` should show `DELL U3425WE` (**not** "Generic PnP Monitor") with a serial. Verbose should report `input, brightness, volume` as supported and `speaker mute` as not supported. If the name falls back to "Generic PnP Monitor", debug `windows_edid::lookup` and the count-matching pairing in `ddc.rs`.
2. `monitorctl` / `monitorctl status`: the input this PC is connected on, plus brightness, volume, and `Speakers: n/a`.
3. `monitorctl status --json`: an object with `inputs` listing `dp`, `hdmi`, and `thunderbolt`, with the active one marked.
4. `monitorctl input` and `monitorctl input list`: the active input is marked `*`.
5. `monitorctl brightness`, `brightness 40`, `brightness +10`, `brightness -10`, and `brightness 150` (must be rejected). Nudges clamp at 0 and 100. In PowerShell, check that `brightness -10` isn't mistaken for a flag.
6. `monitorctl volume` and `volume 30`.
7. `monitorctl speaker`: it must say mute isn't supported and suggest `monitorctl volume 0`.
8. `monitorctl input set <other input>`: the monitor switches. Then check whether `monitorctl input set <this PC's input>` can switch back while another source is showing; DDC/CI may be lost after switching away on some setups. Record the result for the README's Known limitations section (macOS over Thunderbolt *can* switch back).
   - `monitorctl input name <input> Test` should write `%APPDATA%\monitorctl\names.json`; `input list` then shows the name, and `input name <input> --clear` removes it.
9. Multiple monitors: with a second monitor attached, `list` must show correct names for **both**, in the right order. `status` shows all of them, and a change without `--monitor` must refuse to guess. If the GPU is NVIDIA, confirm no monitor is listed twice.
10. Error paths:
    - Turn off DDC/CI in the monitor menu: you should get a "no DDC/CI-capable monitors found" or "couldn't read …" message with a hint.
    - Add `--json` to any failing command: stderr should be `{"error":{"kind":…,"message":…}}` with a non-zero exit code. Check `$LASTEXITCODE` in PowerShell.
    - Check that `→` and other non-ASCII characters in the output (e.g. `Brightness: 65 → 75`) display correctly in Windows Terminal and in legacy `cmd.exe`.
11. Timing: on macOS, `status` takes ~0.25s and `status --json`/`input list` take ~1.3s (reading capabilities is slow). Measure with `Measure-Command { .\target\release\monitorctl.exe status --json }`. Retries in `ddc.rs` may need tuning if Windows shows intermittent read failures.

## The JSON contract (don't break it)

`macos-app/` and any future tray app depend on these shapes, so keep them stable, or update every consumer at the same time.

- `status --json`: one object for a single monitor, or an array when several are connected and none was chosen.
  ```json
  {"monitor":1,"name":"DELL U3425WE","input":{"name":"thunderbolt","code":"0x19","label":null},
   "inputs":[{"name":"dp","code":"0x0f","label":null,"active":false}, …],
   "brightness":78,"volume":88,"speakers":null}
  ```
  `inputs` is read only under `--json` (the text output skips it for speed). It's `null` when the monitor doesn't report its inputs.
- Every input object carries `label`: the user's name for it from `names.json`, or `null`.
- `list` and `input list` always return arrays. `input set` returns `{"before","after","changed"}`. `input name` returns `{"input"}`, plus `"label":{"before","after"}` when it changes a name. `brightness <n>` returns `{"brightness":{"before","after"}}`.
- Errors under `--json` go to stderr as `{"error":{"kind":"no_monitor|multiple_monitors|unsupported|failed","message":…}}`, with a non-zero exit code. To give a new error a kind, raise it as `Failure::new(FailureKind::…, msg)` (see `src/failure.rs`).

## How to make changes

- **Keep platform code at the edge.** `src/commands.rs` must stay platform-neutral. Windows fixes belong in `src/backend/ddc.rs` (behind `#[cfg(windows)]`) or `src/backend/windows_edid.rs`.
- **Every behavior change gets a test** in the existing `FakeBackend` style (`run_with(monitors, &["args"])` in `src/commands.rs`). Keep Windows parsing logic in pure functions like `parse_device_id`, compiled under `cfg(any(windows, test))`, so it's tested on every OS. Only the raw Win32 calls should be Windows-only.
- **Don't break the other platforms.** Gate Windows-only code with `cfg`. Add any new `windows-sys` features to the existing `[target.'cfg(windows)'.dependencies]` entry in `Cargo.toml`.
- **Keep the user-facing style:**
  - Errors are short, lowercase, and specific, with a `hint:` line that says exactly what to do.
  - Text output matches the README examples.
- **Update `README.md`** whenever behavior, setup steps, Known limitations, or Troubleshooting entries change. Once you've verified Windows on hardware, change the Known limitations bullet that says Windows and Linux are untested.
- **Commits:**
  - Only commit when asked.
  - Use short imperative subject lines with a body explaining why (see `git log`).
  - Keep one logical change per commit.
- **Re-enabling CI:** the workflows are parked in `.github/workflows-disabled/`. Move them back to `.github/workflows/` only when asked.

## Building a Windows tray app (if requested)

The macOS app (`macos-app/`) sets the pattern for any Windows counterpart, which would be a notification-area (system tray) icon with a small flyout:
- It's a **front end only**. It runs the bundled `monitorctl.exe` and reads its JSON, and must never talk DDC/CI itself or duplicate input, brightness, or selection rules. If the app needs data the CLI doesn't give, extend the CLI and its JSON.
- Launch `monitorctl.exe` **without a console window** (`CREATE_NO_WINDOW`), or a terminal will flash on every call.
- Read `monitorctl status --json` at launch, then again in the background each time the flyout opens. Show the last readings straight away and update them in place; the loading spinner appears only at startup and on Retry.
- Switching inputs: `input set <code> --json`, then close the flyout on success.
- Brightness: a slider that writes `brightness <n> --json` while dragging. Only one write is in flight at a time, values overtaken during a write are skipped, and the last value is always written. If a write fails, the slider snaps back to the real value from `brightness --json`.
- Only one `monitorctl` runs at a time, so DDC/CI commands never overlap. Each run has a 10s timeout.
- Problem states map from the error `kind`, each with a Retry button. A monitor that's found but has every value `null` means DDC/CI is off or blocked.
- Plus a "Launch at login" option (a Run registry key or Startup folder shortcut), Quit, a version display, and an app icon. The macOS icon is drawn by `macos-app/Icon/make-icon.swift`, a white monitor on a blue tile; match that look.

Read `macos-app/Sources/MonitorControlCore/` (`Monitorctl.swift`, `PanelState.swift`, `Actions.swift`) for the exact behavior to mirror. Agree on the toolkit (e.g. a Rust tray crate, Tauri, WinUI, or WPF) with the user before building.
