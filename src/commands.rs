//! Command logic: talk to the selected monitor(s) and produce output text.

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::backend::{Backend, Caps, Monitor};
use crate::cli::{Cli, Command, InputAction};
use crate::output::{self, Controls, InputEntry, InputRef, ListEntry, Status};
use crate::select::{self, Indexed};
use crate::vcp;

const NOT_YET: &str = "changing settings isn't implemented yet (coming in milestone 2)";

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
                bail!(
                    "{} doesn't support speaker mute over DDC/CI; try `dellctl volume 0` instead",
                    m.monitor.info().name
                );
            };
            Ok(if cli.json {
                output::json(&json!({ "speakers": state }))
            } else {
                format!("Speakers: {state}")
            })
        }

        Command::Input {
            action: Some(InputAction::Set { .. }),
        }
        | Command::Brightness { value: Some(_) }
        | Command::Volume { value: Some(_) }
        | Command::Speaker { state: Some(_) } => bail!(NOT_YET),
    }
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
    use crate::backend::fake::{FakeBackend, FakeMonitor};
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

    #[test]
    fn setting_commands_are_not_yet_implemented() {
        for args in [
            &["brightness", "50"][..],
            &["brightness", "-10"],
            &["volume", "+5"],
            &["speaker", "off"],
            &["input", "set", "hdmi"],
        ] {
            assert_eq!(
                run_with(dell(), args).unwrap_err().to_string(),
                NOT_YET,
                "{args:?}"
            );
        }
    }

    #[test]
    fn missing_monitor_number() {
        let err = run_with(dell(), &["--monitor", "2", "status"])
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("there's no monitor 2"), "{err}");
    }
}
