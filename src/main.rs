mod backend;
mod cli;
mod commands;
mod failure;
mod output;
mod select;
mod vcp;

use std::process::ExitCode;

use clap::Parser;

use backend::DdcHiBackend;
use cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let backend = DdcHiBackend {
        with_serials: matches!(cli.command, Some(Command::List { .. })),
    };

    match commands::run(&cli, &backend) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(err) if cli.json => {
            eprintln!("{}", failure::json(&err));
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
