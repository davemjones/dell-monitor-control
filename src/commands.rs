//! Command logic: talk to the selected monitor(s) and produce output text.

use anyhow::{Result, anyhow, bail};
use serde_json::json;

use crate::backend::{Backend, Caps, Monitor};
use crate::cli::{Cli, Command, InputAction, Level, SpeakerState};
use crate::failure::{Failure, FailureKind};
use crate::names::{self, Labels, NameStore, Names};
use crate::output::{self, Controls, InputEntry, InputRef, ListEntry, Status};
use crate::select::{self, Indexed};
use crate::vcp;

pub fn run(cli: &Cli, backend: &dyn Backend, store: &dyn NameStore) -> Result<String> {
    let wanted = cli.monitor.as_deref();
    let command = cli.command.as_ref().unwrap_or(&Command::Status);
    // Showing names is a nicety: a broken names file mustn't stop status or
    // switching by built-in name. Commands that need the file report it.
    let loaded = store.load();
    let names = loaded.as_ref().ok().cloned().unwrap_or_default();
    let labels_of = |m: &select::Indexed| names.labels(&m.monitor.info().name);

    match command {
        Command::Status => {
            let statuses: Vec<Status> = select::all_or_one(backend.enumerate()?, wanted)?
                .iter_mut()
                // Capabilities are slow to read, so only JSON lists the inputs.
                .map(|m| {
                    let labels = labels_of(m);
                    read_status(m, cli.json, &labels)
                })
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
            let labels = labels_of(&m);
            let input = read_input(m.monitor.as_mut(), &labels)?;
            Ok(if cli.json {
                output::json(&input)
            } else {
                match &input.label {
                    Some(label) => format!("Input: {} [{}] ({label})", input.name, input.code),
                    None => format!("Input: {} [{}]", input.name, input.code),
                }
            })
        }

        Command::Input {
            action: Some(InputAction::List),
        } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let labels = labels_of(&m);
            let entries = read_inputs(m.monitor.as_mut(), &labels)?;
            Ok(if cli.json {
                output::json(&entries)
            } else {
                output::inputs_text(&entries)
            })
        }

        Command::Brightness { value: None } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let level = read_percent(m.monitor.as_mut(), vcp::BRIGHTNESS)?;
            Ok(if cli.json {
                output::json(&json!({ "brightness": level }))
            } else {
                format!("Brightness: {level}")
            })
        }

        Command::Volume { value: None } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let level = read_percent(m.monitor.as_mut(), vcp::VOLUME)?;
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
            let labels = labels_of(&m);
            let monitor = m.monitor.as_mut();
            let code = match (resolve_input(input, monitor, &labels), loaded) {
                (Ok(code), _) => code,
                // It may be a custom name we couldn't read; say why.
                (Err(_), Err(broken)) => return Err(broken),
                (Err(unknown), Ok(_)) => return Err(unknown),
            };

            let before = read_input(monitor, &labels)?;
            let after = InputRef::new(code, &labels);
            let changed = before != after;
            if changed {
                // No read-back: once the monitor shows another computer's
                // input, this one may no longer be able to talk to it.
                monitor.write_vcp(vcp::INPUT, code.into())?;
            }
            Ok(if cli.json {
                output::json(&json!({ "before": before, "after": after, "changed": changed }))
            } else if changed {
                format!("Switched to {}", after.described())
            } else {
                format!("Already on {}", after.described())
            })
        }

        Command::Input {
            action: Some(InputAction::Name { input, name, clear }),
        } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let mut names = loaded?;
            let model = m.monitor.info().name.clone();
            let labels = names.labels(&model);
            let code = resolve_input(input, m.monitor.as_mut(), &labels)?;
            let before = names.get(&model, code).map(str::to_string);
            let after = match (name, clear) {
                (Some(name), _) => Some(names::validate(name, code, &labels)?),
                (None, true) => None,
                (None, false) => return Ok(show_name(cli.json, InputRef::new(code, &labels))),
            };
            if after != before {
                set_name(&mut names, &model, code, after.clone());
                store.save(&names)?;
            }
            let input = InputRef::new(code, &names.labels(&model));
            Ok(if cli.json {
                output::json(&json!({
                    "input": input,
                    "label": { "before": before, "after": after },
                }))
            } else {
                match (&after, &before) {
                    (Some(label), _) => format!("Named {} \"{label}\"", input.name),
                    (None, Some(_)) => format!("Cleared the name for {}", input.name),
                    (None, None) => format!("{} already has no name", input.name),
                }
            })
        }

        Command::Brightness { value: Some(level) } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let (before, after) = change_level(m.monitor.as_mut(), vcp::BRIGHTNESS, *level)?;
            Ok(if cli.json {
                output::json(&json!({ "brightness": { "before": before, "after": after } }))
            } else {
                format!("Brightness: {before} → {after}")
            })
        }

        Command::Volume { value: Some(level) } => {
            let mut m = select::one(backend.enumerate()?, wanted)?;
            let monitor = m.monitor.as_mut();
            let (before, after) = change_level(monitor, vcp::VOLUME, *level)?;
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
fn change_level(monitor: &mut dyn Monitor, code: u8, level: Level) -> Result<(u16, u16)> {
    let raw = monitor.read_vcp(code)?;
    let before = vcp::percent(raw.value, raw.max);
    let after = vcp::apply_level(before, level);
    monitor.write_vcp(code, vcp::from_percent(after, raw.max))?;
    Ok((before, after))
}

fn mute_unsupported(monitor: &dyn Monitor) -> Failure {
    Failure::new(
        FailureKind::Unsupported,
        format!(
            "{} doesn't support speaker mute over DDC/CI; try `monitorctl volume 0` instead",
            monitor.info().name
        ),
    )
}

fn show_name(json: bool, input: InputRef) -> String {
    if json {
        return output::json(&json!({ "input": input }));
    }
    match &input.label {
        Some(label) => format!("{}: {label}", input.name),
        None => format!("{} has no name", input.name),
    }
}

fn set_name(names: &mut Names, model: &str, code: u8, label: Option<String>) {
    match label {
        Some(label) => names.set(model, code, label),
        None => names.clear(model, code),
    }
}

/// The input code `arg` means on this monitor: a built-in name, alias, or
/// code first, then one of the user's names for its inputs.
fn resolve_input(arg: &str, monitor: &mut dyn Monitor, labels: &Labels) -> Result<u8> {
    // Unreadable capabilities only skip the "does it have that input" check;
    // the switch itself can still work.
    let supported = monitor
        .capabilities()
        .ok()
        .and_then(|caps| caps.features.get(&vcp::INPUT).cloned())
        .filter(|codes| !codes.is_empty());
    vcp::parse_input(arg)
        .or_else(|| names::find(labels, arg))
        .filter(|code| supported.as_ref().is_none_or(|s| s.contains(code)))
        .ok_or_else(|| anyhow!(unknown_input(arg, monitor, supported.as_deref(), labels)))
}

fn unknown_input(
    arg: &str,
    monitor: &dyn Monitor,
    supported: Option<&[u8]>,
    labels: &Labels,
) -> String {
    let valid = match supported {
        Some(codes) => {
            let entries: Vec<InputEntry> = codes
                .iter()
                .map(|&code| InputEntry {
                    input: InputRef::new(code, labels),
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
fn read_status(m: &mut Indexed, with_inputs: bool, labels: &Labels) -> Status {
    let monitor = m.monitor.as_mut();
    Status {
        monitor: m.index,
        name: monitor.info().name.clone(),
        input: read_input(monitor, labels).ok(),
        inputs: with_inputs
            .then(|| read_inputs(monitor, labels).ok())
            .flatten(),
        brightness: read_percent(monitor, vcp::BRIGHTNESS).ok(),
        volume: read_percent(monitor, vcp::VOLUME).ok(),
        speakers: read_speakers(monitor),
    }
}

/// The inputs the monitor supports, with the active one marked.
fn read_inputs(monitor: &mut dyn Monitor, labels: &Labels) -> Result<Vec<InputEntry>> {
    let caps = monitor.capabilities()?;
    let Some(codes) = caps.features.get(&vcp::INPUT).filter(|c| !c.is_empty()) else {
        bail!(Failure::new(
            FailureKind::Unsupported,
            format!(
                "{} doesn't report its inputs over DDC/CI",
                monitor.info().name
            )
        ));
    };
    let active = read_input(monitor, labels).ok();
    Ok(codes
        .iter()
        .map(|&code| {
            let input = InputRef::new(code, labels);
            InputEntry {
                active: active.as_ref() == Some(&input),
                input,
            }
        })
        .collect())
}

fn read_input(monitor: &mut dyn Monitor, labels: &Labels) -> Result<InputRef> {
    let raw = monitor.read_vcp(vcp::INPUT)?;
    Ok(InputRef::new(vcp::current_input(raw.value), labels))
}

fn read_percent(monitor: &mut dyn Monitor, code: u8) -> Result<u16> {
    let raw = monitor.read_vcp(code)?;
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
    use crate::names::MemoryStore;
    use clap::Parser;

    fn run_with(monitors: Vec<FakeMonitor>, args: &[&str]) -> Result<String> {
        run_named(monitors, &MemoryStore::new(), args)
    }

    fn run_named(monitors: Vec<FakeMonitor>, store: &MemoryStore, args: &[&str]) -> Result<String> {
        let cli =
            Cli::try_parse_from(std::iter::once("monitorctl").chain(args.iter().copied())).unwrap();
        run(&cli, &FakeBackend(monitors), store)
    }

    const U3425WE: &str = "DELL U3425WE";

    /// The U3425WE's HDMI and Thunderbolt inputs, named.
    fn named() -> MemoryStore {
        MemoryStore::with(U3425WE, &[(0x11, "Gaming PC"), (0x19, "Work Mac")])
    }

    fn u3425we() -> Vec<FakeMonitor> {
        vec![FakeMonitor::u3425we()]
    }

    #[test]
    fn status_text_matches_prd_layout() {
        let out = run_with(u3425we(), &[]).unwrap();
        assert_eq!(
            out,
            "DELL U3425WE (monitor 1)\n  Input:      thunderbolt\n  Brightness: 78\n  Volume:     88\n  Speakers:   n/a"
        );
        assert_eq!(run_with(u3425we(), &["status"]).unwrap(), out);
    }

    #[test]
    fn status_json_single_monitor_is_an_object() {
        let out: serde_json::Value =
            serde_json::from_str(&run_with(u3425we(), &["status", "--json"]).unwrap()).unwrap();
        assert_eq!(
            out,
            json!({
                "monitor": 1,
                "name": "DELL U3425WE",
                "input": { "name": "thunderbolt", "code": "0x19", "label": null },
                "inputs": [
                    { "name": "dp", "code": "0x0f", "label": null, "active": false },
                    { "name": "hdmi", "code": "0x11", "label": null, "active": false },
                    { "name": "thunderbolt", "code": "0x19", "label": null, "active": true }
                ],
                "brightness": 78,
                "volume": 88,
                "speakers": null
            })
        );
    }

    #[test]
    fn status_json_inputs_are_null_without_caps() {
        let out: serde_json::Value = serde_json::from_str(
            &run_with(
                vec![FakeMonitor::named("DELL P2419H")],
                &["status", "--json"],
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(out["inputs"], serde_json::Value::Null);
    }

    #[test]
    fn errors_carry_a_kind() {
        use crate::failure::kind_of;
        let kind = |monitors, args: &[&str]| kind_of(&run_with(monitors, args).unwrap_err());
        assert_eq!(kind(vec![], &["status"]), FailureKind::NoMonitor);
        assert_eq!(
            kind(
                vec![FakeMonitor::u3425we(), FakeMonitor::u3425we()],
                &["brightness"]
            ),
            FailureKind::MultipleMonitors
        );
        let no_inputs = FakeMonitor {
            caps: Some("(prot(monitor)vcp(10 12))"),
            ..FakeMonitor::named("DELL P2419H")
        };
        assert_eq!(
            kind(vec![no_inputs], &["input", "list"]),
            FailureKind::Unsupported
        );
        // Unreadable capabilities are a failure, not a missing feature.
        assert_eq!(
            kind(vec![FakeMonitor::named("DELL P2419H")], &["input", "list"]),
            FailureKind::Failed
        );
        assert_eq!(kind(u3425we(), &["speaker"]), FailureKind::Unsupported);
        assert_eq!(
            kind(u3425we(), &["-m", "9", "brightness"]),
            FailureKind::Failed
        );

        let err = run_with(vec![], &["status", "--json"]).unwrap_err();
        let json: serde_json::Value = serde_json::from_str(&crate::failure::json(&err)).unwrap();
        assert_eq!(json["error"]["kind"], "no_monitor");
        assert!(json["error"]["message"].as_str().unwrap().contains("hint:"));
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
            run_with(u3425we(), &["list"]).unwrap(),
            "1  DELL U3425WE  (serial JK4HV84)"
        );
        assert_eq!(
            run_with(u3425we(), &["list", "--verbose"]).unwrap(),
            "1  DELL U3425WE  (serial JK4HV84)\n   supports:      input, brightness, volume\n   not supported: speaker mute"
        );
        let json: serde_json::Value =
            serde_json::from_str(&run_with(u3425we(), &["list", "-v", "--json"]).unwrap()).unwrap();
        assert_eq!(
            json[0]["controls"],
            json!({ "input": true, "brightness": true, "volume": true, "mute": false })
        );
        assert!(
            run_with(u3425we(), &["list", "--json"])
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
            run_with(u3425we(), &["input"]).unwrap(),
            "Input: thunderbolt [0x19]"
        );
        assert_eq!(
            run_with(u3425we(), &["input", "list"]).unwrap(),
            "  dp           [0x0f]\n  hdmi         [0x11]\n* thunderbolt  [0x19]"
        );
        let json: serde_json::Value =
            serde_json::from_str(&run_with(u3425we(), &["input", "list", "--json"]).unwrap())
                .unwrap();
        assert_eq!(
            json[2],
            json!({ "name": "thunderbolt", "code": "0x19", "label": null, "active": true })
        );
    }

    #[test]
    fn single_reads() {
        assert_eq!(
            run_with(u3425we(), &["brightness"]).unwrap(),
            "Brightness: 78"
        );
        assert_eq!(run_with(u3425we(), &["volume"]).unwrap(), "Volume: 88");
        assert_eq!(
            run_with(u3425we(), &["brightness", "--json"]).unwrap(),
            "{\n  \"brightness\": 78\n}"
        );
    }

    #[test]
    fn speaker_state_and_unsupported() {
        let err = run_with(u3425we(), &["speaker"]).unwrap_err().to_string();
        assert!(
            err.contains("doesn't support speaker mute") && err.contains("monitorctl volume 0"),
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
        let (monitor, writes) = watched(FakeMonitor::u3425we());
        assert_eq!(
            run_with(monitor.clone(), &["brightness", "40"]).unwrap(),
            "Brightness: 78 → 40"
        );
        assert_eq!(
            run_with(monitor.clone(), &["brightness", "+10"]).unwrap(),
            "Brightness: 78 → 88"
        );
        assert_eq!(
            run_with(monitor.clone(), &["brightness", "-10"]).unwrap(),
            "Brightness: 78 → 68"
        );
        assert_eq!(
            run_with(monitor.clone(), &["brightness", "+50"]).unwrap(),
            "Brightness: 78 → 100"
        );
        assert_eq!(
            run_with(monitor, &["brightness", "-100"]).unwrap(),
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
            serde_json::from_str(&run_with(u3425we(), &["brightness", "+5", "--json"]).unwrap())
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
        assert_eq!(err, "couldn't read volume (VCP 0x62) from DELL P2419H");
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
        let (monitor, writes) = watched(FakeMonitor::u3425we());
        for state in ["on", "off", "toggle"] {
            let err = run_with(monitor.clone(), &["speaker", state])
                .unwrap_err()
                .to_string();
            assert!(
                err.contains("doesn't support speaker mute") && err.contains("monitorctl volume 0"),
                "{err}"
            );
        }
        assert!(writes.borrow().is_empty());
    }

    #[test]
    fn input_set_by_name_alias_and_code() {
        let (monitor, writes) = watched(FakeMonitor::u3425we());
        for (arg, expected) in [
            ("hdmi", "Switched to hdmi"),
            ("HDMI1", "Switched to hdmi"),
            ("dp", "Switched to dp"),
            ("DisplayPort", "Switched to dp"),
            ("0x11", "Switched to hdmi"),
            ("17", "Switched to hdmi"),
        ] {
            assert_eq!(
                run_with(monitor.clone(), &["input", "set", arg]).unwrap(),
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
        let (monitor, writes) = watched(FakeMonitor::u3425we());
        for arg in ["thunderbolt", "usb-c", "usbc", "TB", "0x19"] {
            assert_eq!(
                run_with(monitor.clone(), &["input", "set", arg]).unwrap(),
                "Already on thunderbolt",
                "{arg}"
            );
        }
        let out: serde_json::Value =
            serde_json::from_str(&run_with(monitor, &["input", "set", "tb", "--json"]).unwrap())
                .unwrap();
        assert_eq!(out["changed"], json!(false));
        assert!(writes.borrow().is_empty());
    }

    #[test]
    fn input_set_json() {
        let out: serde_json::Value = serde_json::from_str(
            &run_with(u3425we(), &["input", "set", "hdmi", "--json"]).unwrap(),
        )
        .unwrap();
        assert_eq!(
            out,
            json!({
                "before": { "name": "thunderbolt", "code": "0x19", "label": null },
                "after": { "name": "hdmi", "code": "0x11", "label": null },
                "changed": true
            })
        );
    }

    #[test]
    fn input_set_rejects_what_the_monitor_lacks() {
        let (monitor, writes) = watched(FakeMonitor::u3425we());
        for arg in ["vga", "0x12", "hdmi2", "bogus"] {
            let err = run_with(monitor.clone(), &["input", "set", arg])
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
        assert_eq!(err, "couldn't change brightness (VCP 0x10) on DELL U3425WE");
    }

    #[test]
    fn missing_monitor_number() {
        let err = run_with(u3425we(), &["--monitor", "2", "status"])
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("there's no monitor 2"), "{err}");
    }

    #[test]
    fn names_show_in_status_and_input_lists() {
        let store = named();
        assert_eq!(
            run_named(u3425we(), &store, &[]).unwrap(),
            "DELL U3425WE (monitor 1)\n  Input:      thunderbolt (Work Mac)\n  Brightness: 78\n  Volume:     88\n  Speakers:   n/a"
        );
        assert_eq!(
            run_named(u3425we(), &store, &["input"]).unwrap(),
            "Input: thunderbolt [0x19] (Work Mac)"
        );
        assert_eq!(
            run_named(u3425we(), &store, &["input", "list"]).unwrap(),
            "  dp           [0x0f]\n  hdmi         [0x11]  Gaming PC\n* thunderbolt  [0x19]  Work Mac"
        );
        let out: serde_json::Value =
            serde_json::from_str(&run_named(u3425we(), &store, &["--json"]).unwrap()).unwrap();
        assert_eq!(
            out["input"],
            json!({ "name": "thunderbolt", "code": "0x19", "label": "Work Mac" })
        );
        assert_eq!(out["inputs"][0]["label"], json!(null));
        assert_eq!(out["inputs"][1]["label"], json!("Gaming PC"));
    }

    #[test]
    fn names_belong_to_a_model() {
        let store = named();
        let mut other = FakeMonitor::u3425we();
        other.info.name = "DELL U2723QE".into();
        assert_eq!(
            run_named(vec![other], &store, &["input", "list"]).unwrap(),
            "  dp           [0x0f]\n  hdmi         [0x11]\n* thunderbolt  [0x19]"
        );
    }

    #[test]
    fn input_set_by_custom_name() {
        let store = named();
        let (monitor, writes) = watched(FakeMonitor::u3425we());
        assert_eq!(
            run_named(monitor.clone(), &store, &["input", "set", " gaming pc "]).unwrap(),
            "Switched to hdmi (Gaming PC)"
        );
        assert_eq!(
            run_named(monitor.clone(), &store, &["input", "set", "Work Mac"]).unwrap(),
            "Already on thunderbolt (Work Mac)"
        );
        let out: serde_json::Value = serde_json::from_str(
            &run_named(monitor.clone(), &store, &["input", "set", "hdmi", "--json"]).unwrap(),
        )
        .unwrap();
        assert_eq!(out["after"]["label"], json!("Gaming PC"));
        let err = run_named(monitor, &store, &["input", "set", "Gaming"])
            .unwrap_err()
            .to_string();
        assert!(err.contains("hdmi         [0x11]  Gaming PC"), "{err}");
        assert_eq!(*writes.borrow(), vec![(0x60, 0x11), (0x60, 0x11)]);
    }

    #[test]
    fn name_set_show_and_clear() {
        let store = MemoryStore::new();
        let (monitor, writes) = watched(FakeMonitor::u3425we());
        let run = |args: &[&str]| run_named(monitor.clone(), &store, args).unwrap();
        assert_eq!(run(&["input", "name", "hdmi"]), "hdmi has no name");
        assert_eq!(
            run(&["input", "name", "HDMI1", "  Gaming PC "]),
            "Named hdmi \"Gaming PC\""
        );
        assert_eq!(store.names.borrow().get(U3425WE, 0x11), Some("Gaming PC"));
        assert_eq!(run(&["input", "name", "0x11"]), "hdmi: Gaming PC");
        // Rename by its current custom name.
        assert_eq!(
            run(&["input", "name", "gaming pc", "Xbox"]),
            "Named hdmi \"Xbox\""
        );
        assert_eq!(
            run(&["input", "name", "xbox", "--clear"]),
            "Cleared the name for hdmi"
        );
        assert_eq!(
            run(&["input", "name", "hdmi", "--clear"]),
            "hdmi already has no name"
        );
        assert!(store.names.borrow().monitors.is_empty());
        assert!(writes.borrow().is_empty());
    }

    #[test]
    fn name_json() {
        let store = named();
        let json = |args: &[&str]| -> serde_json::Value {
            serde_json::from_str(&run_named(u3425we(), &store, args).unwrap()).unwrap()
        };
        assert_eq!(
            json(&["input", "name", "hdmi", "--json"]),
            json!({ "input": { "name": "hdmi", "code": "0x11", "label": "Gaming PC" } })
        );
        assert_eq!(
            json(&["input", "name", "dp", "Spare", "--json"]),
            json!({
                "input": { "name": "dp", "code": "0x0f", "label": "Spare" },
                "label": { "before": null, "after": "Spare" }
            })
        );
        assert_eq!(
            json(&["input", "name", "hdmi", "--clear", "--json"]),
            json!({
                "input": { "name": "hdmi", "code": "0x11", "label": null },
                "label": { "before": "Gaming PC", "after": null }
            })
        );
    }

    #[test]
    fn name_rejects_bad_names_and_missing_inputs() {
        let store = named();
        let err = |args: &[&str]| run_named(u3425we(), &store, args).unwrap_err().to_string();
        assert!(err(&["input", "name", "dp", "hdmi"]).contains("already means an input"));
        assert!(err(&["input", "name", "dp", "17"]).contains("already means an input"));
        assert!(
            err(&["input", "name", "dp", "work mac"]).starts_with("thunderbolt is already named")
        );
        assert!(err(&["input", "name", "dp", " "]).contains("empty"));
        assert!(err(&["input", "name", "dp", &"x".repeat(21)]).contains("too long"));
        assert!(err(&["input", "name", "vga", "Old PC"]).starts_with("unknown input 'vga'"));
        assert_eq!(*store.names.borrow(), named().names.into_inner());
    }

    #[test]
    fn broken_names_file_only_stops_what_needs_it() {
        let store = MemoryStore {
            broken: true,
            ..named()
        };
        assert!(run_named(u3425we(), &store, &["status"]).is_ok());
        assert!(run_named(u3425we(), &store, &["input", "list"]).is_ok());
        assert_eq!(
            run_named(u3425we(), &store, &["input", "set", "hdmi"]).unwrap(),
            "Switched to hdmi"
        );
        for args in [
            &["input", "set", "Work Mac"][..],
            &["input", "name", "dp", "Spare"],
        ] {
            let err = run_named(u3425we(), &store, args).unwrap_err().to_string();
            assert!(err.contains("isn't a valid names file"), "{args:?}: {err}");
        }
    }
}
