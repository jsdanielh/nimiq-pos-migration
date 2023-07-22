use clap::Parser;
use nimiq_rpc::Client;

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

fn main() {
    let args = Args::parse();
    let client = Client::new(args.rpc);

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
    match get_history_root(&client, block) {
        Ok(history_root) => log::info!(history_root, "Finished building history tree"),
        Err(e) => {
            log::error!(error = ?e, "Failed to build history root");
            std::process::exit(1);
        }
    }
}
