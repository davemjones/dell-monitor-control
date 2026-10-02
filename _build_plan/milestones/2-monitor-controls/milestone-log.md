# Milestone 2 — Monitor controls: log

## What's new in the app

- `dellctl input set hdmi` (or `dp`, `thunderbolt`, `usb-c`, `tb`, `displayport`, or a raw code like `0x11`) switches the monitor's input. If the monitor is already on that input it says `Already on …`. An unknown name prints the inputs this monitor actually has.
- `dellctl brightness 40` sets brightness, and `brightness +10` / `-10` nudges it. The output shows the change, e.g. `Brightness: 65 → 75`. Nudges stop at 0 and 100 without an error.
- `dellctl volume 50` / `+5` / `-5` does the same for the speaker volume. If the speakers are muted it adds `(speakers are muted)` and never unmutes them.
- `dellctl speaker on | off | toggle` mutes and unmutes the speakers on monitors that support it. The U3425WE doesn't, so it explains that and suggests `dellctl volume 0`.
- Add `--json` to any of these to get the before and after values as machine-readable output.
- With several monitors connected and none chosen, commands that change a setting stop and list the monitors rather than guess.
- Commands are more reliable: occasional DDC/CI glitches are retried automatically instead of failing.

## U3425WE verified behavior (M1 Pro over Thunderbolt, 2026-10-02)

- **Brightness:** set, nudge, and clamping (`-100` from 50 gives 0) all work. You confirmed the screen dims and recovers. Each change takes about 0.1s end to end.
- **Volume:** set and nudge work, and you confirmed the change is audible (88 → 30 → 88). Each change takes about 0.15–0.28s.
- **Mute (0x8D):** **doesn't work.** The test wrote 0x8D=1 and later 0x8D=2. The monitor accepted both writes with no error, but reading 0x8D back stayed at `0/65535`, and you confirmed the audio kept playing. `speaker on/off/toggle` therefore uses the PRD fallback message.
- **Input switching away from the Mac's own input:** **works, and the Mac can switch back.**
  - A script switched to HDMI, then to DP. Both times the Mac could still read every value over DDC while the other input was showing. `input set thunderbolt` brought the screen back on the first try both times, and `input` read `thunderbolt` again about 3s after the write.
  - Because of that, the README can say switching away and back works on this setup (M1 Pro, Thunderbolt cable, no dock).
  - Not tested: what happens when another computer is actually connected and active on HDMI/DP. The test inputs had no signal.
- **Each input keeps its own brightness.** Reading brightness while away gave HDMI 75 and DP 80, while Thunderbolt stayed 78. `status` and `brightness` report the active input's value.
- **`input set` takes about 1s**, almost all of it reading the capabilities string so the requested input can be checked against what the monitor has.
- **DDC/CI reads fail now and then** without retries: `invalid DDC/CI length` and `DDC/CI checksum mismatch`, mostly when commands run back to back. With the new retries, a stress run of 100 back-to-back commands (reads, writes, status) had 0 failures.

## What was built

- `src/backend/mod.rs`: added `write_vcp(code, value)` to the `Monitor` trait.
- `src/backend/ddc.rs`:
  - `write_vcp` uses `ddc_hi::Ddc::set_vcp_feature`.
  - New `retry()` helper: both reads and writes get up to 4 attempts, pausing 50, 100, then 150ms between them. Writes are always absolute values, so retrying them is safe.
- `src/backend/fake.rs`:
  - `FakeMonitor::write_vcp` updates the stored value, keeping the old max.
  - Every write is recorded in a `WriteLog` (`Rc<RefCell<Vec<(u8, u16)>>>`). It's shared across the copies `FakeBackend` hands out, so tests can check exactly which writes were sent.
  - A `read_only` flag makes writes fail.
- `src/vcp.rs`:
  - `parse_input` matches an alias, then a canonical name, then a raw code: `0x..` is hex, bare digits are decimal, and anything else hex-like (e.g. `0f`) is hex.
  - Also new: `apply_level` (sets or nudges, clamped to 0–100), `from_percent` (the inverse of `percent`), and `mute_value`.
- `src/commands.rs`:
  - Replaced the `NOT_YET` branch with `input set`, `brightness <level>`, `volume <level>`, and `speaker on|off|toggle`.
  - Brightness and volume share a `change_level` helper.
  - The unsupported-mute message now comes from `mute_unsupported()`, shared by the read and change paths.
  - The invalid-input error comes from `unknown_input()` and reuses `output::inputs_text`.
- `src/cli.rs`: added help text for the `input set` argument.
- Tests: 47 total, up from 32. New tests cover set, nudge and clamping, scaling for monitors whose max isn't 100, the muted note and no auto-unmute, speaker on/off/toggle/already, input aliases, codes, the no-op case and rejected inputs, missing capabilities, refusing to guess between several monitors, write failures, and every JSON shape.

## Decisions made during implementation

- **JSON shapes for changes** (you chose before/after objects):
  - `brightness`: `{"brightness": {"before", "after"}}`.
  - `volume`: `{"volume": {"before", "after"}, "speakers": "muted"|"on"|null}`.
  - `speaker`: `{"speakers": {"before", "after"}}`.
  - `input set`: `{"before": {name, code}, "after": {name, code}, "changed": bool}`.
- **Speaker text:** a change prints `Speakers: on → muted`. Asking for the current state writes nothing and prints `Speakers: already muted` / `already on`.
- **Brightness/volume always write**, even when the new value equals the current one (e.g. `brightness +0`). The write is harmless, and the output stays a uniform `X → X`.
- **No read-back after an input switch.** The original worry was that the Mac might lose DDC after switching away. The test showed it doesn't on this setup, but other setups (docks, other cables) may differ, so the command prints `Switched to …` as soon as the write succeeds.
- **Input validation uses the capabilities list.** Inputs the monitor doesn't list are refused, e.g. `0x12` or `vga` on the U3425WE. If capabilities can't be read, known names and raw codes are accepted unchecked, and the error lists the known names instead.
- **Read failures before a change aren't called "unsupported".** Before the retries were added, a single glitch produced a misleading "doesn't support volume" message. The error now reads `couldn't read volume, so it wasn't changed: <cause>`.

## Notes for milestone 3

- `write_vcp` and the retry helper live in `DdcHiMonitor`, so Windows and Linux get both automatically. The retry timing (up to 4 attempts, 50/100/150ms pauses) was tuned only on macOS. If Linux I²C is flakier, increasing `ATTEMPTS` is the simplest fix.
- On Windows, `ddc-winapi`'s `set_vcp_feature` goes through `SetVCPFeature`; check that the input switch to 0x19 (a Dell-specific code) is accepted there.
- Check that the Linux error for missing write permission on `/dev/i2c-*` isn't wrapped in a misleading message. Write errors surface as `couldn't change setting 0x.. on <name>: <cause>`.
- All behavior lives in `commands.rs` and is platform-independent. A new platform only needs the backend to enumerate, read, write, and return capabilities.

## Deviations from the PRD

- **Added:** `--json` on commands that change settings. The PRD only requires it for reads; you chose to add it.
- **Added:** automatic retries for DDC/CI reads and writes. The PRD doesn't mention them, but without them commands failed now and then on real hardware.
- **Speaker mute** can't be demonstrated on the U3425WE because the monitor ignores 0x8D. The on/off/toggle logic is covered by fake-backend tests, and on real hardware you get the PRD's "not supported, try `dellctl volume 0`" path.
