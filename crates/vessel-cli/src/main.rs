use std::process::ExitCode;

use clap::Parser;
use vessel_cli::{Cli, execute};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    match execute(cli).await {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }

        Err(error) => {
            eprintln!("VESSEL CLI error: {error}");
            ExitCode::FAILURE
        }
    }
}
