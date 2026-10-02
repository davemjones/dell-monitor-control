# dellctl

> **About these build-plan files:** Everything in `_build_plan/` (this PRD and the per-milestone folders) is a **temporary documentation and guidance artifact** for the initial build-out of this codebase. These files are not functional — no code, configuration, runtime logic, tests, or deployment process should import, read, reference, or depend on anything in `_build_plan/`. Once the initial milestones are built and shipped, the entire `_build_plan/` folder is expected to be deleted from the codebase. Do not treat it as long-living documentation.

## What we're building

`dellctl` is a small, cross-platform command-line tool, written in Rust, for controlling a Dell monitor from the terminal: list and switch video inputs, set brightness, mute or unmute the monitor's built-in speakers, and set their volume. The same tool builds and runs on macOS, Windows 11, and Arch Linux.

Monitors accept commands over the video cable through a standard called **DDC/CI** — effectively a remote-control channel that travels over HDMI, DisplayPort, or USB-C/Thunderbolt. Each operating system exposes that channel differently, so `dellctl` has one shared command-line experience on top of a small per-OS adapter. The primary target and test device is the **Dell U3425WE** (34" curved ultrawide with built-in speakers); other Dell models should work where they expose the same controls, but are not promised.

The tool is built in Rust (stable) with Cargo and ships as a single standalone binary per platform. The build is split into four milestones: prove everything read-only on macOS, add the controls on macOS, port to Windows and Linux, then add CI, release binaries, and documentation.

---

### What the app does

- Finds the DDC/CI-capable monitors connected to the computer and targets the single Dell automatically, or a specific one chosen by number or model.
- Lists the inputs the monitor supports and marks which one is active.
- Switches inputs by friendly name (`hdmi`, `dp`, `thunderbolt` / `usb-c`).
- Reads, sets, and nudges brightness.
- Mutes, unmutes, and toggles the monitor's built-in speakers.
- Reads, sets, and nudges speaker volume.
- Shows a one-screen status summary of everything at once.
- Offers `--json` output on every read command so it can be scripted.
- Works identically on macOS (Apple Silicon), Windows 11, and Arch Linux.

---

### Already provided by the existing codebase

- Nothing — this is a fresh, empty project. Milestone 1 creates the Cargo project from scratch.

---

### Out of scope

- **GUI, menu-bar, or tray app** — this is a CLI; a GUI would multiply the work.
- **Other monitor settings** (contrast, color presets, picture modes, KVM, PbP/PiP) — easy to add later on the same foundation; v1 stays focused.
- **Official support for non-Dell monitors** — may work since DDC/CI is a standard, but only Dell is tested and promised.
- **Background daemon, schedules, auto-brightness** — a different product shape.
- **Global hotkeys** — users can bind the CLI commands with their OS's own tools.
- **Saved profiles / presets** (e.g. "work mode") — good v2 candidate.
- **Config file / remembered default monitor** — not needed with sensible auto-detection.
- **Firmware updates or factory reset** — risky and unrelated.
- **Package-manager distribution** (Homebrew, AUR, winget) and installers — v1 ships release binaries and build instructions.
- **Code-signing / notarization** — macOS may show an "unidentified developer" warning on first run.
- **Intel Macs, Windows on ARM, Linux on ARM** — v1 targets Apple Silicon macOS, x64 Windows 11, x64 Arch Linux.
- **Shell tab-completion scripts** — nice-to-have, not needed for v1.

---

### Platform requirements

- **macOS (Apple Silicon, M1 or later):** no setup required.
- **Windows 11 (x64):** no setup required; uses the built-in Windows monitor-control interface.
- **Arch Linux (x64):** one-time setup — load the `i2c-dev` kernel module and give the user access to the I²C devices (e.g. membership in an `i2c` group). The README documents this.
- **All platforms:** DDC/CI must be enabled in the monitor's on-screen menu (it is on by default on Dell monitors). Some docks, adapters, and KVM switches block DDC/CI; error messages should point to that as a possible cause.

There are no external services, accounts, or API keys. Everything happens locally between the computer and the monitor.

---

### Data model

Nothing is saved to disk. These are the things the tool works with while it runs; every value is read live from the monitor each time.

#### Monitor

- **index** — the number shown in `dellctl list` (1, 2, …), used with `--monitor`.
- **model name** — e.g. "DELL U3425WE".
- **serial number** — when the monitor reports one.
- **supported controls** — which of input, brightness, volume, and mute this monitor exposes over DDC/CI.

#### Input

- **friendly name** — e.g. `hdmi`, `dp`, `thunderbolt` / `usb-c`.
- **raw code** — the monitor's internal number for this input (shown for troubleshooting, accepted as an escape hatch).
- **active** — whether this is the input currently showing.

#### Setting reading

- **brightness** — current value (0–100).
- **volume** — current value and the monitor's maximum.
- **mute state** — speakers on or muted.

Each Monitor has several Inputs and one set of Setting readings. The list of inputs comes from the monitor itself rather than being hardcoded per model.

---

### Command reference (user-facing behavior)

All commands accept `--monitor <n | model text>` to pick a monitor and `--help`. Read commands accept `--json`. The binary also supports `--version`.

| Command | Behavior |
|---|---|
| `dellctl` / `dellctl status` | Status summary for the selected monitor (or all monitors if several and none selected) |
| `dellctl list [--verbose]` | List detected monitors; `--verbose` adds supported controls |
| `dellctl input` | Show the active input |
| `dellctl input list` | List supported inputs, active one marked |
| `dellctl input set <name \| code>` | Switch input |
| `dellctl brightness [value \| +n \| -n]` | Show, set, or nudge brightness |
| `dellctl speaker [on \| off \| toggle]` | Show or change speaker mute state |
| `dellctl volume [value \| +n \| -n]` | Show, set, or nudge volume |

---

### Feature details

#### 1. Detect monitors

**In scope**
- `dellctl list` shows each DDC-capable monitor with index, model name, and serial, e.g. `1  DELL U3425WE  (serial ABC123)`.
- If exactly one Dell is connected, every command targets it automatically.
- `--monitor <n>` or `--monitor <model text>` (e.g. `--monitor U3425`) selects a specific monitor.
- If several monitors match and none was chosen, commands that change settings stop with a short message listing them.
- When no monitor is found, a clear message with a platform-specific hint (e.g. the Arch I²C setup, DDC/CI menu setting, docks/adapters).
- `dellctl list --verbose` also shows which controls each monitor supports.

**Out of scope**
- Remembering a default monitor between runs.
- Applying a command to all monitors at once.
- Live watching for monitors being plugged/unplugged.

#### 2. List inputs

**In scope**
- `dellctl input list` shows the monitor's supported inputs with friendly names; the active one is marked with `*`, and each shows its raw code in brackets.
- `dellctl input` with no arguments shows just the active input.
- `--json` gives machine-readable output (true of every read command).

**Out of scope**
- Showing whether a cable or signal is present on each input.
- Custom labels for inputs (e.g. "Work laptop").

#### 3. Switch input

**In scope**
- `dellctl input set <name>` switches inputs. Names are case-insensitive with common aliases: `usb-c` / `usbc` / `tb` / `thunderbolt`, `dp` / `displayport`, `hdmi` / `hdmi1`.
- A raw code (e.g. `0x0f`) is also accepted.
- An unknown name prints the list of valid inputs for this monitor.
- Switching to the already-active input is a no-op that prints `Already on hdmi`.
- Success prints a one-line confirmation: `Switched to hdmi`.
- Note: after switching away from the computer running the command, that computer may not be able to switch back on some setups; behavior on the U3425WE should be verified and noted in the README.

**Out of scope**
- Cycling to the "next" input.
- Timed or automatic switch-back.
- Controlling KVM or auto-input-select settings.

#### 4. Brightness

**In scope**
- `dellctl brightness` shows `Brightness: 65`.
- `dellctl brightness 40` sets an exact value (0–100).
- `dellctl brightness +10` / `-10` nudges, clamped to 0–100 without error.
- Exact values outside 0–100 are rejected with a clear message.
- Changes print before → after: `Brightness: 65 → 75`.

**Out of scope**
- Fade/transition effects.
- Time-of-day or ambient auto-brightness.
- Contrast and other picture settings.

#### 5. Speaker on/off

**In scope**
- `dellctl speaker` shows `Speakers: on` or `Speakers: muted`.
- `dellctl speaker off` mutes; `dellctl speaker on` unmutes; `dellctl speaker toggle` flips the current state.
- If the monitor doesn't support mute over DDC/CI, a plain "not supported on this monitor" message that suggests `dellctl volume 0` instead.

**Out of scope**
- Routing audio between speakers and the headphone jack.
- Changing the computer's sound output device.

#### 6. Speaker volume

**In scope**
- `dellctl volume` shows `Volume: 30`.
- `dellctl volume 50` sets an exact value (0–100); `+5` / `-5` nudges, clamped at the edges.
- Changes print before → after: `Volume: 30 → 35`.
- If the speakers are muted, the change still applies and a note is added: `(speakers are muted)`. Volume changes **never** auto-unmute.
- If volume isn't supported over DDC/CI, a clear "not supported on this monitor" message.

**Out of scope**
- Syncing with the computer's system volume or media keys.
- Per-input volume levels.

#### 7. Status

**In scope**
- `dellctl status` (and bare `dellctl`) prints:
  ```
  DELL U3425WE (monitor 1)
    Input:      thunderbolt
    Brightness: 65
    Volume:     30
    Speakers:   on
  ```
- Unsupported settings show `n/a` rather than failing the whole command.
- `--json` returns the same data.
- With several monitors connected and none selected, shows status for all of them.

**Out of scope**
- Live/watch mode.
- Resolution, refresh rate, firmware version, or other extra details.

#### 8. Cross-platform

**In scope**
- Identical commands, flags, and output on macOS (Apple Silicon), Windows 11 (x64), and Arch Linux (x64).
- One standalone binary per platform; nothing extra to install to run it.
- README with per-OS build instructions and the Arch I²C setup.
- Friendly, actionable error messages (permissions, unsupported cable/dock, DDC/CI disabled in the monitor menu).
- `--help` on every command and a `--version` flag.
- CI that builds and tests on all three OSes on every push, and attaches the three binaries to a GitHub release when a version tag is pushed.

**Out of scope**
- Windows/Linux on ARM, Intel Macs.
- Installers and package managers.
- Code-signing / notarization.
- Shell completion scripts.

---

## Milestone 1 — Foundation & read-only status

Create the Rust project and the shared command-line experience, get the tool talking to the U3425WE on macOS (Apple Silicon), and deliver every read-only command. This proves the hardest piece — reliable DDC/CI communication — before anything changes monitor settings.

### What gets built

- A new Cargo project producing a `dellctl` binary with `--help` and `--version`.
- The command structure for all commands in the command reference, with the per-OS adapter boundary in place so Windows and Linux can be added later without changing the user-facing commands.
- macOS (Apple Silicon) monitor access.
- `dellctl list` and `dellctl list --verbose` (Feature 1), including auto-targeting the single Dell and `--monitor` selection.
- `dellctl input` and `dellctl input list` (Feature 2).
- Read-only `dellctl brightness`, `dellctl speaker`, and `dellctl volume` (show current value only).
- `dellctl status` / bare `dellctl` (Feature 7), including `n/a` for unsupported settings and multi-monitor output.
- `--json` on all read commands.
- A short capability report for the U3425WE recorded in the milestone log: which inputs it reports, and whether brightness, volume, and mute are readable over DDC/CI.

### What milestone 1 explicitly does NOT include

- Changing any monitor setting (input switching, setting brightness/volume, mute/unmute).
- Windows or Linux support (stubs that report "not yet supported on this platform" are fine).
- CI, release binaries, or the README.

### Done when

On the Apple Silicon Mac with the U3425WE connected, `dellctl list`, `dellctl input list`, `dellctl status`, and `dellctl status --json` all show correct, live values that match the monitor's on-screen menu, and the capability report is written to the milestone log.

---

## Milestone 2 — Monitor controls

Add every command that changes the monitor, on macOS, completing the full feature set on one platform.

### What gets built

- `dellctl input set <name | code>` with aliases, raw codes, no-op on the active input, and the valid-inputs list on a bad name (Feature 3).
- `dellctl brightness <value | +n | -n>` with clamping, validation, and before → after output (Feature 4).
- `dellctl speaker on | off | toggle` with the "not supported" fallback message (Feature 5).
- `dellctl volume <value | +n | -n>` with clamping, the muted note, and no auto-unmute (Feature 6).
- Setting-changing commands refuse to guess when several monitors match and none was selected.
- Verified behavior on the U3425WE recorded in the milestone log, including what happens when switching away from the Mac's own input.

### What milestone 2 explicitly does NOT include

- Windows or Linux support.
- CI, releases, or README.
- Any monitor setting beyond input, brightness, mute, and volume.

### Done when

On the Mac with the U3425WE, every command in the command reference works as described: inputs switch, brightness and volume change visibly/audibly, mute toggles, and `dellctl status` reflects each change.

---

## Milestone 3 — Windows & Linux support

Add the Windows 11 and Arch Linux adapters so every command behaves identically on all three platforms.

### What gets built

- Windows 11 (x64) monitor access, with no extra setup required.
- Arch Linux (x64) monitor access via the kernel's I²C interface, with a clear, actionable error when the `i2c-dev` module isn't loaded or the user lacks permission.
- Identical commands, flags, text output, and `--json` output across macOS, Windows, and Linux.
- Platform-specific hints in the "no monitor found" and permission error messages.
- The project compiles cleanly for all three targets.

### What milestone 3 explicitly does NOT include

- CI pipelines or release binaries.
- README and setup documentation (beyond notes in the milestone log).
- ARM Windows/Linux or Intel macOS.

### Done when

The binary builds on Windows 11 and Arch Linux, and — when run on a machine connected to the U3425WE (or another Dell) — `dellctl status`, `dellctl input set`, `dellctl brightness`, `dellctl speaker`, and `dellctl volume` behave the same as on macOS. If hardware access on a platform isn't available during the session, the log states exactly what was and wasn't verified on real hardware.

---

## Milestone 4 — Ship it

Make the tool easy to build, download, and trust: automated builds, release binaries, documentation, and a polish pass on messages.

### What gets built

- CI that builds and runs tests on macOS, Windows, and Linux on every push.
- A release workflow that, when a version tag is pushed, builds the three binaries (macOS Apple Silicon, Windows x64, Linux x64) and attaches them to a GitHub release.
- A README covering: what the tool does, the command reference with examples, per-OS build instructions, the Arch I²C setup, the DDC/CI menu setting, known limitations (docks/adapters, switching away from the current computer, the macOS "unidentified developer" warning), and the U3425WE as the tested model.
- A review pass over all `--help` text and error messages for clarity and consistency.

### What milestone 4 explicitly does NOT include

- Package-manager distribution (Homebrew, AUR, winget) or installers.
- Code-signing or notarization.
- Shell completion scripts.
- New features beyond those already built.

### Done when

Pushing to the repository shows green builds on all three OSes, pushing a version tag produces a GitHub release with three downloadable binaries, and a new user can follow the README to get `dellctl status` working on any of the three platforms.
