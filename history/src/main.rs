use std::time::Instant;

use clap::Parser;
use log::level_filters::LevelFilter;
use nimiq_rpc::Client;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use history_migration::get_history_root;

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
    let args = Args::parse();
    let client = Client::new(&args.rpc);

    initialize_logging();

    // Get block according to arguments and check if it exists
    let block = client.get_block_by_hash(&args.hash, false).unwrap();
    if block.number != args.height {
        log::error!(
            height = args.height,
            hash = args.hash,
            "Could not find provided block"
        );
        std::process::exit(1);
    }

    log::info!(filename = args.file, "Building history tree");
    let start = Instant::now();
    match get_history_root(&client, block) {
        Ok(history_root) => {
            let duration = start.elapsed();
            log::info!(?duration, history_root, "Finished building history tree")
        }
        Err(e) => {
            log::error!(error = ?e, "Failed to build history root");
            std::process::exit(1);
        }
    }
}
