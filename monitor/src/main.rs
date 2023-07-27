use clap::Parser;
use log::{error, info};
use nimiq_primitives::policy::Policy;
use nimiq_rpc::{
    primitives::{OutgoingTransaction, Transaction},
    Client,
};
use simple_logger::SimpleLogger;
use std::{collections::HashMap, process::exit, thread::sleep, time::Duration};

const ACTIVATION_HEIGHT: u64 = 100;

pub enum ValidatorsReadiness {
    NotReady(u16),
    Ready(u16),
}

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

// Sends a transaction to the Nimiq PoW chain to report that we are ready
// The transaction format is defined as follow:
//   Sender: Validator address
//   Recipient: Burn address
//   Value: 100 Lunas
//   Data: TBD
//
//
fn generate_ready_tx(validator: String) -> OutgoingTransaction {
    info!(" Generating ready transaction, from {} ", validator);
    let tx = OutgoingTransaction {
        from: validator,
        to: "NQ07 0000 0000 0000 0000 0000 0000 0000 0000".to_string(),
        value: 1, //Lunas
        fee: 0,
    };

    tx
}

// Checks if enough validators are ready
// If thats the case, the number of slots which are ready are returned
fn check_validators_ready(client: &Client) -> ValidatorsReadiness {
    // First we need to obtain the validator list, along with the slot allocation for the first epoch.
    let mut validator_list = HashMap::new();

    // This is a mock list for testing purposes(for now)
    // The validator address and the slots assigned to each address
    validator_list.insert(
        "NQ28 GSPY V07Q DJTK Y8TG DFYD KR5Q 9KBF HV5A".to_string(),
        100 as u16,
    );

    validator_list.insert(
        "NQ56 7L0M GQPS GNCU VGGT LV4S 4HHN F701 2DEF".to_string(),
        412 as u16,
    );

    let mut ready_validators = Vec::new();

    log::info!("Starting to collect transactions from validators...");

    // Now we need to collect all the transations for each validator
    for (validator, _slots) in &validator_list {
        if let Ok(transactions) = client.get_transactions_by_address(&validator, 10) {
            info!(
                "There are {} transactions from {}",
                transactions.len(),
                validator
            );
            // We only keep the ones past the activation window that met the activation criteria
            let filtered_txns: Vec<Transaction> = transactions
                .into_iter()
                .filter(|txn| {
                    // Here we filter by the readiness criteria, TBD
                    (txn.block_number > ACTIVATION_HEIGHT)
                        && (txn.to_address
                            == "NQ07 0000 0000 0000 0000 0000 0000 0000 0000".to_string())
                        && txn.value == 1
                })
                .collect();
            info!(
                "Transactions that met the readiness criteria: {}",
                filtered_txns.len()
            );
            if filtered_txns.len() >= 1 {
                ready_validators.push(validator);
            }
        }
    }

    // Now we need to see if 2f+1 validator are ready, in order to select the election block candidate.
    let mut ready_slots = 0;

    for ready_validator in ready_validators {
        let validator_slots = validator_list
            .get(ready_validator)
            .expect("The validator must be present");
        info!(
            " Validator {} is ready with {} slots.",
            ready_validator, validator_slots
        );
        ready_slots += validator_slots;
    }

    info!(" We have {} total slots ready", ready_slots);

    if ready_slots >= Policy::TWO_F_PLUS_ONE {
        info!(" Enough validators are ready to start the PoS Chain! ");
        ValidatorsReadiness::Ready(ready_slots)
    } else {
        info!(
            " Not enough validators are ready, we need at least {} slots ",
            Policy::TWO_F_PLUS_ONE
        );
        ValidatorsReadiness::NotReady(ready_slots)
    }
}

fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    let args = Args::parse();

    let client = Client::new(args.rpc);

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

    //Report we are ready to the Nimiq PoW chain:
    let transaction = generate_ready_tx(args.validator);

    match client.send_transaction(&transaction) {
        Ok(_) => info!(" Sent ready transaction to the Nimiq PoW network"),
        Err(err) => {
            error!(" Failed sending ready transaction {}", err);
            exit(1);
        }
    }

    loop {
        let validators_status = check_validators_ready(&client);
        match validators_status {
            ValidatorsReadiness::NotReady(slots) => {
                info!(
                    "Not enough validators are ready yet, we have {} slots ready",
                    slots
                );
                sleep(Duration::from_secs(10));
                continue;
            }
            ValidatorsReadiness::Ready(slots) => {
                info!(
                    "Enough validators are ready to start the PoS chain, we have {} slots ready",
                    slots
                );
                break;
            }
        }
    }

    // Now that we have enough validators ready, we need to pick the next election block candidate

    let candidate =
        Policy::election_block_after(client.block_number().unwrap().try_into().unwrap()) as u64;

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
