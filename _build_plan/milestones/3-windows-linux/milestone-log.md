# Milestone 3 — Windows & Linux support: log

## What's new in the app

- `dellctl` now builds for **Windows 11 (x64)** and **Arch Linux (x64)** as well as macOS, with the same commands, flags, and text/JSON output everywhere.
- On Windows, monitors show their real name and serial (e.g. `DELL U3425WE`) instead of "Generic PnP Monitor". The single Dell is still picked automatically, and `--monitor U3425` still works.
- On NVIDIA graphics cards in Windows, each monitor is listed once instead of twice.
- On Linux, if the I²C setup is missing, `dellctl` says exactly what's wrong and prints the commands to fix it. That covers the `i2c-dev` module not being loaded and your user lacking permission.
- The Windows `.exe` runs on a clean Windows install. It doesn't need the Visual C++ runtime.
- Reading the monitor's capability list (used by `input list`, `input set`, `list -v`) now retries glitches too, so those commands are as reliable as the rest.

## Hardware verification status

**No Windows or Linux machine was connected to the U3425WE during this milestone.** You chose to verify by compiling and cross-building only. What was and wasn't checked:

| Check | macOS (M1 Pro) | Windows 11 x64 | Arch Linux x64 |
|---|---|---|---|
| Compiles, clippy has no warnings (all targets) | ✅ | ✅ (from macOS) | ✅ (Docker, Debian bookworm) |
| Unit tests run | ✅ 52 pass | ❌ not run (test binary not executed) | ✅ 49 pass (in Docker; the other 3 are macOS-only) |
| Release binary builds | ✅ | ✅ `dellctl.exe`: PE32+ x86-64 console, built with cargo-xwin | ✅ ELF x86-64, built in Docker |
| Binary starts, `--version`/`--help` | ✅ | ❌ not run | ✅ in an `archlinux:latest` container |
| "No I²C devices" message | n/a | n/a | ✅ in the Arch container |
| "No permission" message | n/a | n/a | ✅ in the Arch container: a root-only `/dev/i2c-3`, run as `nobody` |
| Finds the U3425WE, shows its name and serial | ✅ (re-checked) | ❌ **not verified** | ❌ **not verified** |
| `status`, `input set`, `brightness`, `speaker`, `volume` on real hardware | ✅ (re-checked `list`, `status`, `brightness ±5`, `input list`) | ❌ **not verified** | ❌ **not verified** |

When hardware is available, run `dellctl list`, `dellctl status`, `dellctl input list`, `dellctl brightness +5` / `-5`, `dellctl volume +5` / `-5` and `dellctl input set <other input>` / back on each OS. Also check the name shows as `DELL U3425WE` and not a device path.

## What was built

- **`Cargo.toml`**
  - `ddc-hi` now uses `default-features = false, features = ["ddc-i2c", "ddc-winapi", "ddc-macos"]`, which drops `nvapi`.
  - Added a Windows-only `windows-sys` 0.61 dependency with the features `Win32_Foundation`, `Win32_Devices_Display`, `Win32_Graphics_Gdi` and `Win32_System_Registry`.
- **`.cargo/config.toml`** (new): `+crt-static` for `x86_64-pc-windows-msvc`. Without it the `.exe` needed `VCRUNTIME140.dll`. It now imports only system DLLs: advapi32, user32, dxva2, kernel32, ntdll, dbghelp, and an api-ms-win synch shim.
- **`src/backend/windows_edid.rs`** (new; the Win32 code is `cfg(windows)`, the parsing compiles everywhere for tests)
  - `lookup()` walks the monitors in the same order `ddc-winapi` uses: `EnumDisplayMonitors`, then `GetNumberOfPhysicalMonitorsFromHMONITOR`, then `EnumDisplayDevicesW(…, EDD_GET_DEVICE_INTERFACE_NAME)`, keeping only active devices.
  - It reads each monitor's `EDID` value from `HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\<hw>\<instance>\Device Parameters` and parses it with `ddc_hi::DisplayInfo::from_edid`, the same parser macOS uses.
  - `parse_device_id()` (pure, unit-tested) turns the device interface name into the 3-letter PnP manufacturer code and the registry key.
  - Without an EDID, the PnP code alone (`DEL`) still marks the monitor as a Dell.
- **`src/backend/linux_i2c.rs`** (new; the I/O is `cfg(linux)`, the logic is tested everywhere)
  - `probe()` tries to open every `/dev/i2c-*` read-write.
  - `diagnose()` turns the result into the "module not loaded" message (no devices), the "no permission to open …" message (any device fails with `EACCES`), or `None`, which means the generic hint applies.
- **`src/backend/ddc.rs`**
  - Restructured `enumerate`: build every `MonitorInfo` first, through the new `info_from()`, then apply the per-OS enrichment:
    - **Windows:** EDID names, used only when the EDID list has the same count as the displays.
    - **macOS:** serials, unchanged.
    - **Linux:** when no displays are found, a diagnosis replaces the empty list.
  - `capabilities()` now uses `retry()` as well.
- **`src/select.rs`**: `no_monitors_message` now shares one hint (DDC/CI menu setting, docks and adapters, try a direct cable). Windows adds a note about graphics drivers. The Linux-specific advice moved into the backend diagnosis.
- Tests: 52 on macOS, up from 47. New tests cover device-ID parsing (2) and the Linux diagnosis branches (3).

## Decisions made during implementation

- **`nvapi` disabled.** On NVIDIA GPUs it lists every monitor again, so `select::one` would see two "DELL U3425WE"s and refuse to guess. The WinAPI path works on every GPU vendor. If WinAPI DDC turns out to be unreliable on a particular NVIDIA setup, re-enabling `nvapi` would need de-duplication by serial or EDID.
- **EDID names on Windows are paired with monitors by position**, and only when the counts match. If they don't match (unusual mirrored or virtual-display setups), the names fall back to ddc-hi's description instead of risking the wrong name. `--monitor <n>` still works there.
- **Only `EACCES` counts as "no permission"** on Linux. Testing in a container showed `EPERM` comes from device cgroups, sandboxes or security modules, where the i2c-group fix wouldn't help, so that case falls back to the generic hint.
- **The Linux diagnosis runs only when zero monitors are found.** If one bus works and another is denied, the working monitors are used without a warning.
- **The Linux build keeps the udev dependency** that `ddc-i2c` uses to find devices. The binary links `libudev.so.1` at runtime. It's part of `systemd-libs` in Arch's `base`, so nothing extra needs installing. Building it needs `libudev` headers and `pkg-config`; Arch's `systemd` package provides both, and on Debian/Ubuntu they're `libudev-dev` and `pkg-config`.
- **The static Windows CRT** is set in a checked-in `.cargo/config.toml`, so every build (local, cargo-xwin, CI) gets it.
- **Unicode in output** (`→`, `²`): Rust writes to Windows consoles with `WriteConsoleW`, so the characters show correctly in Windows Terminal and conhost. Piped output is UTF-8. Not seen on a real Windows console yet.

## Notes for milestone 4

- **CI (Linux runner):** `sudo apt-get install -y libudev-dev pkg-config` before building. `cargo check` fails without them too.
- **CI (Windows runner):** build natively with `--target x86_64-pc-windows-msvc` (or the host default). `.cargo/config.toml` adds the static CRT automatically. The `windows_edid` unit tests run on every OS.
- **CI (macOS runner):** use `macos-14` or newer (Apple Silicon) for an `aarch64-apple-darwin` binary.
- **Cross-building on this Mac (handy for spot checks):**
  - Windows: `cargo xwin build --release --target x86_64-pc-windows-msvc` (cargo-xwin is installed; it caches the MSVC SDK).
  - Linux: `docker run --platform linux/amd64 -v "$PWD":/src -w /src -e CARGO_TARGET_DIR=/src/target/docker-linux rust:1-bookworm`, then install `libudev-dev pkg-config` and run `cargo build --release`.
- **README, Arch setup section:** reuse the exact commands from `linux_i2c::diagnose`:
  - `sudo modprobe i2c-dev`
  - `echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf`
  - `sudo groupadd --system i2c`
  - `sudo usermod -aG i2c $USER`
  - the udev rule `KERNEL=="i2c-[0-9]*", GROUP="i2c", MODE="0660"` in `/etc/udev/rules.d/45-i2c.rules`
  - then log out and back in.
- **README, known limitations:** Windows and Linux haven't been tested on real hardware yet, unless that happens before release.
- **README, switching away:** milestone 2 showed that on macOS over Thunderbolt the Mac keeps DDC access and can switch back.
- The `nom` 3.2.1 future-incompatibility warning from `mccs-caps` is still there on every platform. It's harmless, but CI shouldn't use `-D warnings` on Cargo's own future-incompat output.

## Deviations from the PRD

- **Real-hardware verification on Windows and Arch wasn't done**, because no hardware was available this session (your call). The "Done when" behavior on real monitors is unverified for those two platforms; see the table above.
- **Added:** a static CRT for Windows, so the PRD's "nothing extra to install" holds even without the Visual C++ Redistributable.
- **Added (fix):** `capabilities()` retries. Milestone 2 only retried reads and writes; this gap turned up while re-checking the Mac.
