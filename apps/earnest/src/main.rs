//! The `earnest` command line: checks models written in TypeScript or Rust.

mod arguments;
mod cache;
mod changes;
mod commands;
mod confirm;
mod discovery;
mod print;
mod reading;
mod run;
mod select;
#[cfg(test)]
mod sources;
mod verify;
mod workers;

use arguments::{Arguments, Command};
use clap::Parser;
use commands::Exit;
use std::process::ExitCode;

const MAIN_STACK: usize = 512 * 1024 * 1024;

fn main() -> ExitCode {
    let started = std::thread::Builder::new()
        .stack_size(MAIN_STACK)
        .spawn(run);
    match started {
        Ok(running) => running
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
        Err(_) => run(),
    }
}

fn run() -> ExitCode {
    let arguments = match Arguments::try_parse() {
        Ok(arguments) => arguments,
        Err(error) => {
            let code = if error.use_stderr() {
                Exit::Misused
            } else {
                Exit::Passed
            };
            drop(error.print());
            return code.into();
        }
    };
    let ended = match &arguments.command {
        Command::Verify(verify) => verify::verify(verify),
        Command::Chart(chart) => commands::chart_command(chart),
        Command::Report(report) => commands::report_command(report),
        Command::Tla(tla) => commands::tla_command(tla),
        Command::Confirm(confirm) => commands::confirm_command(confirm),
    };
    match ended {
        Ok(exit) => exit.into(),
        Err(misuse) => {
            print::to_stderr(&format!("error: {misuse}"));
            Exit::Misused.into()
        }
    }
}
