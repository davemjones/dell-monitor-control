# Milestone 1 — Foundation & read-only status: log

## What's new in the app

- `dellctl list` shows your connected monitor and its serial number, e.g. `1  DELL U3425WE  (serial JK4HV84)`. `list --verbose` adds which controls the monitor supports.
- `dellctl` (or `dellctl status`) shows the active input, brightness, volume, and speaker state on one screen. Anything the monitor doesn't support shows `n/a`.
- `dellctl input` shows the active input; `dellctl input list` lists every input the monitor offers, with the active one marked `*`.
- `dellctl brightness`, `dellctl volume`, and `dellctl speaker` each show their current value.
- Add `--json` to any of these to get machine-readable output.
- `--monitor <number or model>` picks a specific monitor; with a single Dell connected you never need it.
- Setting commands (`brightness 50`, `volume +5`, `speaker off`, `input set hdmi`) are recognized and validated, but print "coming in milestone 2" for now.

## U3425WE capability report (tested on an M1 Pro over Thunderbolt, 2026-10-02)

- **Raw capabilities string:**
  `(prot(monitor)type(LCD)model(U3425WE)cmds(01 02 03 07 0C E3 F3)vcp(02 04 05 08 10 12 14(01 04 05 06 08 09 0B 0C) 16 18 1A 52 60(19 0F 11 ) 62 66(00F2) 67 68 87 AC AE B2 B6 C6 C8 C9 CA CC(02 0A 03 04 08 09 0D 06 ) D6(01 04 05) DC(00 03 05 ) DF E0(03) E2(00 02 04 0C 0D 0F 10 11 13 0B 1A 1B 3D 14 ) E4(00 01) E5 E7(02 03) E8 E9(00 01 02 21 22 24 2B 2C ) EA(FC F8 ) F0(09 0A A1 ) EE EF(00 01 0F) F1 F2 FD)mswhql(1)asset_eep(40)mccs_ver(2.1))`
- **MCCS version:** 2.1.
- **Inputs (VCP 0x60):** `0x0F` = DisplayPort (`dp`), `0x11` = HDMI (`hdmi`), `0x19` = Thunderbolt/USB-C (`thunderbolt`, a Dell-specific code). The active input is reported as `0x1919`, with the code repeated in the high byte, so only the low byte is used. The reported maximum for 0x60 is meaningless.
- **Brightness (0x10):** supported, scale 0–100 (read 78).
- **Volume (0x62):** supported, scale 0–100 (read 88).
- **Speaker mute (0x8D):** **not advertised** in the capabilities string. Reading it returns 0 with max 65535, which isn't a valid mute value (1 = muted, 2 = unmuted). Treated as unsupported: `status` shows `n/a` and `dellctl speaker` explains this and suggests `volume 0`.
- **Timings:** finding monitors takes about 50ms, each value read about 50ms, and the capabilities string about 950ms. Measured end-to-end: `status` 0.24s, `brightness` 0.1s, `list` 0.5s (includes the serial lookup), `input list` 1.0s.

## What was built

- `Cargo.toml`: the binary `dellctl`, edition 2024. Dependencies: `clap` (derive), `ddc-hi` 0.4, `mccs-caps` 0.1, `mccs` 0.1, `serde`, `serde_json`, `anyhow`, and `plist` (macOS only).
- `src/main.rs`: parses arguments, runs the command, prints output. Errors print as `error: …` and exit 1; clap usage errors exit 2.
- `src/cli.rs`: clap definitions for the full PRD command reference, including the setting forms. `Level` is `Set(u16)` or `Delta(i16)`; out-of-range or non-numeric values are rejected at parse time. `allow_hyphen_values` lets `-10` work as a value.
- `src/backend/mod.rs`: the `Backend`, `Monitor`, `MonitorInfo`, `VcpValue` and `Caps` types. `Caps::parse` wraps `mccs_caps`. `MonitorInfo::is_dell` matches manufacturer `DEL` or "DELL" in the name.
- `src/backend/ddc.rs`: `DdcHiBackend` / `DdcHiMonitor`, the real implementation for all OSes via `ddc-hi`. The monitor name is `model_name`, falling back to `id`; on macOS the id is "DELL U3425WE".
- `src/backend/macos_serial.rs`: runs `ioreg -a -r -c AppleCLCD2 -d 1` and reads `DisplayAttributes.ProductAttributes` (`ProductName` → `AlphanumericSerialNumber`). Best effort: duplicate names get no serial.
- `src/backend/fake.rs` (tests only): `FakeMonitor` / `FakeBackend`, including a `u3425we()` fixture built from the real readings above.
- `src/vcp.rs`: VCP codes, the input name table, `INPUT_ALIASES` (ready for milestone 2; marked `allow(dead_code)`), low-byte masking, mute validation, and percent scaling.
- `src/select.rs`: monitor selection (`one`, `all_or_one`) and OS-specific "no monitors found" hints.
- `src/output.rs`: output structs (`Status`, `ListEntry`, `Controls`, `InputEntry`, `InputRef`) and their text renderers.
- `src/commands.rs`: `run(&Cli, &dyn Backend) -> Result<String>`, the logic for every command.
- 32 unit tests, covering parsing, selection, the real capabilities fixture, the ioreg plist parser, and exact text/JSON output for each command.

## Decisions made during implementation

- **Fast reads skip the capabilities string.** `status`, `input`, `brightness`, `volume` and `speaker` read values directly; a failed or invalid read becomes `n/a`. Only `input list` and `list --verbose` read the capabilities string, which takes about 1s.
- **Serials are looked up only for `list`**, through `DdcHiBackend { with_serials }`, to keep other commands fast.
- **Values are shown as 0–100.** Readings are scaled with `vcp::percent(value, max)`; on the U3425WE the monitor's max is already 100. Milestone 2 must scale back the other way when writing to a monitor whose max isn't 100.
- **`status --json`** returns a single object when it shows one monitor and an array when it shows several (several monitors connected, none selected). `list --json` and `input list --json` always return arrays.
- **The input list follows code order** (dp, hdmi, thunderbolt), not the monitor's own order.
- **Input code 0x1B was left unnamed** (it shows as `input-0x1b`), so that the alias `usb-c` maps unambiguously to Dell's `thunderbolt` (0x19).
- **Range validation happens at parse time**, so `brightness 150` exits 2 with a usage error before any monitor is touched.

## Notes for milestone 2

- Writing: use `ddc_hi::Ddc::set_vcp_feature(code, value)` on `display.handle` and add a `write_vcp` method to the `Monitor` trait and `FakeMonitor`.
- **Mute:** the U3425WE doesn't advertise 0x8D. As agreed, try writing 0x8D = 1 and 0x8D = 2 on the real monitor before settling behavior. If it does nothing, keep the current "not supported, try `volume 0`" path.
- **Input switching:** write the bare code (e.g. `0x11`), not the doubled form. Check what happens when switching away from Thunderbolt: can the Mac still read or switch back over DDC? Document the result.
- Input-name resolution should check `INPUT_ALIASES`, then the canonical names from `vcp::input_name`, then raw codes (`0x0f` or decimal). It should refuse codes not in the monitor's capabilities list, and list the valid ones.
- The `NOT_YET` branch in `commands.rs` is the single place to replace.

## Notes for milestone 3

- `ddc-hi`'s default features already include Windows (`ddc-winapi` and `nvapi`) and Linux (`ddc-i2c`). Nothing beyond macOS has been compiled or run yet.
- On Windows and Linux, `model_name`, `manufacturer_id` and `serial_number` should come from EDID. Confirm the names look like "DELL U3425WE" and not a device path or "Generic PnP Monitor".
- `select::no_monitors_message` already has Linux and Windows hint text.

## Deviations from the PRD

- None in behavior. The only addition is that `status --json` returns an object or an array depending on how many monitors it shows.
- Toolchain warning: `mccs-caps` 0.1 pulls in `nom` 3.2.1, which triggers a "future incompatibility" warning from Cargo. It's harmless today. `ddc-hi` 0.4 requires `mccs-caps` 0.1, so a fix has to come upstream.
