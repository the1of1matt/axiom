mod archive;
mod commands;
mod deps;
mod detect;
mod graph;
mod model;
mod platform;
mod orchestrate;
mod provider;
mod providers;
mod scan;
mod util;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "axiom",
    version,
    about = "Axiom — application runtime orchestrator",
    long_about = "Discover components and providers, build an application graph, prepare, start, and verify readiness."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    New {
        name: String,
        #[arg(short, long)]
        path: Option<String>,
    },
    Find {
        name: String,
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    Run {
        target: Option<String>,
        #[arg(short, long)]
        verbose: bool,
        #[arg(long)]
        keep_temp: bool,
    },
    Doctor {
        path: Option<String>,
        #[arg(long)]
        repair: bool,
    },
    Uninstall {
        #[arg(long)]
        yes: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::New { name, path } => commands::new::run(&name, path.as_deref()),
        Commands::Find { name, limit } => commands::find::run(&name, limit),
        Commands::Run { target, verbose, keep_temp } => {
            commands::run::run(target.as_deref(), verbose, keep_temp)
        }
        Commands::Doctor { path, repair } => commands::doctor::run(path.as_deref(), repair),
        Commands::Uninstall { yes } => commands::uninstall::run(yes),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
