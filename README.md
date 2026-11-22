# monitorctl

Control a monitor from the terminal: switch inputs, set brightness, and set or mute the built-in speakers. It runs on macOS (Apple Silicon), Windows 11, and Linux (x64 and ARM64; Arch users can install it from the AUR).

On macOS there's also an app, [**Monitor Control**](#monitor-control-app-macos), that puts brightness and input switching in Control Center, with Shortcuts actions too.

```console
$ monitorctl
DELL U3425WE (monitor 1)
  Input:      thunderbolt
  Brightness: 78
  Volume:     88
  Speakers:   n/a

$ monitorctl input set hdmi
Switched to hdmi

$ monitorctl brightness +10
Brightness: 78 → 88
```

`monitorctl` talks to the monitor over **DDC/CI**, a standard control channel that runs over the video cable (HDMI, DisplayPort, or USB-C/Thunderbolt). There's no daemon and no account, and the only file it keeps is an optional list of [your names for inputs](#naming-inputs). Each command reads the monitor live, does one thing, and exits.

**Tested model:** Dell U3425WE (34" curved ultrawide). Since DDC/CI is a standard, other monitors that expose the same controls should work too, but they aren't promised.

## Install

Download the archive for your platform from the [latest release](https://github.com/davemjones/dell-monitor-control/releases/latest):

| Platform | File |
|---|---|
| macOS (Apple Silicon) | `monitorctl-vX.Y.Z-macos-arm64.tar.gz` |
| Windows 11 (x64) | `monitorctl-vX.Y.Z-windows-x64.zip` |
| Linux (x64) | `monitorctl-vX.Y.Z-linux-x64.tar.gz` |
| Linux (ARM64) | `monitorctl-vX.Y.Z-linux-arm64.tar.gz` |
| Arch Linux (x64 or ARM64) | the [`monitorctl` AUR package](https://aur.archlinux.org/packages/monitorctl), see [below](#arch-linux-setup) |

The Linux binaries need glibc 2.35 or later (Ubuntu 22.04, Debian 12, Fedora 36, Arch, and newer). Each archive contains a folder with a single standalone `monitorctl` binary (`monitorctl.exe` on Windows), the README, and the licenses. Put the binary somewhere on your `PATH`. For the macOS app, see [Monitor Control app](#monitor-control-app-macos).

Every download has a matching `.sha256` file. To check a download, put both in the same folder and run:

```sh
shasum -a 256 -c monitorctl-vX.Y.Z-macos-arm64.tar.gz.sha256      # macOS
sha256sum -c monitorctl-vX.Y.Z-linux-x64.tar.gz.sha256            # Linux
```

```powershell
# Windows: the two hashes must match
(Get-FileHash .\monitorctl-vX.Y.Z-windows-x64.zip).Hash.ToLower()
Get-Content .\monitorctl-vX.Y.Z-windows-x64.zip.sha256
```

### macOS: "unidentified developer" warning

The macOS binary isn't signed or notarized, so the first run may be blocked. Remove the quarantine flag once:

```sh
xattr -d com.apple.quarantine ./monitorctl
```

You can also allow it under **System Settings → Privacy & Security** after the first blocked attempt.

### Arch Linux setup

On Arch, install the [`monitorctl`](https://aur.archlinux.org/packages/monitorctl) package from the AUR, with an AUR helper (`yay -S monitorctl`) or by hand:

```sh
git clone https://aur.archlinux.org/monitorctl.git
cd monitorctl
makepkg -si
```

It builds `monitorctl` from source and sets up I²C access for you: it loads the `i2c-dev` module at every boot and gives the logged-in user access to `/dev/i2c-*`. Run `sudo modprobe i2c-dev` once (or reboot) and `monitorctl` is ready. It can't be installed alongside the unrelated `monitorctl-bin` package, which installs a different program with the same name.

### Linux setup (without the AUR package)

On Linux, DDC/CI goes through the kernel's I²C devices (`/dev/i2c-*`), and you need access to them.

1. Load the `i2c-dev` module now and at every boot:

   ```sh
   sudo modprobe i2c-dev
   echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf
   ```

2. Give your user access to the I²C devices:

   ```sh
   sudo groupadd --system i2c
   sudo usermod -aG i2c $USER
   echo 'KERNEL=="i2c-[0-9]*", GROUP="i2c", MODE="0660"' | sudo tee /etc/udev/rules.d/45-i2c.rules
   ```

3. Log out and back in (or reboot) so the group membership takes effect.

If either step is missing, `monitorctl` says which one and prints these commands.

### Windows

No setup is needed. `monitorctl.exe` uses the built-in Windows monitor-control interface and doesn't need the Visual C++ runtime.

### All platforms: enable DDC/CI on the monitor

DDC/CI is on by default on most monitors. If it's been turned off, open the monitor's on-screen menu and turn it back on (on the U3425WE: **Others → DDC/CI → On**).

## Usage

```
monitorctl [--monitor <N|MODEL>] [--json] [COMMAND]
```

| Command | What it does |
|---|---|
| `monitorctl` / `monitorctl status` | Show input, brightness, volume, and speaker state |
| `monitorctl list [--verbose]` | List detected monitors; `--verbose` adds supported controls |
| `monitorctl input` | Show the active input |
| `monitorctl input list` | List the monitor's inputs; the active one is marked `*` |
| `monitorctl input set <name \| code>` | Switch input |
| `monitorctl input name <input> [name \| --clear]` | Show, set, or remove your own name for an input |
| `monitorctl brightness [value \| +n \| -n]` | Show, set, or nudge brightness (0–100) |
| `monitorctl volume [value \| +n \| -n]` | Show, set, or nudge speaker volume (0–100) |
| `monitorctl speaker [on \| off \| toggle]` | Show or change whether the speakers are muted |

Every command has `--help`, and `monitorctl --version` prints the version.

### Status

```console
$ monitorctl status
DELL U3425WE (monitor 1)
  Input:      thunderbolt
  Brightness: 78
  Volume:     88
  Speakers:   n/a
```

Anything the monitor doesn't support over DDC/CI shows `n/a`. With several monitors connected and none selected, `status` shows all of them.

### Monitors

```console
$ monitorctl list
1  DELL U3425WE  (serial ABC1234)

$ monitorctl list --verbose
1  DELL U3425WE  (serial ABC1234)
   supports:      input, brightness, volume
   not supported: speaker mute
```

With exactly one monitor connected, every command targets it automatically. Otherwise pick one with `--monitor` (or `-m`), either by its number from `monitorctl list` or by part of its model name:

```sh
monitorctl --monitor 2 brightness 50
monitorctl -m U3425 input set dp
```

If several monitors match and none was chosen, commands that change a setting stop and list the monitors instead of guessing.

### Inputs

```console
$ monitorctl input
Input: thunderbolt [0x19]

$ monitorctl input list
  dp           [0x0f]
  hdmi         [0x11]
* thunderbolt  [0x19]

$ monitorctl input set hdmi
Switched to hdmi

$ monitorctl input set usb-c
Already on thunderbolt
```

Input names are case-insensitive, and these aliases work:

| Input | Accepted names |
|---|---|
| USB-C / Thunderbolt | `thunderbolt`, `tb`, `usb-c`, `usbc` |
| DisplayPort | `dp`, `displayport`, `dp1` |
| HDMI | `hdmi`, `hdmi1` |

You can also use a raw code, such as `monitorctl input set 0x11`, or [your own name](#naming-inputs) for the input. A name or code the monitor doesn't have prints the list of valid inputs.

### Naming inputs

Give an input a name that says what's plugged into it. `input name` accepts anything `input set` does, including the input's current name:

```console
$ monitorctl input name hdmi "Gaming PC"
Named hdmi "Gaming PC"

$ monitorctl input name thunderbolt "Work Mac"
Named thunderbolt "Work Mac"

$ monitorctl input list
  dp           [0x0f]
  hdmi         [0x11]  Gaming PC
* thunderbolt  [0x19]  Work Mac

$ monitorctl input set "gaming pc"
Switched to hdmi (Gaming PC)

$ monitorctl input name hdmi
hdmi: Gaming PC

$ monitorctl input name hdmi --clear
Cleared the name for hdmi
```

Names show up in `status` (`Input:      thunderbolt (Work Mac)`), `input`, `input list`, and in the [Monitor Control app](#monitor-control-app-macos), where you can also rename inputs. They're case-insensitive when you switch, up to 20 characters, and can't be something `input set` already understands (like `hdmi`, `tb`, or `17`) or another input's name.

Names belong to the monitor model, and are kept in a small JSON file:

| OS | File |
|---|---|
| macOS | `~/Library/Application Support/monitorctl/names.json` |
| Linux | `$XDG_CONFIG_HOME/monitorctl/names.json` (usually `~/.config/monitorctl/names.json`) |
| Windows | `%APPDATA%\monitorctl\names.json` |

Set `MONITORCTL_NAMES_FILE` to use a different file. You can edit the file by hand:

```json
{
  "version": 1,
  "monitors": {
    "DELL U3425WE": {
      "inputs": { "0x11": "Gaming PC", "0x19": "Work Mac" }
    }
  }
}
```

### Brightness and volume

```console
$ monitorctl brightness
Brightness: 78

$ monitorctl brightness 60
Brightness: 78 → 60

$ monitorctl brightness +10
Brightness: 60 → 70

$ monitorctl volume -8
Volume: 88 → 80
```

Exact values must be 0–100. Nudges (`+n` / `-n`) stop at 0 and 100 without an error. If the speakers are muted, volume changes still apply, with a `(speakers are muted)` note. Changing the volume never unmutes the speakers.

### Speakers

```sh
monitorctl speaker          # Speakers: on  /  Speakers: muted
monitorctl speaker off      # mute
monitorctl speaker on       # unmute
monitorctl speaker toggle
```

Some monitors, including the U3425WE, don't support mute over DDC/CI. On those, `monitorctl speaker` says so, and `monitorctl volume 0` is the alternative.

### JSON output

Add `--json` to any command for machine-readable output:

```console
$ monitorctl status --json
{
  "monitor": 1,
  "name": "DELL U3425WE",
  "input": {
    "name": "thunderbolt",
    "code": "0x19",
    "label": "Work Mac"
  },
  "inputs": [
    { "name": "dp", "code": "0x0f", "label": null, "active": false },
    { "name": "hdmi", "code": "0x11", "label": "Gaming PC", "active": false },
    { "name": "thunderbolt", "code": "0x19", "label": "Work Mac", "active": true }
  ],
  "brightness": 78,
  "volume": 88,
  "speakers": null
}

$ monitorctl brightness +5 --json
{
  "brightness": {
    "before": 78,
    "after": 83
  }
}
```

The output shapes:

- `status --json` returns an object for one monitor and an array when it shows several. It also lists the supported inputs under `"inputs"` (`null` if the monitor doesn't report them), which the text output skips because reading them takes about a second.
- `list` and `input list` always return arrays.
- Every input object has a `"label"`: your name for it, or `null`.
- `input set` returns `{"before": …, "after": …, "changed": true|false}`.
- `input name` returns `{"input": …}`, plus `"label": {"before": …, "after": …}` when it sets or clears a name.
- `speaker` changes return `{"speakers": {"before": …, "after": …}}`.
- `volume` changes also include `"speakers"`, so scripts can tell whether the speakers are muted.
- Errors go to stderr as `{"error": {"kind": …, "message": …}}`, with a non-zero exit code. `kind` is one of `no_monitor`, `multiple_monitors`, `unsupported`, or `failed`.

Errors go to stderr with exit code 1. Invalid arguments exit with code 2.

## Monitor Control app (macOS)

**Monitor Control** puts your monitor's brightness and inputs in macOS **Control Center**, so you don't need a terminal. It needs **macOS 27 or later** on Apple Silicon. It has no Dock icon, and it does everything by running its own bundled copy of `monitorctl`, so it behaves exactly like the command line.

### Install it

1. Download `Monitor-Control-vX.Y.Z-macos-arm64.zip` from the [latest release](https://github.com/davemjones/dell-monitor-control/releases/latest), unzip it, and move **Monitor Control** into `/Applications`.
2. Open it. The app isn't notarized by Apple yet, so the first time macOS blocks it: open **System Settings › Privacy & Security**, scroll down to the message about Monitor Control, and click **Open Anyway**.
3. Its **Settings** window opens. Opening the app again at any time (from Applications or Spotlight) brings Settings back.

### Add the controls

Click **Control Center** in the menu bar, choose **Edit Controls**, search for **Monitor Control**, and add the controls you want. You can also drag a control into the menu bar.

| Control | What it does |
|---|---|
| **Monitor Brightness** | Opens a brightness slider at the top right of the screen |
| **Monitor Input** | Opens the monitor's inputs (with your names for them); click one to switch |
| **Monitor Control Settings** | Opens Settings |

The panels close when you press Esc or click elsewhere. Control Center only offers buttons and switches, which is why brightness opens a slider rather than sitting in Control Center itself. If no monitor is connected, the panel says **No monitor detected**.

### Settings

- **Input Names:** type a name next to each input and press Return (or click away). Clear the field to go back to the built-in name. These are the same names `monitorctl input name` sets, and they appear in the controls, the menu bar panel, and Shortcuts.
- **Show menu bar icon** (off by default): adds the original menu bar panel, with the input list (right-click an input to rename it), the brightness slider, **Settings…** (⌘,), and Quit.
- **Launch at login:** keeps the controls quick to respond; otherwise the first click after logging in starts the app. You can also manage it in System Settings › General › Login Items.
- **Quit Monitor Control**, since with the icon off there's no menu to quit from.

### Shortcuts

The Shortcuts app has two Monitor Control actions, ready to use: **Switch Monitor Input** (choose an input by name) and **Set Monitor Brightness** (0–100). Use them in your own shortcuts or bind them to keyboard shortcuts. Siri and Spotlight phrases aren't supported yet.

### Good to know

- **Switching away from this Mac:** picking another input sends the monitor to that source right away. If this monitor is the Mac's only display, you won't see anything until you switch back, so use the monitor's own buttons (see also [Known limitations](#known-limitations)).
- **One monitor:** the app controls the single monitor `monitorctl` picks automatically. With several DDC/CI monitors connected it says so instead.
- **No volume controls** in the app yet; use `monitorctl volume` and `monitorctl speaker`.
- Readings are taken when the app starts and again each time a panel or Settings opens. Panels show the last readings straight away and update them in place, so changes made with the monitor's buttons appear a moment after you open them.

### Build it

You need [Rust](https://rustup.rs) and Xcode 27.2 or later, signed in with an Apple ID (Xcode › Settings › Accounts; a free Personal Team is enough).

1. Copy `macos-app/Config/Signing.local.xcconfig.example` to `macos-app/Config/Signing.local.xcconfig` and set your Team ID: the 10-character code next to your team in Xcode › Settings › Accounts. That file is git-ignored.
2. Build:
   ```sh
   ./macos-app/build.sh
   ```

This builds `monitorctl`, the app, and its Control Center controls with Xcode, puts `monitorctl` inside the app, signs everything with your team, and leaves `Monitor Control.app` in `target/`. Move it to `/Applications` before adding the controls. You can also open `macos-app/MonitorControl.xcodeproj` in Xcode, after running `cargo build --release` once so there's a `monitorctl` to bundle.

To run the app's tests: `swift test --package-path macos-app`.

## Build from source

You need [Rust](https://rustup.rs) (stable). Then, on any platform:

```sh
git clone https://github.com/davemjones/dell-monitor-control.git
cd dell-monitor-control
cargo build --release
# the binary is target/release/monitorctl (monitorctl.exe on Windows)
```

Per-platform requirements:

- **macOS (Apple Silicon):** install Rust with rustup; no other setup.
- **Windows 11:** install Rust with rustup, plus the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload, which provides the MSVC linker. The project links the C runtime statically, so the resulting `.exe` runs on machines without the Visual C++ runtime.
- **Arch Linux:** `sudo pacman -S --needed rustup base-devel pkgconf`, then `rustup default stable`. The `libudev` it links against comes with `systemd`, which every Arch install has. Do the [I²C setup](#linux-setup-without-the-aur-package) above before running it.
- **Debian/Ubuntu (for building only):** `sudo apt install libudev-dev pkg-config`.

Run the tests with `cargo test`. They use a simulated monitor, so no hardware is needed.

## Known limitations

- **Docks, adapters, and KVM switches** can block DDC/CI. If `monitorctl` can't find or talk to the monitor, try a direct cable.
- **Switching away from the computer running `monitorctl`:** on the tested setup (MacBook Pro M1 Pro → U3425WE over Thunderbolt, no dock), the Mac can still reach the monitor while another input is showing, so `monitorctl input set thunderbolt` switches back. Other setups may lose DDC/CI after switching away. If so, switch back with the monitor's buttons.
- **The U3425WE doesn't support speaker mute over DDC/CI.** Use `monitorctl volume 0` instead.
- **Input names belong to a monitor model**, so two identical monitors share the same names.
- **The U3425WE keeps a separate brightness for each input.** `monitorctl brightness` reads and sets the brightness of the input that's currently showing.
- **Windows and Linux** builds pass the test suite against a simulated monitor, but have not yet been tried against real monitor hardware. Only macOS has been verified end to end on the U3425WE. Reports are welcome.
- **macOS:** the binary is unsigned (see [above](#macos-unidentified-developer-warning)). Intel Macs and Windows on ARM aren't supported. Linux on ARM64 is built but hasn't been tried on real hardware yet.
- **The Monitor Control app is macOS-only** (macOS 27 or later) and isn't notarized yet, so macOS asks you to approve it the first time (Privacy & Security › Open Anyway). There's no tray app for Windows or Linux.
- **The app works with one monitor.** With several DDC/CI monitors connected it says so; use `monitorctl --monitor` for the others.
- Only input, brightness, volume, and mute are supported. Contrast, color presets, KVM, and PbP/PiP aren't.

## Troubleshooting

| Message | What to do |
|---|---|
| `no DDC/CI-capable monitors found` | Turn on DDC/CI in the monitor's menu (Others → DDC/CI) and try a direct cable instead of a dock or adapter. On Windows, also update your graphics driver. |
| `…the i2c-dev kernel module probably isn't loaded` (Linux) | Install the [AUR package](#arch-linux-setup) and run `sudo modprobe i2c-dev`, or do step 1 of the [manual I²C setup](#linux-setup-without-the-aur-package). |
| `no permission to open /dev/i2c-N` (Linux) | Do steps 2–3 of the [manual I²C setup](#linux-setup-without-the-aur-package) (the AUR package does this for you). |
| `couldn't read … from <monitor>` / `couldn't change …` | The monitor didn't answer, even after automatic retries. Check DDC/CI is on and try a direct cable. |
| `several monitors are connected; choose one with --monitor` | Add `--monitor <number or model>`; see `monitorctl list`. |
| `doesn't support speaker mute over DDC/CI` | Use `monitorctl volume 0`. |
| `… isn't a valid names file` | Fix the JSON in the [names file](#naming-inputs), or delete it to start over. Status and switching by built-in name keep working meanwhile. |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
