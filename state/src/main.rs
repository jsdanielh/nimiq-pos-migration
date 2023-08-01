use std::time::{Duration, Instant};

use clap::Parser;
use log::level_filters::LevelFilter;
use nimiq_rpc::Client;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use state_migration::{get_pos_genesis, write_pos_genesis};

/// Command line arguments for the binary
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// RPC connection URL to use
    #[arg(short, long)]
    rpc: String,

    /// TOML output file name
    #[arg(short, long)]
    file: String,

    /// Cutting block height to use
    #[arg(short, long)]
    height: u32,

    /// Cutting block hash to use
    #[arg(short, long)]
    hash: String,

    /// VrfSeed
    #[arg(short, long)]
    vrf: String,

    /// Genesis delay in minutes
    #[arg(short, long)]
    delay: u64,
}

fn initialize_logging() {
    let filter = Targets::new().with_default(LevelFilter::DEBUG);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_filter(filter),
        )
        .init();
}

fn main() {
    initialize_logging();

    let args = Args::parse();
    let client = Client::new(&args.rpc);
    let vrf_seed = match serde_json::from_str(&format!(r#""{}""#, args.vrf)) {
        Ok(value) => value,
        Err(error) => {
            log::error!(?error, value = args.vrf, "Invalid VRF seed");
            std::process::exit(1);
        }
    };

    log::info!("Generating genesis configuration from PoW chain");
    let genesis_delay = Duration::from_secs(args.delay * 60);
    let start = Instant::now();
    let genesis_config =
        match get_pos_genesis(&client, args.hash, args.height, &vrf_seed, genesis_delay) {
            Ok(config) => config,
            Err(error) => {
                log::error!(?error, "Failed to build PoS genesis");
                std::process::exit(1);
            }
        };

    log::info!(filename = args.file, "Writing PoS genesis to file");
    if let Err(error) = write_pos_genesis(&args.file, genesis_config) {
        log::error!(?error, "Could not write genesis config file");
        std::process::exit(1);
    }
    let duration = start.elapsed();
    log::info!(
        duration = humantime::format_duration(duration).to_string(),
        filename = args.file,
        "Finished writing PoS genesis to file"
    );
}
