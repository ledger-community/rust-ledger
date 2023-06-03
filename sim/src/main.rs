//! Rust ledger-sim example application, supports invoking speculos from the command line.

use clap::Parser;
use tracing::{debug, info};
use tracing_subscriber::{filter::LevelFilter, EnvFilter, FmtSubscriber};

use ledger_sim::*;

/// Ledger Speculos simulator wrapper tool
///
/// This calls out to a Docker or local speculos install
/// to provide a simple way of executing speculos in CI/CD.
#[derive(Clone, Debug, PartialEq, Parser)]
pub struct Args {
    /// Application to run
    app: String,

    /// Driver mode
    #[clap(long, value_enum, default_value = "docker")]
    driver: DriverMode,

    #[clap(flatten)]
    speculos_opts: Options,

    /// Log level
    #[clap(long, default_value = "debug")]
    log_level: LevelFilter,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    info!("Launching speculos...");

    // Setup logging
    // Setup logging
    let filter = EnvFilter::from_default_env()
        .add_directive("bollard=warn".parse()?)
        .add_directive(args.log_level.into());

    let _ = FmtSubscriber::builder()
        .compact()
        .without_time()
        .with_max_level(args.log_level)
        .with_env_filter(filter)
        .try_init();

    // Run with specified driver
    match args.driver {
        DriverMode::Local => {
            let d = LocalDriver::new();
            run_simulator(d, &args.app, args.speculos_opts).await?;
        }
        DriverMode::Docker => {
            let d = DockerDriver::new()?;
            run_simulator(d, &args.app, args.speculos_opts).await?;
        }
    }

    Ok(())
}

async fn run_simulator<D: Driver>(driver: D, app: &str, opts: Options) -> anyhow::Result<()> {
    // Start simulator
    let mut h = driver.run(app, opts).await?;

    // Await simulator exit or exit signal
    tokio::select!(
        // Await simulator task completion
        _ = driver.wait(&mut h) => {
            debug!("Complete!");
        }
        // Exit on ctrl + c
        _ = tokio::signal::ctrl_c() => {
            debug!("Exit!");
            driver.exit(h).await?;
        },
    );

    Ok(())
}
