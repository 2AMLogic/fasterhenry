//! `fasterhenry` CLI — placeholder until the engine's first milestone lands.

use clap::Parser;

/// Clean-room PEEC inductance/resistance extractor.
#[derive(Parser)]
#[command(name = "fasterhenry", version, about)]
struct Cli {}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    println!("fasterhenry {} — engine not yet implemented", fasterhenry::VERSION);
    Ok(())
}
