use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    match parcel::cli::run(parcel::cli::Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("parcel: {e}");
            ExitCode::FAILURE
        }
    }
}
