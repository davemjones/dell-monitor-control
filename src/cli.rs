use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "dellctl",
    version,
    about = "Control a Dell monitor's input, brightness, and speakers over DDC/CI"
)]
pub struct Cli {
    /// Monitor to control: its number from `dellctl list`, or part of its model name
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
    /// List connected monitors
    List {
        /// Also show which controls each monitor supports
        #[arg(long, short)]
        verbose: bool,
    },
    /// Show the active input, list inputs, or switch input
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
    /// Show the speaker state, or turn the built-in speakers on, off, or toggle them
    Speaker {
        #[arg(value_enum)]
        state: Option<SpeakerState>,
    },
}

#[derive(Debug, Subcommand)]
pub enum InputAction {
    /// List the inputs this monitor supports; the active one is marked with *
    List,
    /// Switch to an input by name (hdmi, dp, thunderbolt, usb-c, …) or raw code (e.g. 0x0f)
    Set { input: String },
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
        let cli = Cli::try_parse_from(["dellctl", "brightness", "-10"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Brightness {
                value: Some(Level::Delta(-10))
            })
        ));
    }

    #[test]
    fn bare_command_means_status() {
        let cli = Cli::try_parse_from(["dellctl"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn verifies_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
