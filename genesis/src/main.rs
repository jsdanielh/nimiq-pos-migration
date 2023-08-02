use std::{path::Path, time::Instant};

use clap::Parser;
use log::level_filters::LevelFilter;
use nimiq_database::mdbx::MdbxDatabase;
use nimiq_rpc::Client;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use genesis_migration::{get_pos_genesis, types::PoWRegistrationWindow, write_pos_genesis};

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

    /// Block number of the block that starts the validator registration window
    #[arg(short, long)]
    validator_start_hash: u32,

    /// Block number of the block that starts the pre-stake registration window
    #[arg(short, long)]
    prestake_start_hash: u32,

    /// Block number of the block that ends the pre-stake registration window
    #[arg(short, long)]
    prestake_end_hash: u32,

    /// Hash of the block will be taken as the genesis block for the PoS chain
    #[arg(short, long)]
    final_hash: String,

    /// VrfSeed
    #[arg(short, long)]
    vrf: String,

    /// Genesis delay in minutes
    #[arg(short, long)]
    confirmations: u32,

    /// TOML output file name
    #[arg(short, long)]
    db_path: String,

    /// Set to true for testnet usage
    #[arg(short, long)]
    testnet: bool,
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
    let pow_registration_window = PoWRegistrationWindow {
        pre_stake_start: args.prestake_start_hash,
        pre_stake_end: args.prestake_end_hash,
        validator_start: args.validator_start_hash,
        final_block: args.final_hash,
        confirmations: args.confirmations,
    };

    // Create DB environment
    let network_id = if args.testnet { "test" } else { "main" };
    let db_name = format!("{network_id}-history-consensus").to_lowercase();
    let db_path = Path::new(&args.db_path).join(db_name);
    let env = match MdbxDatabase::new_with_max_readers(
        db_path.clone(),
        100 * 1024 * 1024 * 1024,
        20,
        600,
    ) {
        Ok(db) => db,
        Err(e) => {
            log::error!(error = ?e, "Failed to create database");
            std::process::exit(1);
        }
    };

    log::info!("Generating genesis configuration from PoW chain");
    let start = Instant::now();
    let genesis_config = match get_pos_genesis(&client, &pow_registration_window, &vrf_seed, env) {
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
