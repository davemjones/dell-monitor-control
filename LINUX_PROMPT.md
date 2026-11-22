# Working on monitorctl from Linux

You are working on this repository from a **Linux** machine (the target distribution is **Arch Linux, x64**). This file has what you need to build, verify on real hardware, and improve the project on Linux. Read it fully, then read `README.md`, before changing anything.

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
  - **Linux:** builds and passes tests (the tests use a simulated monitor) but **has never been run against real monitor hardware.** Proving and refining that is the main job here.
- **Menu bar app:** `macos-app/` holds **Monitor Control**, a native macOS menu bar app (Swift/SwiftUI). It's macOS-only; ignore it on Linux unless you're building a Linux counterpart (see "Building a Linux tray/desktop app" below).

## Repository map

| Path | What it is |
|---|---|
| `src/main.rs` | Entry point. Prints output, or errors (`error: …` as text, or JSON with `--json`) |
| `packaging/aur/` | The AUR package `monitorctl`: `PKGBUILD` (builds from the tag's source tarball, runs `cargo test`, `arch=('x86_64' 'aarch64')`), `monitorctl.install`, and the `i2c-dev` modules-load and `uaccess` udev files it installs. Keep it working when Linux build steps or dependencies change |
| `src/cli.rs` | clap argument definitions (`Cli`, `Command`, `InputAction`, `Level`) |
| `src/commands.rs` | All command logic, written against the `Backend`/`Monitor` traits. Most tests live here |
| `src/output.rs` | Output structs (`Status`, `InputEntry`, …) and text/JSON rendering |
| `src/select.rs` | Choosing the monitor (auto-picks the monitor when only one is connected), plus the no-monitor hint text `DDC_HINT` |
| `src/failure.rs` | `Failure { kind, message }`. Kinds: `no_monitor`, `multiple_monitors`, `unsupported`, `failed` |
| `src/vcp.rs` | VCP codes (0x10 brightness, 0x60 input, 0x62 volume, 0x8D mute), input names and aliases, percent scaling |
| `src/backend/mod.rs` | The `Backend` / `Monitor` traits and capability parsing |
| `src/backend/ddc.rs` | The real backend, built on the `ddc-hi` crate (0.4). Retries each DDC transaction up to 4 times with a growing pause |
| `src/backend/linux_i2c.rs` | **Linux-specific.** When no monitors are found, it probes `/dev/i2c-*` to say whether `i2c-dev` isn't loaded or permissions are missing |
| `src/backend/fake.rs` | `FakeMonitor` / `FakeBackend` for tests. Includes the real U3425WE capabilities string |
| `src/backend/windows_edid.rs`, `macos_serial.rs` | Other platforms (Windows compiles only its pure parsing under `cfg(test)`) |
| `.cargo/config.toml` | Windows-only static CRT flag; irrelevant on Linux |
| `macos-app/` | macOS menu bar app; not built on Linux |

How it works on Linux:
1. `ddc-hi` uses its `ddc-i2c` feature, which finds monitors through udev and talks to them through `/dev/i2c-*`.
2. `DdcHiBackend::enumerate` (`src/backend/ddc.rs`) lists the displays. If that finds none, it calls `linux_i2c::diagnose(linux_i2c::probe())` to explain why.
3. There's no Linux-specific name or serial lookup: names come from the EDID that `ddc-hi` reads.

## Set up the machine (Arch)

```sh
sudo pacman -S --needed rustup base-devel pkgconf git
rustup default stable
rustup component add clippy rustfmt
```

`libudev` comes with `systemd`, which every Arch install has. On Debian/Ubuntu use `sudo apt install libudev-dev pkg-config` instead.

Give your user access to the monitor's I²C devices. Without this step, `monitorctl` explains what's missing and prints these same commands:

```sh
sudo modprobe i2c-dev
echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo groupadd --system i2c
sudo usermod -aG i2c $USER
echo 'KERNEL=="i2c-[0-9]*", GROUP="i2c", MODE="0660"' | sudo tee /etc/udev/rules.d/45-i2c.rules
# log out and back in (or reboot) for the group to apply
```

Make sure DDC/CI is on in the monitor's menu (**Others → DDC/CI → On**). Use a direct cable if possible, since docks, adapters, and KVMs can block DDC/CI.

**Optional cross-check tool:** `sudo pacman -S ddcutil`. `ddcutil detect` and `ddcutil getvcp 10` give an independent answer when deciding whether a problem is in `monitorctl` or in the system or driver. Known issue: some GPU drivers (notably NVIDIA's proprietary driver) don't expose DDC over I²C by default. Check `ddcutil`'s documentation for the needed driver options before assuming a `monitorctl` bug.

## Build and test

```sh
cargo build                 # debug
cargo build --release       # → target/release/monitorctl
cargo test                  # simulated monitor, no hardware needed
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

All of these must pass before anything is committed. (A `nom v3.2.1` "future incompatibility" note during builds comes from a dependency and is expected.)

## Verify on the real monitor

Work through this with the U3425WE connected. Note what Linux actually reports for each step.

1. `./target/release/monitorctl list` and `list --verbose` should show `DELL U3425WE` with a serial. Verbose should report `input, brightness, volume` as supported and `speaker mute` as not supported. If the name or serial is missing or wrong, fix the Linux path (it comes from `ddc-hi`'s EDID read).
2. `monitorctl` / `monitorctl status`: the input this PC is connected on, plus brightness, volume, and `Speakers: n/a`.
3. `monitorctl status --json`: an object with `inputs` listing `dp`, `hdmi`, and `thunderbolt`, with the active one marked.
4. `monitorctl input` and `monitorctl input list`: the active input is marked `*`.
5. `monitorctl brightness`, `brightness 40`, `brightness +10`, `brightness -10`, and `brightness 150` (must be rejected). Nudges clamp at 0 and 100.
6. `monitorctl volume` and `volume 30`.
7. `monitorctl speaker`: it must say mute isn't supported and suggest `monitorctl volume 0`.
8. `monitorctl input set <other input>`: the monitor switches. Then check whether `monitorctl input set <this PC's input>` can switch back while another source is showing; DDC/CI may be lost after switching away on some setups. Record the result for the README's Known limitations section (macOS over Thunderbolt *can* switch back).
   - `monitorctl input name <input> Test` should write `~/.config/monitorctl/names.json` (or under `$XDG_CONFIG_HOME`); `input list` then shows the name, and `input name <input> --clear` removes it.
9. Error paths:
   - Unload the module (`sudo modprobe -r i2c-dev`): you should get the "i2c-dev kernel module probably isn't loaded" message.
   - Run as a user without access: you should get the "no permission to open /dev/i2c-N" message.
   - Turn off DDC/CI in the monitor menu: you should get "no DDC/CI-capable monitors found" or "couldn't read …" with a hint.
   - Add `--json` to any failing command: stderr should be `{"error":{"kind":…,"message":…}}` with exit code 1.
10. Timing: on macOS, `status` takes ~0.25s and `status --json`/`input list` take ~1.3s (reading capabilities is slow). Note the Linux numbers. Retries in `ddc.rs` may need tuning if Linux shows intermittent read failures.

Laptop note: the built-in panel isn't DDC-capable, so it normally doesn't show up. If it does, `select::one` will ask for `--monitor`; check that the message lists both.

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
- Errors under `--json` go to stderr as `{"error":{"kind":"no_monitor|multiple_monitors|unsupported|failed","message":…}}`, with exit code 1. To give a new error a kind, raise it as `Failure::new(FailureKind::…, msg)` (see `src/failure.rs`).

## How to make changes

- **Release builds** for Linux x64 and ARM64 happen in `ubuntu:22.04` containers (glibc ≤ 2.35) and the AUR package is published from an Arch machine; follow `specs/releases/<version>/tasks.md`.
- **Keep platform code at the edge.** `src/commands.rs` must stay platform-neutral. Linux fixes belong in `src/backend/ddc.rs` (behind `#[cfg(target_os = "linux")]`) or `src/backend/linux_i2c.rs`.
- **Every behavior change gets a test** in the existing `FakeBackend` style (`run_with(monitors, &["args"])` in `src/commands.rs`). Linux diagnostics are pure functions (`diagnose`) for exactly this reason; keep them that way so tests run on any OS.
- **Don't break the other platforms.** Gate Linux-only code with `cfg`, and make sure `cargo test` still compiles everywhere. In particular, don't add Linux-only dependencies outside a `[target.'cfg(target_os = "linux")'.dependencies]` table in `Cargo.toml`.
- **Keep the user-facing style:**
  - Errors are short, lowercase, and specific, with a `hint:` line that says exactly what to do.
  - Text output matches the README examples.
- **Update `README.md`** whenever behavior, setup steps, Known limitations, or Troubleshooting entries change. The Arch setup section is the source of truth for Linux setup. Once you've verified Linux on hardware, change the Known limitations bullet that says Windows and Linux are untested.
- **Commits:**
  - Only commit when asked.
  - Use short imperative subject lines with a body explaining why (see `git log`).
  - Keep one logical change per commit.
- **No GitHub workflows, ever.** All builds are local. To build this platform's release assets, follow `specs/releases/<version>/tasks.md` in the specs repository.

## Building a Linux tray/desktop app (if requested)

The macOS app (`macos-app/`) sets the pattern for any Linux counterpart:
- It's a **front end only**. It runs the bundled `monitorctl` and reads its JSON, and must never talk DDC/CI itself or duplicate input, brightness, or selection rules. If the app needs data the CLI doesn't give, extend the CLI and its JSON.
- Read `monitorctl status --json` at launch, then again in the background each time the panel opens. Show the last readings straight away and update them in place; the loading spinner appears only at startup and on Retry.
- Switching inputs: `input set <code> --json`, then close the panel on success.
- Brightness: a slider that writes `brightness <n> --json` while dragging. Only one write is in flight at a time, values overtaken during a write are skipped, and the last value is always written. If a write fails, the slider snaps back to the real value from `brightness --json`.
- Only one `monitorctl` runs at a time, so DDC/CI commands never overlap. Each run has a 10s timeout.
- Problem states map from the error `kind`, each with a Retry button. A monitor that's found but has every value `null` means DDC/CI is off or blocked.
- Plus a Launch at login option, Quit, and a version display.

Read `macos-app/Sources/MonitorControlCore/` (`Monitorctl.swift`, `PanelState.swift`, `Actions.swift`) for the exact behavior to mirror. Agree on the toolkit (e.g. GTK, Qt, or a Rust tray crate) and on desktop-environment support (KDE, GNOME and its tray-extension caveat, etc.) with the user before building.
