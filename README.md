# dellctl

Control a Dell monitor from the terminal: switch inputs, set brightness, and set or mute the built-in speakers. It runs on macOS (Apple Silicon), Windows 11, and Arch Linux.

On macOS there's also a [menu bar app](#menu-bar-app-macos), **Monitor Control**, for switching inputs and adjusting brightness without a terminal.

```console
$ dellctl
DELL U3425WE (monitor 1)
  Input:      thunderbolt
  Brightness: 78
  Volume:     88
  Speakers:   n/a

$ dellctl input set hdmi
Switched to hdmi

$ dellctl brightness +10
Brightness: 78 → 88
```

`dellctl` talks to the monitor over **DDC/CI**, a standard control channel that runs over the video cable (HDMI, DisplayPort, or USB-C/Thunderbolt). There's no daemon, no config file, and no account. Each command reads the monitor live, does one thing, and exits.

**Tested model:** Dell U3425WE (34" curved ultrawide). Other Dell monitors that expose the same DDC/CI controls should work but aren't promised. Non-Dell monitors may work too, since DDC/CI is a standard.

## Install

Download the archive for your platform from the [latest release](https://github.com/davemjones/dell-monitor-control/releases/latest):

| Platform | File |
|---|---|
| macOS (Apple Silicon) | `dellctl-vX.Y.Z-macos-arm64.tar.gz` |
| Windows 11 (x64) | `dellctl-vX.Y.Z-windows-x64.zip` |
| Linux (x64), e.g. Arch | `dellctl-vX.Y.Z-linux-x64.tar.gz` |

Each archive contains a single standalone `dellctl` binary (`dellctl.exe` on Windows). Put it somewhere on your `PATH`.

To check a download, compare it against `SHA256SUMS` from the same release:

```sh
shasum -a 256 -c SHA256SUMS --ignore-missing      # macOS
sha256sum -c SHA256SUMS --ignore-missing          # Linux
```

```powershell
Get-FileHash .\dellctl-vX.Y.Z-windows-x64.zip     # Windows: compare with SHA256SUMS
```

### macOS: "unidentified developer" warning

The macOS binary isn't signed or notarized, so the first run may be blocked. Remove the quarantine flag once:

```sh
xattr -d com.apple.quarantine ./dellctl
```

You can also allow it under **System Settings → Privacy & Security** after the first blocked attempt.

### Arch Linux setup

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

If either step is missing, `dellctl` says which one and prints these commands.

### Windows

No setup is needed. `dellctl.exe` uses the built-in Windows monitor-control interface and doesn't need the Visual C++ runtime.

### All platforms: enable DDC/CI on the monitor

DDC/CI is on by default on Dell monitors. If it's been turned off, open the monitor's on-screen menu and go to **Others → DDC/CI → On**.

## Usage

```
dellctl [--monitor <N|MODEL>] [--json] [COMMAND]
```

| Command | What it does |
|---|---|
| `dellctl` / `dellctl status` | Show input, brightness, volume, and speaker state |
| `dellctl list [--verbose]` | List detected monitors; `--verbose` adds supported controls |
| `dellctl input` | Show the active input |
| `dellctl input list` | List the monitor's inputs; the active one is marked `*` |
| `dellctl input set <name \| code>` | Switch input |
| `dellctl brightness [value \| +n \| -n]` | Show, set, or nudge brightness (0–100) |
| `dellctl volume [value \| +n \| -n]` | Show, set, or nudge speaker volume (0–100) |
| `dellctl speaker [on \| off \| toggle]` | Show or change whether the speakers are muted |

Every command has `--help`, and `dellctl --version` prints the version.

### Status

```console
$ dellctl status
DELL U3425WE (monitor 1)
  Input:      thunderbolt
  Brightness: 78
  Volume:     88
  Speakers:   n/a
```

Anything the monitor doesn't support over DDC/CI shows `n/a`. With several monitors connected and none selected, `status` shows all of them.

### Monitors

```console
$ dellctl list
1  DELL U3425WE  (serial ABC1234)

$ dellctl list --verbose
1  DELL U3425WE  (serial ABC1234)
   supports:      input, brightness, volume
   not supported: speaker mute
```

With exactly one monitor, or exactly one Dell, connected, every command targets it automatically. Otherwise pick one with `--monitor` (or `-m`), either by its number from `dellctl list` or by part of its model name:

```sh
dellctl --monitor 2 brightness 50
dellctl -m U3425 input set dp
```

If several monitors match and none was chosen, commands that change a setting stop and list the monitors instead of guessing.

### Inputs

```console
$ dellctl input
Input: thunderbolt [0x19]

$ dellctl input list
  dp           [0x0f]
  hdmi         [0x11]
* thunderbolt  [0x19]

$ dellctl input set hdmi
Switched to hdmi

$ dellctl input set usb-c
Already on thunderbolt
```

Input names are case-insensitive, and these aliases work:

| Input | Accepted names |
|---|---|
| USB-C / Thunderbolt | `thunderbolt`, `tb`, `usb-c`, `usbc` |
| DisplayPort | `dp`, `displayport`, `dp1` |
| HDMI | `hdmi`, `hdmi1` |

You can also use a raw code, such as `dellctl input set 0x11`. A name or code the monitor doesn't have prints the list of valid inputs.

### Brightness and volume

```console
$ dellctl brightness
Brightness: 78

$ dellctl brightness 60
Brightness: 78 → 60

$ dellctl brightness +10
Brightness: 60 → 70

$ dellctl volume -8
Volume: 88 → 80
```

Exact values must be 0–100. Nudges (`+n` / `-n`) stop at 0 and 100 without an error. If the speakers are muted, volume changes still apply, with a `(speakers are muted)` note. Changing the volume never unmutes the speakers.

### Speakers

```sh
dellctl speaker          # Speakers: on  /  Speakers: muted
dellctl speaker off      # mute
dellctl speaker on       # unmute
dellctl speaker toggle
```

Some monitors, including the U3425WE, don't support mute over DDC/CI. On those, `dellctl speaker` says so, and `dellctl volume 0` is the alternative.

### JSON output

Add `--json` to any command for machine-readable output:

```console
$ dellctl status --json
{
  "monitor": 1,
  "name": "DELL U3425WE",
  "input": {
    "name": "thunderbolt",
    "code": "0x19"
  },
  "inputs": [
    { "name": "dp", "code": "0x0f", "active": false },
    { "name": "hdmi", "code": "0x11", "active": false },
    { "name": "thunderbolt", "code": "0x19", "active": true }
  ],
  "brightness": 78,
  "volume": 88,
  "speakers": null
}

$ dellctl brightness +5 --json
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
- `input set` returns `{"before": …, "after": …, "changed": true|false}`.
- `speaker` changes return `{"speakers": {"before": …, "after": …}}`.
- `volume` changes also include `"speakers"`, so scripts can tell whether the speakers are muted.
- Errors go to stderr as `{"error": {"kind": …, "message": …}}`, with a non-zero exit code. `kind` is one of `no_monitor`, `multiple_monitors`, `unsupported`, or `failed`.

Errors go to stderr with exit code 1. Invalid arguments exit with code 2.

## Menu bar app (macOS)

**Monitor Control** puts a monitor icon in the macOS menu bar. Click it to see the monitor's active input and brightness, switch inputs with one click, and drag a slider to change brightness. It has no Dock icon or window. It does everything by running its own bundled copy of `dellctl`, so it behaves exactly like the command line.

### Build it

You need [Rust](https://rustup.rs) and Apple's Command Line Tools (`xcode-select --install`). The full Xcode app isn't needed. Then:

```sh
./macos-app/build.sh
```

This builds `dellctl` and the app, puts `dellctl` inside it, and leaves `Monitor Control.app` in `target/`.

### Install it

1. Drag `target/Monitor Control.app` into `/Applications` (or run `cp -R "target/Monitor Control.app" /Applications/`).
2. Open it from `/Applications`. The monitor icon appears in the menu bar.
3. To start it automatically, tick **Launch at login** at the bottom of its panel. It's added to System Settings › General › Login Items, where you can also turn it off. Turn it on from the copy in `/Applications`, since the login item remembers where the app was.

The app isn't signed by a registered developer. An app you build yourself opens normally. If you copy it from another Mac and macOS blocks it, right-click it and choose **Open**, or run `xattr -dr com.apple.quarantine "/Applications/Monitor Control.app"` (see [above](#macos-unidentified-developer-warning)).

### Good to know

- **Switching away from this Mac:** picking another input from the panel sends the monitor to that source right away. If this monitor is the Mac's only display, you won't see the menu bar until you switch back, so use the monitor's own buttons (see also [Known limitations](#known-limitations)).
- **One monitor:** the app controls the single Dell `dellctl` picks automatically. With several DDC/CI monitors connected it says so instead.
- **No volume controls** in the app yet; use `dellctl volume` and `dellctl speaker`.
- Readings are taken each time the panel opens, so changes made with the monitor's buttons show up the next time you open it.

To run the app's tests: `swift test --package-path macos-app`.

## Build from source

You need [Rust](https://rustup.rs) (stable). Then, on any platform:

```sh
git clone https://github.com/davemjones/dell-monitor-control.git
cd dell-monitor-control
cargo build --release
# the binary is target/release/dellctl (dellctl.exe on Windows)
```

Per-platform requirements:

- **macOS (Apple Silicon):** install Rust with rustup; no other setup.
- **Windows 11:** install Rust with rustup, plus the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the "Desktop development with C++" workload, which provides the MSVC linker. The project links the C runtime statically, so the resulting `.exe` runs on machines without the Visual C++ runtime.
- **Arch Linux:** `sudo pacman -S --needed rustup base-devel pkgconf`, then `rustup default stable`. The `libudev` it links against comes with `systemd`, which every Arch install has. Do the [I²C setup](#arch-linux-setup) above before running it.
- **Debian/Ubuntu (for building only):** `sudo apt install libudev-dev pkg-config`.

Run the tests with `cargo test`. They use a simulated monitor, so no hardware is needed.

## Known limitations

- **Docks, adapters, and KVM switches** can block DDC/CI. If `dellctl` can't find or talk to the monitor, try a direct cable.
- **Switching away from the computer running `dellctl`:** on the tested setup (MacBook Pro M1 Pro → U3425WE over Thunderbolt, no dock), the Mac can still reach the monitor while another input is showing, so `dellctl input set thunderbolt` switches back. Other setups may lose DDC/CI after switching away. If so, switch back with the monitor's buttons.
- **The U3425WE doesn't support speaker mute over DDC/CI.** Use `dellctl volume 0` instead.
- **The U3425WE keeps a separate brightness for each input.** `dellctl brightness` reads and sets the brightness of the input that's currently showing.
- **Windows and Linux** builds are tested automatically, but have not yet been tried against real monitor hardware. Only macOS has been verified end to end on the U3425WE. Reports are welcome.
- **macOS:** the binary is unsigned (see [above](#macos-unidentified-developer-warning)). Intel Macs, Windows on ARM, and Linux on ARM aren't supported.
- **The menu bar app is macOS-only** and isn't signed or notarized. There's no tray app for Windows or Linux.
- Only input, brightness, volume, and mute are supported. Contrast, color presets, KVM, and PbP/PiP aren't.

## Troubleshooting

| Message | What to do |
|---|---|
| `no DDC/CI-capable monitors found` | Turn on DDC/CI in the monitor's menu (Others → DDC/CI) and try a direct cable instead of a dock or adapter. On Windows, also update your graphics driver. |
| `…the i2c-dev kernel module probably isn't loaded` (Linux) | Do step 1 of the [Arch I²C setup](#arch-linux-setup). |
| `no permission to open /dev/i2c-N` (Linux) | Do steps 2–3 of the [Arch I²C setup](#arch-linux-setup). |
| `couldn't read … from DELL …` / `couldn't change …` | The monitor didn't answer, even after automatic retries. Check DDC/CI is on and try a direct cable. |
| `several monitors are connected; choose one with --monitor` | Add `--monitor <number or model>`; see `dellctl list`. |
| `doesn't support speaker mute over DDC/CI` | Use `dellctl volume 0`. |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
