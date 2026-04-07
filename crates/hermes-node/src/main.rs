use std::{fs, path::PathBuf, thread, time::Duration};

use anyhow::Result;
use clap::{Parser, Subcommand};
use hermes_core::{
    consensus::Yc3Input,
    config::HermesConfig,
    runtime::DeploymentPlanner,
    Yc3Engine, Yc3Parameters,
};

#[derive(Debug, Parser)]
#[command(name = "hermes-node")]
#[command(about = "Hermes validator runtime and planning CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    ValidateConfig { path: PathBuf },
    Plan { path: PathBuf },
    Serve {
        path: PathBuf,
        #[arg(long)]
        snapshot: Option<PathBuf>,
        #[arg(long, default_value_t = 60)]
        interval_seconds: u64,
        #[arg(long, default_value_t = false)]
        once: bool,
    },
    ScoreSnapshot {
        path: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        pretty: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::ValidateConfig { path } => {
            let config = load_config(&path)?;
            config.validate()?;
            println!("validated {}", path.display());
        }
        Command::Plan { path } => {
            let config = load_config(&path)?;
            let plan = DeploymentPlanner::build(&config)?;
            println!("{}", serde_json::to_string_pretty(&plan)?);
        }
        Command::Serve {
            path,
            snapshot,
            interval_seconds,
            once,
        } => {
            run_serve(path, snapshot, interval_seconds, once)?;
        }
        Command::ScoreSnapshot {
            path,
            config,
            pretty,
        } => {
            let input = load_snapshot(&path)?;
            let params = if let Some(config_path) = config {
                let config = load_config(&config_path)?;
                Yc3Parameters {
                    kappa: config.consensus.kappa,
                    bonds_penalty: config.consensus.bonds_penalty,
                    ema_alpha: config.consensus.ema_alpha,
                    validator_emission_ratio: config.consensus.validator_emission_ratio,
                }
            } else {
                Yc3Parameters::default()
            };

            let report = Yc3Engine::new(params).evaluate(&input)?;
            if pretty {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", serde_json::to_string(&report)?);
            }
        }
    }

    Ok(())
}

fn load_config(path: &PathBuf) -> Result<HermesConfig> {
    let raw = fs::read_to_string(path)?;
    Ok(HermesConfig::from_json_str(&raw)?)
}

fn load_snapshot(path: &PathBuf) -> Result<Yc3Input> {
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn run_serve(
    path: PathBuf,
    snapshot: Option<PathBuf>,
    interval_seconds: u64,
    once: bool,
) -> Result<()> {
    let config = load_config(&path)?;
    config.validate()?;

    loop {
        let plan = DeploymentPlanner::build(&config)?;
        println!("{}", serde_json::to_string(&plan)?);

        if let Some(snapshot_path) = snapshot.as_ref() {
            let params = Yc3Parameters {
                kappa: config.consensus.kappa,
                bonds_penalty: config.consensus.bonds_penalty,
                ema_alpha: config.consensus.ema_alpha,
                validator_emission_ratio: config.consensus.validator_emission_ratio,
            };
            let report = Yc3Engine::new(params).evaluate(&load_snapshot(snapshot_path)?)?;
            println!(
                "processed snapshot with {} validators and {} miners",
                report.validators.len(),
                report.miners.len()
            );
        } else {
            println!("validated runtime configuration for {}", config.cluster.network.public_endpoint);
        }

        if once {
            break;
        }

        thread::sleep(Duration::from_secs(interval_seconds.max(5)));
    }

    Ok(())
}