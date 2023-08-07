use clap::Parser;
use log::info;
use nimiq_pow_monitor::{
    check_validators_ready, generate_ready_tx, get_ready_txns, send_tx,
    types::{ValidatorsReadiness, ACTIVATION_HEIGHT},
};
use nimiq_primitives::policy::Policy;
use nimiq_rpc::Client;
use simple_logger::SimpleLogger;
use std::{process::exit, thread::sleep, time::Duration};

/// Command line arguments for the binary
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// RPC connection URL to use
    #[arg(short, long)]
    rpc: String,

    /// The validator address
    #[arg(short, long)]
    validator: String,
}

fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let args = Args::parse();
    let validator_address = args.validator.clone();

    info!(" This is our validator address: {}", validator_address);

    let client = Client::new(&args.rpc);

    loop {
        let status = client.consensus().unwrap();
        if status.eq("established") {
            info!(" Consensus is established");
            break;
        }
        info!(" Consensus has not been established yet..");
        info!(" Current block height: {}", client.block_number().unwrap());
        sleep(Duration::from_secs(10));
    }

    let mut reported_ready = false;
    loop {
        let current_height = client.block_number().unwrap();
        info!(" Current block height: {}", current_height);

        let next_election_block = Policy::election_block_after(current_height);
        let mut previous_election_block = Policy::election_block_before(current_height);

        if previous_election_block < ACTIVATION_HEIGHT {
            previous_election_block = ACTIVATION_HEIGHT;
        }

        if !reported_ready {
            // Obtain all the transactions that we have sent previously.
            let transactions = get_ready_txns(
                &client,
                validator_address.clone(),
                previous_election_block,
                next_election_block,
            );

            if transactions.is_empty() {
                // Report we are ready to the Nimiq PoW chain:
                let transaction = generate_ready_tx(validator_address.clone());

                match send_tx(&client, transaction) {
                    Ok(_) => reported_ready = true,
                    Err(_) => exit(1),
                }
            } else {
                log::info!(" We found a ready transaction from our validator");
                reported_ready = true;
            }
        }

        let validators_status = check_validators_ready(&client);
        match validators_status {
            ValidatorsReadiness::NotReady(slots) => {
                info!(
                    "Not enough validators are ready yet, we have {} slots ready",
                    slots
                );
            }
            ValidatorsReadiness::Ready(slots) => {
                info!(
                    "Enough validators are ready to start the PoS chain, we have {} slots ready",
                    slots
                );
                break;
            }
        }

        sleep(Duration::from_secs(60));

        if next_election_block != Policy::election_block_after(client.block_number().unwrap()) {
            reported_ready = false;
        }
    }

    // Now that we have enough validators ready, we need to pick the next election block candidate

    let candidate = Policy::election_block_after(client.block_number().unwrap());

    info!("The next election candidate is {}", candidate);

    loop {
        if candidate <= client.block_number().unwrap() {
            info!("We are ready to start the migration process.. ");
            break;
        } else {
            info!(
                "Election candidate {}, current height {}",
                candidate,
                client.block_number().unwrap()
            );
            sleep(Duration::from_secs(100));
        }
    }
}
