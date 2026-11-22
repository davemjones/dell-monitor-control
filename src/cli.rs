use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "monitorctl",
    version,
    about = "Control a monitor's input, brightness, and speakers over DDC/CI",
    long_about = "Control a monitor's input, brightness, and speakers over DDC/CI.\n\n\
        With one monitor connected, every command targets it automatically. \
        DDC/CI must be enabled in the monitor's on-screen menu (Others → DDC/CI).",
    after_help = "Examples:
  monitorctl                       Show input, brightness, volume, and speakers
  monitorctl list                  List connected monitors
  monitorctl input set hdmi        Switch to HDMI
  monitorctl input name dp Work    Name the DisplayPort input \"Work\"
  monitorctl brightness +10        Raise brightness by 10
  monitorctl volume 30             Set speaker volume to 30
  monitorctl speaker toggle        Mute or unmute the speakers
  monitorctl status --json         Print status as JSON
  monitorctl -m U3425 brightness   Pick a monitor by model"
)]
pub struct Cli {
    /// Monitor to control: its number from `monitorctl list`, or part of its model name
    #[arg(long, short, global = true, value_name = "N|MODEL")]
    pub monitor: Option<String>,

    /// Print machine-readable JSON instead of text
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show input, brightness, volume, and speaker state (the default command)
    Status,
    /// List connected monitors and their numbers for --monitor
    List {
        /// Also show which controls each monitor supports
        #[arg(long, short)]
        verbose: bool,
    },
    /// Show the active input, list inputs, or switch input (hdmi, dp, thunderbolt / usb-c)
    Input {
        #[command(subcommand)]
        action: Option<InputAction>,
    },
    /// Show or change brightness (0–100, or +n / -n to nudge)
    Brightness {
        /// 0–100 to set an exact level, or +n / -n to nudge (omit to show the current level)
        #[arg(allow_hyphen_values = true, value_parser = parse_level)]
        value: Option<Level>,
    },
    /// Show or change speaker volume (0–100, or +n / -n to nudge)
    Volume {
        /// 0–100 to set an exact level, or +n / -n to nudge (omit to show the current level)
        #[arg(allow_hyphen_values = true, value_parser = parse_level)]
        value: Option<Level>,
    },
    /// Show or change whether the built-in speakers are muted (on, off, or toggle)
    Speaker {
        /// on unmutes, off mutes, toggle flips (omit to show the current state)
        #[arg(value_enum)]
        state: Option<SpeakerState>,
    },
}

#[derive(Debug, Subcommand)]
pub enum InputAction {
    /// List the inputs this monitor supports; the active one is marked with *
    List,
    /// Switch to an input by name (hdmi, dp, thunderbolt, usb-c, …), your name for it, or raw code (e.g. 0x11)
    Set {
        /// Input name or alias (hdmi, dp, displayport, thunderbolt, usb-c, tb), your name for it, or raw code (e.g. 0x11)
        input: String,
    },
    /// Show, set, or clear your own name for an input (e.g. "Work Mac")
    Name {
        /// The input to name: anything `input set` accepts
        input: String,
        /// The new name (omit to show the current one)
        name: Option<String>,
        /// Remove the input's name
        #[arg(long, conflicts_with = "name")]
        clear: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SpeakerState {
    On,
    Off,
    Toggle,
}

/// A brightness or volume argument: an exact value or a relative nudge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Set(u16),
    Delta(i16),
}

fn parse_level(s: &str) -> Result<Level, String> {
    let s = s.trim();
    let relative = s.starts_with('+') || s.starts_with('-');
    let n: i32 = s
        .parse()
        .map_err(|_| format!("'{s}' isn't a number; use 0–100, or +n / -n to nudge"))?;
    if n.unsigned_abs() > 100 {
        return Err(format!(
            "{s} is out of range; use 0–100, or +n / -n up to 100"
        ));
    }
    Ok(if relative {
        Level::Delta(n as i16)
    } else {
        Level::Set(n as u16)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_levels() {
        assert_eq!(parse_level("40"), Ok(Level::Set(40)));
        assert_eq!(parse_level("0"), Ok(Level::Set(0)));
        assert_eq!(parse_level("100"), Ok(Level::Set(100)));
        assert_eq!(parse_level("+10"), Ok(Level::Delta(10)));
        assert_eq!(parse_level("-5"), Ok(Level::Delta(-5)));
        assert!(parse_level("101").is_err());
        assert!(parse_level("+200").is_err());
        assert!(parse_level("abc").is_err());
        assert!(parse_level("").is_err());
    }

    #[test]
    fn negative_nudge_is_not_a_flag() {
        let cli = Cli::try_parse_from(["monitorctl", "brightness", "-10"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Brightness {
                value: Some(Level::Delta(-10))
            })
        ));
    }

    #[test]
    fn bare_command_means_status() {
        let cli = Cli::try_parse_from(["monitorctl"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn input_name_takes_a_name_or_clear() {
        let cli =
            Cli::try_parse_from(["monitorctl", "input", "name", "hdmi", "Gaming PC"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Input {
                action: Some(InputAction::Name { name: Some(ref n), clear: false, .. })
            }) if n == "Gaming PC"
        ));
        assert!(Cli::try_parse_from(["monitorctl", "input", "name", "hdmi", "--clear"]).is_ok());
        assert!(
            Cli::try_parse_from(["monitorctl", "input", "name", "hdmi", "X", "--clear"]).is_err()
        );
    }

    #[test]
    fn verifies_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
