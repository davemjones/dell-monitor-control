# Monitor Control

> **About these build-plan files:** Everything in `_build_plan/` (this PRD and the per-milestone folders) is a **temporary documentation and guidance artifact** for the initial build-out of this codebase. These files are not functional — no code, configuration, runtime logic, tests, or deployment process should import, read, reference, or depend on anything in `_build_plan/`. Once the initial milestones are built and shipped, the entire `_build_plan/` folder is expected to be deleted from the codebase. Do not treat it as long-living documentation.

## What we're building

**Monitor Control** is a native macOS menu bar app for the Dell monitor. Clicking its icon opens a small panel that shows the active input and lets you switch to another with one click, plus a brightness slider. The app is purely a front end: it does all monitor reading and control by running the existing `dellctl` command-line tool and reading its JSON output — it never talks to the monitor itself.

This is a new phase of the `dellctl` project. The `dellctl` CLI (Rust, in this repo) is complete: it detects the monitor, lists and switches inputs, reads and sets brightness, and offers `--json` output on its read commands. Monitor Control reuses all of that by running the CLI as a separate program. **The app must not reimplement any monitor-control logic** — no DDC/CI, no direct hardware access, no duplicated input-name or brightness rules beyond what is needed to display the CLI's answers. If the app needs something the CLI doesn't provide, the CLI is extended instead.

The primary target is the **Dell U3425WE** on an **Apple Silicon Mac running macOS 14 (Sonoma) or newer**. The app is written in **Swift with SwiftUI**, built with **Swift Package Manager** plus a small build script (no Xcode project required — this Mac has the Command Line Tools only), and lives in its own folder in this repo alongside the Rust code. The build is split into three milestones: the menu bar shell with live read-only status, then the controls, then polish and shipping.

---

### What the app does

- Puts a small monitor icon in the macOS menu bar; clicking it opens a compact panel.
- Shows the monitor's model name, its active input, and its current brightness — read fresh from `dellctl` every time the panel opens.
- Lists the monitor's inputs with a checkmark on the active one; one click switches inputs.
- Offers a brightness slider (0–100) that the monitor follows live while dragging.
- Shows clear, friendly messages with a Retry button when something goes wrong, instead of freezing or crashing.
- Ships as a single `Monitor Control.app` with its own copy of `dellctl` inside, built by one command.
- Can launch automatically at login, and can be quit from the panel.

---

### Already provided by the existing codebase

- The `dellctl` CLI (Rust, built with `cargo build --release`), including:
  - Monitor detection, with automatic selection when exactly one Dell is connected.
  - `dellctl input list` / `dellctl input set <name>` — list and switch inputs, with friendly names (`thunderbolt`, `dp`, `hdmi`, …).
  - `dellctl brightness [value]` — show and set brightness (0–100).
  - `dellctl status` — one-shot summary of model, input, brightness, and more.
  - `--json` output on read commands, intended for scripting.
  - Friendly error messages for "no monitor found", multiple monitors, DDC/CI disabled, and unsupported controls.
- The README, CI, and release setup for the CLI.

---

### Out of scope

- **Volume and speaker mute controls** — deliberately skipped this phase; the CLI already supports them, so they're easy to add later.
- **Multiple-monitor support** — the app controls the single Dell `dellctl` auto-selects; no monitor picker.
- **Global keyboard shortcuts** — would need their own permissions and settings flow.
- **Windows and Linux tray apps** — this phase is macOS only.
- **Background auto-refresh** — the app only reads the monitor when the panel opens.
- **Custom input names** (e.g. "Work laptop") — the app uses `dellctl`'s names, made readable.
- **Code-signing, notarization, App Store distribution** — built and run locally; macOS may ask for approval on first launch.
- **Auto-updates** — no update checks or "new version available" prompts.
- **Any monitor-control code in the app itself** — everything goes through the `dellctl` CLI.

---

### Data model

The app stores almost nothing; the monitor and `dellctl` are the source of truth.

#### Monitor (read live from `dellctl`, not stored)
- Model name — e.g. "DELL U3425WE", shown at the top of the panel.
- Brightness — the current brightness, 0–100.
- Active input — which input the monitor is currently showing.

#### Input (read live from `dellctl`, not stored)
- Name — `dellctl`'s friendly name (e.g. `thunderbolt`, `dp`, `hdmi`), shown with a readable label.
- Active — whether this is the input currently showing.

A Monitor has several Inputs, exactly one of which is active.

#### Launch at login (remembered)
- On or off. Stored by macOS itself as a standard Login Item; the app only turns it on or off and reads its current state.

---

### Feature details

#### 1. Menu bar icon

**In scope**
- A monochrome monitor icon (Apple's standard "display" symbol) that adapts to light and dark menu bars like system icons.
- Clicking it opens a compact panel anchored below the icon; clicking elsewhere closes it.
- No Dock icon, no app window, not in the Cmd-Tab switcher.
- The panel's top line shows the monitor's model name.
- A quiet loading indicator in the panel while it reads from the monitor.

**Out of scope**
- Showing brightness or input as text next to the icon in the menu bar.
- An icon that changes based on the active input.
- Custom icon choice.
- Separate right-click behavior.

#### 2. Current status at a glance

**In scope**
- Every time the panel opens, the app asks `dellctl` for fresh status — readings are never stale.
- The panel shows model name, active input (as the checkmarked item in the input list), and brightness (the number beside the slider).
- If the monitor was changed with its own buttons, the panel reflects the new values the next time it opens.
- While reading, the input list and slider are shown disabled with a small spinner, so wrong values are never displayed.

**Out of scope**
- Live refresh while the panel stays open.
- Showing volume, speaker state, serial number, or other details.
- A manual "Refresh" button in the normal state (Retry appears only in problem states).

#### 3. One-click input switching

**In scope**
- An "Input" section listing every input the monitor supports, with readable labels: "Thunderbolt / USB-C", "DisplayPort", "HDMI". Unrecognized inputs show the name `dellctl` reports.
- A checkmark marks the active input.
- Clicking a different input switches to it immediately, with no confirmation; the panel closes.
- Clicking the already-active input does nothing.
- If the switch fails, the panel shows a short error (see Feature 5).
- Known behavior: switching away from this Mac sends the monitor to another source. If this monitor is the Mac's only display, switching back requires the monitor's own buttons or the other computer — same as the CLI.

**Out of scope**
- An "Are you sure?" confirmation.
- Per-input keyboard shortcuts.
- Cycling to the "next" input.
- Hiding unused inputs.

#### 4. Brightness slider

**In scope**
- A "Brightness" section with a horizontal slider (0–100), a dim-sun icon on the left, a bright-sun icon on the right, and the current number (e.g. "65").
- The number updates as you drag.
- The monitor follows the slider live while dragging. It may trail slightly behind (monitors are slow to respond), but it always ends on exactly the value where you let go.
- The panel stays open while adjusting.
- If setting brightness fails, the slider snaps back to the monitor's real value and a short error appears.

**Out of scope**
- Preset buttons (25 / 50 / 75 / 100).
- Contrast or other picture settings.
- Automatic or time-of-day brightness.
- Mapping the keyboard's brightness keys to the monitor.

#### 5. Friendly problem states

**In scope** — each shown as a plain message inside the panel:
- **`dellctl` missing or won't run:** "Couldn't run dellctl. Try reinstalling the app."
- **No monitor found:** "No Dell monitor detected", with a one-line hint to check that DDC/CI is enabled in the monitor's menu and that the cable or dock supports it.
- **Multiple monitors, no automatic choice:** "Multiple monitors found — not supported yet."
- **Brightness or input not supported on this monitor:** that section shows "Not available"; the rest of the panel still works.
- **A command fails or takes too long:** a short error line, using `dellctl`'s own message where it's readable.
- Every problem state has a **Retry** button, plus the Launch at login and Quit items, so the user is never stuck.
- The app never freezes; slow monitor responses only show the spinner.

**Out of scope**
- An error log or "copy diagnostics" feature.
- System notifications or alerts outside the panel.
- Guided setup or troubleshooting beyond the one-line hints.

#### 6. Bundled `dellctl`

**In scope**
- One build command, run from the repo, compiles the `dellctl` CLI, builds the menu bar app, and places the CLI inside it — producing a single `Monitor Control.app` to drag into Applications.
- The app always uses its own bundled copy of `dellctl`, so app and CLI versions always match.
- The app shows its version (the same version number as `dellctl`) at the bottom of the panel.
- README instructions for building, installing, and approving the unsigned app on first launch.

**Out of scope**
- Using a `dellctl` installed elsewhere on the Mac (e.g. on the terminal's PATH), or a setting to choose one.
- Putting the bundled `dellctl` on the terminal's PATH.
- A .dmg installer, Homebrew cask, or signed/notarized download.
- Intel Mac builds — Apple Silicon only, matching the CLI.

#### 7. Launch at login and Quit

**In scope**
- A footer at the bottom of the panel with a "Launch at login" checkbox and a "Quit Monitor Control" item.
- The checkbox registers the app as a standard macOS Login Item, so it also appears in System Settings › General › Login Items and can be managed there.
- The checkbox always reflects the actual macOS setting, even if it was changed in System Settings.
- Off by default on first run.
- Quit closes the app and removes the icon from the menu bar.

**Out of scope**
- A separate Settings window.
- Hiding the menu bar icon while the app keeps running.
- A first-run welcome screen.

---

## Milestone 1 — Menu bar shell & live status

Delivers a working `Monitor Control.app`, built by one command with `dellctl` bundled inside, that lives in the menu bar and shows the monitor's live status read-only.

### What gets built

- The Swift/SwiftUI app in its own folder in this repo, and one build command that compiles `dellctl`, builds the app, and produces `Monitor Control.app` with the CLI inside.
- The menu bar icon and panel (Feature 1): no Dock icon, no window, panel opens and closes like a system menu.
- Fresh status on every open (Feature 2): model name, the input list with a checkmark on the active input, and the brightness value with a slider — all **read-only** in this milestone.
- The loading state with a spinner and disabled controls.
- All problem states with Retry (Feature 5): `dellctl` missing, no monitor, multiple monitors, unsupported control, command failure or timeout.
- A Quit item in the panel footer.
- If the CLI's JSON output is missing anything the panel needs, the CLI is extended (not worked around in the app).

### What milestone 1 explicitly does NOT include

- Switching inputs or changing brightness — inputs and slider are display-only.
- The Launch at login checkbox.
- The version display and README updates.
- Volume or speaker controls.

### Done when

Running the one build command produces `Monitor Control.app`; launching it shows a monitor icon in the menu bar (and nothing in the Dock), and opening the panel shows the U3425WE's model, its active input checkmarked, and its current brightness. With `dellctl` removed from the bundle or the monitor's DDC/CI turned off, the panel shows the right friendly message and Retry works once the problem is fixed.

---

## Milestone 2 — Input switching & brightness

Makes the panel interactive: one-click input switching and a live brightness slider, both going through the bundled `dellctl`.

### What gets built

- Clicking a non-active input switches the monitor to it immediately and closes the panel (Feature 3); clicking the active one does nothing.
- Readable input labels ("Thunderbolt / USB-C", "DisplayPort", "HDMI"), falling back to `dellctl`'s name for anything unrecognized.
- The brightness slider becomes live (Feature 4): the number tracks the drag, the monitor follows while dragging, and the final value always lands exactly on release; the panel stays open.
- Failure handling for both: a short error in the panel; the slider snaps back to the monitor's real value on failure.
- The app stays responsive throughout — no freezes while the monitor is slow.

### What milestone 2 explicitly does NOT include

- Confirmation before switching inputs.
- Brightness presets, keyboard brightness keys, or contrast.
- The Launch at login checkbox, version display, or README updates.
- Volume or speaker controls.

### Done when

From the menu bar panel, the user can switch the U3425WE to another input with one click (and the panel closes), and drag the brightness slider to watch the monitor dim and brighten, ending exactly at the released value. Re-opening the panel shows the new state.

---

## Milestone 3 — Polish & ship

Finishes the app for everyday use: launch at login, version display, documentation, and a final pass over wording and edge cases.

### What gets built

- The "Launch at login" checkbox in the panel footer (Feature 7): registers a standard macOS Login Item, reflects changes made in System Settings, off by default.
- The app version (same number as `dellctl`) shown at the bottom of the panel.
- A README section covering: building `Monitor Control.app` with the one command, installing it into Applications, approving the unsigned app on first launch, and the "switching away from your only display" caveat.
- A final pass on all panel wording, error messages, light/dark appearance, and the edge cases from Features 1–7.

### What milestone 3 explicitly does NOT include

- Code-signing, notarization, .dmg, Homebrew cask, or auto-updates.
- Adding the app to CI or the GitHub release workflow.
- Volume or speaker controls, multiple-monitor support, global shortcuts.

### Done when

The user can build the app from the README instructions, drag it into Applications, approve it on first launch, turn on Launch at login (and see it in System Settings › Login Items), log out and back in to find the icon in the menu bar, and see the app version at the bottom of the panel.
