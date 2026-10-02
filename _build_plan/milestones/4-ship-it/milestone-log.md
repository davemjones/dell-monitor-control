# Milestone 4 — Ship it: log

## What's new in the app

- A **README** explains what `dellctl` does, how to install it on each OS, the one-time Arch Linux setup, every command with real example output, known limitations, and a troubleshooting table.
- **Downloadable releases:** pushing a version tag will build macOS, Windows, and Linux binaries and publish them on GitHub as archives, with a `SHA256SUMS` file for checking downloads.
- **Automatic checks:** every push will build, lint, and test the tool on macOS, Windows, and Linux.
- `dellctl --help` now has a short intro and an **Examples** section, and the subcommand help is clearer (e.g. `speaker` says it controls mute).
- **Clearer errors** when the monitor stops answering. They name the setting (e.g. "couldn't read brightness (VCP 0x10) from DELL U3425WE") and suggest checking the DDC/CI menu setting and docks.
- **Changes now retry automatically**, like reads. This fixes a bug left over from milestone 2.
- `--json` output now lists `before` before `after`, and keeps fields in a natural order.
- The project is open-source under **MIT OR Apache-2.0**.

## What was built

- **`.github/workflows/ci.yml`**
  - Runs on every push and PR on a `macos-14` (Apple Silicon), `windows-latest`, `ubuntu-latest` matrix, with `fail-fast: false`.
  - Steps: checkout, stable toolchain with clippy and rustfmt, `Swatinem/rust-cache`, `libudev-dev pkg-config` on Linux, `cargo fmt --check` (on Linux only), `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build --release`.
- **`.github/workflows/release.yml`** (on tags `v*`)
  - `check-version`: the tag minus `v` must equal the `version` in `Cargo.toml`.
  - `build` matrix, using `cargo build --release --locked --target …`:
    - `macos-14`/`aarch64-apple-darwin` gives `.tar.gz`
    - `ubuntu-22.04`/`x86_64-unknown-linux-gnu` gives `.tar.gz` (glibc 2.35)
    - `windows-latest`/`x86_64-pc-windows-msvc` gives `.zip` (via `7z`)
  - Each archive is `dellctl-<tag>-<label>/` containing the binary, `README.md`, `LICENSE-MIT` and `LICENSE-APACHE`.
  - `release`: downloads all three artifacts, writes `SHA256SUMS`, and publishes with `softprops/action-gh-release@v2` (`generate_release_notes`, `fail_on_unmatched_files`). Only this job gets `contents: write`.
- **`README.md`**: covers everything listed under "What's new". It links to `davemjones/dell-monitor-control` and doesn't reference `_build_plan/`.
- **`LICENSE-MIT`, `LICENSE-APACHE`**: the canonical Apache 2.0 text (sha256 `cfc7749b…`), and MIT with "Copyright (c) 2026 David Jones".
- **`Cargo.toml`**
  - Added `description`, `license`, `repository`, `readme`, `keywords` and `categories`.
  - `serde_json` now has `preserve_order`.
  - New `[profile.release]` with `strip = true`, `lto = "thin"` and `codegen-units = 1`. Binaries are about 1.0 MB on macOS and Windows.
- **`src/cli.rs`**: `long_about` mentions auto-targeting and the DDC/CI menu setting, and `after_help` has 8 examples. The `list`, `input`, `speaker` and `input set` help text is reworded.
- **`src/backend/ddc.rs`**
  - Read, write and capabilities errors now name the setting through `vcp::feature_name` and end with `hint: …` (`select::DDC_HINT`).
  - **`write_vcp` now goes through `retry()`.**
- **`src/vcp.rs`**: added `feature_name(code)`, e.g. "brightness (VCP 0x10)".
- **`src/select.rs`**: `DDC_HINT` is public and shared with the backend.
- **`src/commands.rs`**: removed the duplicate outer error contexts in `read_input`, `read_percent` and `change_level`, and their now-unused `what` parameters.
- **`src/backend/fake.rs`**: its error messages now match the real backend's wording.

## Verification done locally (nothing pushed, at your request)

| Check | Result |
|---|---|
| `actionlint` on both workflows, with shellcheck on every `run:` script | 0 errors |
| CI steps on macOS (`fmt --check`, `clippy -D warnings`, test, release build) | ✅ 52 tests |
| CI steps on **Ubuntu 22.04** in Docker (amd64), the same OS as the release runner | ✅ 49 tests (the 3 macOS-only ones excluded) |
| Windows: `clippy --all-targets -D warnings --target x86_64-pc-windows-msvc`, plus a `cargo xwin build --release --locked` | ✅; the `.exe` doesn't need the VC++ runtime |
| Release **Package** step, taken from `release.yml` itself and run with the matrix values filled in | ✅ macOS and Linux archives; layout checked |
| `SHA256SUMS` generated and checked (`shasum -c`) | ✅ |
| Linux binary from the archive (built on Ubuntu 22.04) running on Arch | ✅ `--version`; `status` gives the i2c-dev hint |
| Windows zip layout (made locally with `zip`, not `7z`) | ✅ same layout |
| README examples against the U3425WE | ✅ output matches word for word. Your serial is shown as `ABC1234`, and the example values come from real runs. |
| Final Mac hardware check: status, 30 back-to-back changes and reads, input list/set | ✅ 0 failures; settings restored to 78/88 |

**Can only be confirmed after a push:**
- green CI on GitHub's runners;
- the Windows `7z` packaging step;
- the release job creating a release with 4 assets: 3 archives plus `SHA256SUMS`.

**Still open from milestone 3:**
- real-hardware checks on Windows 11 and Arch. The README says so under Known limitations.

## Decisions made during implementation

- **Example output uses a placeholder serial (`ABC1234`)**, so the README doesn't publish your monitor's serial. The unit-test fixture still holds the real serial (`JK4HV84`), as it has since milestone 1.
- **`serde_json` `preserve_order`** fixed `json!` objects printing keys alphabetically (`after` came before `before`). This only changes key order; tests compare parsed values and didn't need changes.
- **Error layering:** the backend error is now the whole message, followed by a hint. Before, commands wrapped it in a second "couldn't read volume: couldn't read setting 0x62…" layer. Example:
  `couldn't read volume (VCP 0x62) from DELL U3425WE: DDC/CI error: …` then `hint: make sure DDC/CI is enabled…`
- **Release builds Linux on `ubuntu-22.04`**, for an older glibc baseline. CI uses `ubuntu-latest`.
- **`--locked` in release builds**, so a release never resolves different dependency versions from the committed `Cargo.lock`.
- **Version stays `0.1.0`**, so the first release tag should be `v0.1.0`.
- **Release notes** are GitHub's auto-generated ones. There's no CHANGELOG file.

## Correction to milestone 2

The milestone 2 log says writes are retried. **They weren't.** That edit targeted a single-line form of the call, but `cargo fmt` had already split it across lines, so the replacement matched nothing and the code was left unchanged. Milestone 2's hardware runs passed without it. It was found while reworking `ddc.rs` in this milestone and is fixed now (`ddc.rs` `write_vcp` → `retry()`).

## How to release (for whoever ships next)

1. Create the GitHub repo `davemjones/dell-monitor-control` and push: `git remote add origin git@github.com:davemjones/dell-monitor-control.git && git push -u origin main`. Check that CI is green on all three OSes.
2. Bump `version` in `Cargo.toml` if needed, run `cargo build` (to update `Cargo.lock`), and commit.
3. `git tag v0.1.0 && git push origin v0.1.0`. The Release workflow checks the version, builds and packages the three binaries, and publishes the release with `SHA256SUMS`.
4. If the tag and `Cargo.toml` disagree, `check-version` fails before anything is built. Delete the tag, fix the mismatch, and tag again.

## Deviations from the PRD

- **"Done when" isn't fully demonstrated:** there's been no push yet, so no green builds on GitHub and no published release. Everything those workflows run was reproduced locally, as the table shows. The remaining risk is runner-specific: action versions, `7z` on Windows, and release permissions.
- **"A new user can follow the README on any of the three platforms":** the setup steps are written and the Linux error paths were exercised in containers. Following them on real Windows and Arch hardware is still open (see milestone 3).
- **Added beyond the PRD:** `SHA256SUMS`, archives instead of bare binaries (your choice), the dual MIT/Apache license (your choice), a smaller release profile, and stable JSON key order.
