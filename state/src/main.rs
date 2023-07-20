use clap::Parser;
use nimiq_rpc::Client;

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

fn main() {
    let args = Args::parse();
    let client = Client::new(args.rpc);
    let genesis_config =
        get_pos_genesis(client, args.hash, args.height).expect("Failed to produce genesis config");
    log::info!(filename = args.file, "Writing PoS genesis to file");
    write_pos_genesis(&args.file, genesis_config).expect("Could not write genesis config file");
    log::info!(filename = args.file, "Finished writing PoS genesis to file");
}
