//! Command logic: talk to the selected monitor(s) and produce output text.

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::backend::{Backend, Caps, Monitor};
use crate::cli::{Cli, Command, InputAction, Level, SpeakerState};
use crate::output::{self, Controls, InputEntry, InputRef, ListEntry, Status};
use crate::select::{self, Indexed};
use crate::vcp;

pub fn run(cli: &Cli, backend: &dyn Backend) -> Result<String> {
    let wanted = cli.monitor.as_deref();
    let command = cli.command.as_ref().unwrap_or(&Command::Status);

    match command {
        Command::Status => {
            let statuses: Vec<Status> = select::all_or_one(backend.enumerate()?, wanted)?
                .iter_mut()
                .map(read_status)
                .collect();
            Ok(match (cli.json, &statuses[..]) {
                (true, [single]) => output::json(single),
                (true, _) => output::json(&statuses),
                (false, _) => output::status_text(&statuses),
            })
        }

        Command::List { verbose } => {
            let monitors = select::index(backend.enumerate()?);
            if monitors.is_empty() {
                // Reuse the selector's "nothing found" message and hint.
                select::one(vec![], None)?;
            }
            let entries: Vec<ListEntry> = monitors
                .into_iter()
                .map(|mut m| ListEntry {
                    monitor: m.index,
                    name: m.monitor.info().name.clone(),
                    serial: m.monitor.info().serial.clone(),
                    controls: verbose
                        .then(|| m.monitor.capabilities().ok().map(|c| controls(&c)))
                        .flatten(),
                })
                .collect();
            Ok(if cli.json {
                output::json(&entries)
            } else {
                output::list_text(&entries, *verbose)
            })
        }

        Command::Input { action: None } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let input = read_input(m.monitor.as_mut())?;
            Ok(if cli.json {
                output::json(&input)
            } else {
                format!("Input: {} [{}]", input.name, input.code)
            })
        }

        Command::Input {
            action: Some(InputAction::List),
        } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let caps = m.monitor.capabilities()?;
            let codes = caps.features.get(&vcp::INPUT).filter(|c| !c.is_empty());
            let Some(codes) = codes else {
                bail!(
                    "{} doesn't report its inputs over DDC/CI",
                    m.monitor.info().name
                );
            };
            let active = read_input(m.monitor.as_mut()).ok();
            let entries: Vec<InputEntry> = codes
                .iter()
                .map(|&code| {
                    let input = InputRef::new(code);
                    InputEntry {
                        active: active.as_ref() == Some(&input),
                        input,
                    }
                })
                .collect();
            Ok(if cli.json {
                output::json(&entries)
            } else {
                output::inputs_text(&entries)
            })
        }

        Command::Brightness { value: None } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let level = read_percent(m.monitor.as_mut(), vcp::BRIGHTNESS, "brightness")?;
            Ok(if cli.json {
                output::json(&json!({ "brightness": level }))
            } else {
                format!("Brightness: {level}")
            })
        }

        Command::Volume { value: None } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let level = read_percent(m.monitor.as_mut(), vcp::VOLUME, "volume")?;
            Ok(if cli.json {
                output::json(&json!({ "volume": level }))
            } else {
                format!("Volume: {level}")
            })
        }

        Command::Speaker { state: None } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let Some(state) = read_speakers(m.monitor.as_mut()) else {
                bail!(mute_unsupported(m.monitor.as_ref()));
            };
            Ok(if cli.json {
                output::json(&json!({ "speakers": state }))
            } else {
                format!("Speakers: {state}")
            })
        }

        Command::Input {
            action: Some(InputAction::Set { input }),
        } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let monitor = m.monitor.as_mut();
            // Unreadable capabilities only skip the "does it have that
            // input" check; the switch itself can still work.
            let supported = monitor
                .capabilities()
                .ok()
                .and_then(|caps| caps.features.get(&vcp::INPUT).cloned())
                .filter(|codes| !codes.is_empty());
            let code = vcp::parse_input(input)
                .filter(|code| supported.as_ref().is_none_or(|s| s.contains(code)));
            let Some(code) = code else {
                bail!(unknown_input(input, monitor, supported.as_deref()));
            };

            let before = read_input(monitor)?;
            let after = InputRef::new(code);
            let changed = before != after;
            if changed {
                // No read-back: once the monitor shows another computer's
                // input, this one may no longer be able to talk to it.
                monitor.write_vcp(vcp::INPUT, code.into())?;
            }
            Ok(if cli.json {
                output::json(&json!({ "before": before, "after": after, "changed": changed }))
            } else if changed {
                format!("Switched to {}", after.name)
            } else {
                format!("Already on {}", after.name)
            })
        }

        Command::Brightness { value: Some(level) } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let (before, after) =
                change_level(m.monitor.as_mut(), vcp::BRIGHTNESS, "brightness", *level)?;
            Ok(if cli.json {
                output::json(&json!({ "brightness": { "before": before, "after": after } }))
            } else {
                format!("Brightness: {before} → {after}")
            })
        }

        Command::Volume { value: Some(level) } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let monitor = m.monitor.as_mut();
            let (before, after) = change_level(monitor, vcp::VOLUME, "volume", *level)?;
            // Volume changes never touch mute; just say so when it matters.
            let speakers = read_speakers(monitor);
            Ok(if cli.json {
                output::json(&json!({
                    "volume": { "before": before, "after": after },
                    "speakers": speakers,
                }))
            } else if speakers == Some("muted") {
                format!("Volume: {before} → {after}  (speakers are muted)")
            } else {
                format!("Volume: {before} → {after}")
            })
        }

        Command::Speaker { state: Some(want) } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let monitor = m.monitor.as_mut();
            let Some(before) = read_speakers(monitor) else {
                bail!(mute_unsupported(monitor));
            };
            let was_muted = before == "muted";
            let mute = match want {
                SpeakerState::On => false,
                SpeakerState::Off => true,
                SpeakerState::Toggle => !was_muted,
            };
            let after = if mute { "muted" } else { "on" };
            if mute != was_muted {
                monitor.write_vcp(vcp::MUTE, vcp::mute_value(mute))?;
            }
            Ok(if cli.json {
                output::json(&json!({ "speakers": { "before": before, "after": after } }))
            } else if mute == was_muted {
                format!("Speakers: already {after}")
            } else {
                format!("Speakers: {before} → {after}")
            })
        }
    }
}

/// Set or nudge a 0–100 setting; returns the level before and after.
fn change_level(
    monitor: &mut dyn Monitor,
    code: u8,
    what: &str,
    level: Level,
) -> Result<(u16, u16)> {
    let raw = monitor
        .read_vcp(code)
        .with_context(|| format!("couldn't read {what}, so it wasn't changed"))?;
    let before = vcp::percent(raw.value, raw.max);
    let after = vcp::apply_level(before, level);
    monitor.write_vcp(code, vcp::from_percent(after, raw.max))?;
    Ok((before, after))
}

fn mute_unsupported(monitor: &dyn Monitor) -> String {
    format!(
        "{} doesn't support speaker mute over DDC/CI; try `dellctl volume 0` instead",
        monitor.info().name
    )
}

fn unknown_input(arg: &str, monitor: &dyn Monitor, supported: Option<&[u8]>) -> String {
    let valid = match supported {
        Some(codes) => {
            let entries: Vec<InputEntry> = codes
                .iter()
                .map(|&code| InputEntry {
                    input: InputRef::new(code),
                    active: false,
                })
                .collect();
            format!(
                "valid inputs on {}:\n{}",
                monitor.info().name,
                output::inputs_text(&entries)
            )
        }
        None => "known names: dp, hdmi, thunderbolt (or usb-c), dvi, vga, or a raw code like 0x0f"
            .into(),
    };
    format!("unknown input '{arg}'; {valid}")
}

/// Read everything `status` shows; anything unreadable becomes `n/a`.
fn read_status(m: &mut Indexed) -> Status {
    let monitor = m.monitor.as_mut();
    Status {
        monitor: m.index,
        name: monitor.info().name.clone(),
        input: read_input(monitor).ok(),
        brightness: read_percent(monitor, vcp::BRIGHTNESS, "brightness").ok(),
        volume: read_percent(monitor, vcp::VOLUME, "volume").ok(),
        speakers: read_speakers(monitor),
    }
}

fn read_input(monitor: &mut dyn Monitor) -> Result<InputRef> {
    let raw = monitor
        .read_vcp(vcp::INPUT)
        .context("couldn't read the active input")?;
    Ok(InputRef::new(vcp::current_input(raw.value)))
}

fn read_percent(monitor: &mut dyn Monitor, code: u8, what: &str) -> Result<u16> {
    let raw = monitor
        .read_vcp(code)
        .with_context(|| format!("couldn't read {what}"))?;
    Ok(vcp::percent(raw.value, raw.max))
}

fn read_speakers(monitor: &mut dyn Monitor) -> Option<&'static str> {
    let raw = monitor.read_vcp(vcp::MUTE).ok()?;
    vcp::muted(raw.value).map(|muted| if muted { "muted" } else { "on" })
}

fn controls(caps: &Caps) -> Controls {
    Controls {
        input: caps.supports(vcp::INPUT),
        brightness: caps.supports(vcp::BRIGHTNESS),
        volume: caps.supports(vcp::VOLUME),
        mute: caps.supports(vcp::MUTE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::VcpValue;
    use crate::backend::fake::{FakeBackend, FakeMonitor, WriteLog};
    use clap::Parser;

    fn run_with(monitors: Vec<FakeMonitor>, args: &[&str]) -> Result<String> {
        let cli =
            Cli::try_parse_from(std::iter::once("dellctl").chain(args.iter().copied())).unwrap();
        run(&cli, &FakeBackend(monitors))
    }

    fn dell() -> Vec<FakeMonitor> {
        vec![FakeMonitor::u3425we()]
    }

    #[test]
    fn status_text_matches_prd_layout() {
        let out = run_with(dell(), &[]).unwrap();
        assert_eq!(
            out,
            "DELL U3425WE (monitor 1)\n  Input:      thunderbolt\n  Brightness: 78\n  Volume:     88\n  Speakers:   n/a"
        );
        assert_eq!(run_with(dell(), &["status"]).unwrap(), out);
    }

    #[test]
    fn status_json_single_monitor_is_an_object() {
        let out: serde_json::Value =
            serde_json::from_str(&run_with(dell(), &["status", "--json"]).unwrap()).unwrap();
        assert_eq!(
            out,
            json!({
                "monitor": 1,
                "name": "DELL U3425WE",
                "input": { "name": "thunderbolt", "code": "0x19" },
                "brightness": 78,
                "volume": 88,
                "speakers": null
            })
        );
    }

    #[test]
    fn status_shows_every_monitor_when_unselected() {
        let out = run_with(
            vec![FakeMonitor::named("LG 27UK850"), FakeMonitor::u3425we()],
            &[],
        )
        .unwrap();
        assert!(
            out.starts_with("LG 27UK850 (monitor 1)\n  Input:      n/a"),
            "{out}"
        );
        assert!(out.contains("\n\nDELL U3425WE (monitor 2)"), "{out}");
        let json = run_with(
            vec![FakeMonitor::named("LG"), FakeMonitor::u3425we()],
            &["--json"],
        )
        .unwrap();
        assert!(
            serde_json::from_str::<serde_json::Value>(&json)
                .unwrap()
                .is_array()
        );
    }

    #[test]
    fn list_and_verbose() {
        assert_eq!(
            run_with(dell(), &["list"]).unwrap(),
            "1  DELL U3425WE  (serial JK4HV84)"
        );
        assert_eq!(
            run_with(dell(), &["list", "--verbose"]).unwrap(),
            "1  DELL U3425WE  (serial JK4HV84)\n   supports:      input, brightness, volume\n   not supported: speaker mute"
        );
        let json: serde_json::Value =
            serde_json::from_str(&run_with(dell(), &["list", "-v", "--json"]).unwrap()).unwrap();
        assert_eq!(
            json[0]["controls"],
            json!({ "input": true, "brightness": true, "volume": true, "mute": false })
        );
        assert!(
            run_with(dell(), &["list", "--json"])
                .unwrap()
                .find("controls")
                .is_none()
        );
    }

    #[test]
    fn list_verbose_without_caps() {
        let out = run_with(vec![FakeMonitor::named("Mystery")], &["list", "-v"]).unwrap();
        assert_eq!(
            out,
            "1  Mystery\n   supports:      unknown (couldn't read its capabilities)"
        );
    }

    #[test]
    fn list_with_no_monitors_errors_with_hint() {
        assert!(
            run_with(vec![], &["list"])
                .unwrap_err()
                .to_string()
                .contains("hint:")
        );
    }

    #[test]
    fn input_active_and_list() {
        assert_eq!(
            run_with(dell(), &["input"]).unwrap(),
            "Input: thunderbolt [0x19]"
        );
        assert_eq!(
            run_with(dell(), &["input", "list"]).unwrap(),
            "  dp           [0x0f]\n  hdmi         [0x11]\n* thunderbolt  [0x19]"
        );
        let json: serde_json::Value =
            serde_json::from_str(&run_with(dell(), &["input", "list", "--json"]).unwrap()).unwrap();
        assert_eq!(
            json[2],
            json!({ "name": "thunderbolt", "code": "0x19", "active": true })
        );
    }

    #[test]
    fn single_reads() {
        assert_eq!(run_with(dell(), &["brightness"]).unwrap(), "Brightness: 78");
        assert_eq!(run_with(dell(), &["volume"]).unwrap(), "Volume: 88");
        assert_eq!(
            run_with(dell(), &["brightness", "--json"]).unwrap(),
            "{\n  \"brightness\": 78\n}"
        );
    }

    #[test]
    fn speaker_state_and_unsupported() {
        let err = run_with(dell(), &["speaker"]).unwrap_err().to_string();
        assert!(
            err.contains("doesn't support speaker mute") && err.contains("dellctl volume 0"),
            "{err}"
        );

        let mut muted = FakeMonitor::u3425we();
        muted
            .values
            .insert(vcp::MUTE, VcpValue { value: 1, max: 2 });
        assert_eq!(
            run_with(vec![muted.clone()], &["speaker"]).unwrap(),
            "Speakers: muted"
        );
        muted
            .values
            .insert(vcp::MUTE, VcpValue { value: 2, max: 2 });
        assert_eq!(run_with(vec![muted], &["speaker"]).unwrap(), "Speakers: on");
    }

    /// A U3425WE whose writes the test can inspect afterwards.
    fn watched(m: FakeMonitor) -> (Vec<FakeMonitor>, WriteLog) {
        let writes = m.writes.clone();
        (vec![m], writes)
    }

    fn with_mute(value: u16) -> FakeMonitor {
        let mut m = FakeMonitor::u3425we();
        m.values.insert(vcp::MUTE, VcpValue { value, max: 2 });
        m
    }

    #[test]
    fn brightness_set_and_nudge() {
        let (dell, writes) = watched(FakeMonitor::u3425we());
        assert_eq!(
            run_with(dell.clone(), &["brightness", "40"]).unwrap(),
            "Brightness: 78 → 40"
        );
        assert_eq!(
            run_with(dell.clone(), &["brightness", "+10"]).unwrap(),
            "Brightness: 78 → 88"
        );
        assert_eq!(
            run_with(dell.clone(), &["brightness", "-10"]).unwrap(),
            "Brightness: 78 → 68"
        );
        assert_eq!(
            run_with(dell.clone(), &["brightness", "+50"]).unwrap(),
            "Brightness: 78 → 100"
        );
        assert_eq!(
            run_with(dell, &["brightness", "-100"]).unwrap(),
            "Brightness: 78 → 0"
        );
        assert_eq!(
            *writes.borrow(),
            vec![(0x10, 40), (0x10, 88), (0x10, 68), (0x10, 100), (0x10, 0)]
        );
    }

    #[test]
    fn brightness_json_and_other_scales() {
        let out: serde_json::Value =
            serde_json::from_str(&run_with(dell(), &["brightness", "+5", "--json"]).unwrap())
                .unwrap();
        assert_eq!(out, json!({ "brightness": { "before": 78, "after": 83 } }));

        let mut m = FakeMonitor::u3425we();
        m.values.insert(
            vcp::BRIGHTNESS,
            VcpValue {
                value: 51,
                max: 255,
            },
        );
        let (list, writes) = watched(m);
        assert_eq!(
            run_with(list, &["brightness", "+10"]).unwrap(),
            "Brightness: 20 → 30"
        );
        assert_eq!(*writes.borrow(), vec![(0x10, 77)]);
    }

    #[test]
    fn volume_changes_and_never_unmutes() {
        let (list, writes) = watched(FakeMonitor::u3425we());
        assert_eq!(
            run_with(list, &["volume", "-8"]).unwrap(),
            "Volume: 88 → 80"
        );
        assert_eq!(*writes.borrow(), vec![(0x62, 80)]);

        let (muted, writes) = watched(with_mute(1));
        assert_eq!(
            run_with(muted.clone(), &["volume", "50"]).unwrap(),
            "Volume: 88 → 50  (speakers are muted)"
        );
        let out: serde_json::Value =
            serde_json::from_str(&run_with(muted, &["volume", "+5", "--json"]).unwrap()).unwrap();
        assert_eq!(
            out,
            json!({ "volume": { "before": 88, "after": 93 }, "speakers": "muted" })
        );
        assert!(writes.borrow().iter().all(|(code, _)| *code == vcp::VOLUME));

        assert_eq!(
            run_with(vec![with_mute(2)], &["volume", "50"]).unwrap(),
            "Volume: 88 → 50"
        );
    }

    #[test]
    fn unreadable_level_settings_are_not_changed() {
        let err = format!(
            "{:#}",
            run_with(vec![FakeMonitor::named("DELL P2419H")], &["volume", "50"]).unwrap_err()
        );
        assert_eq!(
            err,
            "couldn't read volume, so it wasn't changed: couldn't read setting 0x62 from DELL P2419H"
        );
    }

    #[test]
    fn speaker_on_off_toggle() {
        let (muted, writes) = watched(with_mute(1));
        assert_eq!(
            run_with(muted.clone(), &["speaker", "on"]).unwrap(),
            "Speakers: muted → on"
        );
        assert_eq!(
            run_with(muted.clone(), &["speaker", "toggle"]).unwrap(),
            "Speakers: muted → on"
        );
        assert_eq!(
            run_with(muted, &["speaker", "off"]).unwrap(),
            "Speakers: already muted"
        );
        assert_eq!(*writes.borrow(), vec![(0x8D, 2), (0x8D, 2)]);

        let (on, writes) = watched(with_mute(2));
        assert_eq!(
            run_with(on.clone(), &["speaker", "toggle"]).unwrap(),
            "Speakers: on → muted"
        );
        let out: serde_json::Value =
            serde_json::from_str(&run_with(on, &["speaker", "off", "--json"]).unwrap()).unwrap();
        assert_eq!(
            out,
            json!({ "speakers": { "before": "on", "after": "muted" } })
        );
        assert_eq!(*writes.borrow(), vec![(0x8D, 1), (0x8D, 1)]);
    }

    #[test]
    fn speaker_change_unsupported_suggests_volume() {
        let (dell, writes) = watched(FakeMonitor::u3425we());
        for state in ["on", "off", "toggle"] {
            let err = run_with(dell.clone(), &["speaker", state])
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("doesn't support speaker mute") && err.contains("dellctl volume 0"),
                "{err}"
            );
        }
        assert!(writes.borrow().is_empty());
    }

    #[test]
    fn input_set_by_name_alias_and_code() {
        let (dell, writes) = watched(FakeMonitor::u3425we());
        for (arg, expected) in [
            ("hdmi", "Switched to hdmi"),
            ("HDMI1", "Switched to hdmi"),
            ("dp", "Switched to dp"),
            ("DisplayPort", "Switched to dp"),
            ("0x11", "Switched to hdmi"),
            ("17", "Switched to hdmi"),
        ] {
            assert_eq!(
                run_with(dell.clone(), &["input", "set", arg]).unwrap(),
                expected,
                "{arg}"
            );
        }
        assert_eq!(
            *writes.borrow(),
            vec![
                (0x60, 0x11),
                (0x60, 0x11),
                (0x60, 0x0F),
                (0x60, 0x0F),
                (0x60, 0x11),
                (0x60, 0x11)
            ]
        );
    }

    #[test]
    fn input_set_active_is_a_no_op() {
        let (dell, writes) = watched(FakeMonitor::u3425we());
        for arg in ["thunderbolt", "usb-c", "usbc", "TB", "0x19"] {
            assert_eq!(
                run_with(dell.clone(), &["input", "set", arg]).unwrap(),
                "Already on thunderbolt",
                "{arg}"
            );
        }
        let out: serde_json::Value =
            serde_json::from_str(&run_with(dell, &["input", "set", "tb", "--json"]).unwrap())
                .unwrap();
        assert_eq!(out["changed"], json!(false));
        assert!(writes.borrow().is_empty());
    }

    #[test]
    fn input_set_json() {
        let out: serde_json::Value =
            serde_json::from_str(&run_with(dell(), &["input", "set", "hdmi", "--json"]).unwrap())
                .unwrap();
        assert_eq!(
            out,
            json!({
                "before": { "name": "thunderbolt", "code": "0x19" },
                "after": { "name": "hdmi", "code": "0x11" },
                "changed": true
            })
        );
    }

    #[test]
    fn input_set_rejects_what_the_monitor_lacks() {
        let (dell, writes) = watched(FakeMonitor::u3425we());
        for arg in ["vga", "0x12", "hdmi2", "bogus"] {
            let err = run_with(dell.clone(), &["input", "set", arg])
                .unwrap_err()
                .to_string();
            assert_eq!(
                err,
                format!(
                    "unknown input '{arg}'; valid inputs on DELL U3425WE:\n  dp           [0x0f]\n  hdmi         [0x11]\n  thunderbolt  [0x19]"
                )
            );
        }
        assert!(writes.borrow().is_empty());
    }

    #[test]
    fn input_set_without_caps_trusts_known_names() {
        let mut m = FakeMonitor::u3425we();
        m.caps = None;
        let (list, writes) = watched(m);
        assert_eq!(
            run_with(list.clone(), &["input", "set", "dp"]).unwrap(),
            "Switched to dp"
        );
        assert!(
            run_with(list, &["input", "set", "bogus"])
                .unwrap_err()
                .to_string()
                .contains("known names:")
        );
        assert_eq!(*writes.borrow(), vec![(0x60, 0x0F)]);
    }

    #[test]
    fn changes_refuse_to_guess_between_monitors() {
        let a = FakeMonitor::u3425we();
        let b = FakeMonitor::named("DELL U2723QE");
        let (wa, wb) = (a.writes.clone(), b.writes.clone());
        for args in [
            &["brightness", "50"][..],
            &["volume", "+5"],
            &["speaker", "off"],
            &["input", "set", "hdmi"],
        ] {
            let err = run_with(vec![a.clone(), b.clone()], args)
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("--monitor") && err.contains("2  DELL U2723QE"),
                "{args:?}: {err}"
            );
        }
        assert!(wa.borrow().is_empty() && wb.borrow().is_empty());

        assert_eq!(
            run_with(vec![a, b], &["-m", "u3425", "brightness", "50"]).unwrap(),
            "Brightness: 78 → 50"
        );
        assert_eq!(*wa.borrow(), vec![(0x10, 50)]);
    }

    #[test]
    fn write_failures_are_errors() {
        let mut m = FakeMonitor::u3425we();
        m.read_only = true;
        let err = run_with(vec![m], &["brightness", "50"])
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("couldn't change setting 0x10"), "{err}");
    }

    #[test]
    fn missing_monitor_number() {
        let err = run_with(dell(), &["--monitor", "2", "status"])
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("there's no monitor 2"), "{err}");
    }
}
