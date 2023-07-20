use log::{error, info};
use nimiq_rpc::{
    primitives::{Address, OutgoingTransaction, Transaction},
    Client,
};
use simple_logger::SimpleLogger;
use std::{collections::HashMap, process::exit, thread::sleep, time::Duration};

const ACTIVATION_HEIGHT: u64 = 100;

pub enum ValidatorsReadiness {
    NotReady,
    Ready(u32),
}

// Sends a transaction to the Nimiq PoW chain to report that we are ready
// The transaction format is defined as follow:
//   Sender: Any address which has the minium balance needed to send this txn
//           This address needs to be imported into the RPC server wallet.
//   Recipient: The Validator address.
//              This implies that these funds can be later recovered.
//   Value: 100 Lunas
//   Data: This field is used to prove that the validator owner is the one who is
//         sending this transaction. To prove its identity, it needs to sign the
//         validator address using his private key. In this way, when we check for
//         validator readiness we know that the validator owner is ready
//
// It is possible to signal that a validator is no longer ready by reversing this transaction:
// Removing all funds from the validator address acount.
//
fn generate_ready_tx() -> OutgoingTransaction {
    let tx = OutgoingTransaction {
        from: "Any address".to_string(),
        to: "Validator address".to_string(),
        value: 100, //Lunas
        fee: 0,
        //data: "signature(address)"
    };

    tx
}

// Checks if enough validators are ready
// If thats the case, the next election block candidate is returned
fn check_validators_ready(client: &Client) -> ValidatorsReadiness {
    // First we need to obtain the validator list for the first epoch.
    let validator_list: Vec<Address> = Vec::new();
    let mut validator_transactions = HashMap::new();

    // Now we need to collect all the transations for each validator
    for validator in validator_list {
        if let Ok(transactions) = client.get_transactions_by_address(&validator.address, 10) {
            // We only keep the ones past the activation window
            let filtered_txns: Vec<Transaction> = transactions
                .into_iter()
                .filter(|txn| txn.block_number > ACTIVATION_HEIGHT)
                .collect();
            // We also need to verify that ech txn was signed by each validator owner
            validator_transactions.insert(validator.address, filtered_txns);
        }
    }

    ValidatorsReadiness::NotReady
}

fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    //let client = Client::new("http://seed1.nimiq-testnet.com:8648/".to_string());
    let client = Client::new("http://127.0.0.1:8648/".to_string());

    let mut consensus_established = false;

    while !consensus_established {
        let status = client.consensus().unwrap();
        if status.eq("established") {
            info!(" Consensus is established");
            consensus_established = true;
        }
        info!(" Consensus has not been established yet..");
        info!(" Current block height: {}", client.block_number().unwrap());
        sleep(Duration::from_secs(10));
    }

    //Report we are ready to the Nimiq PoW chain:
    let transaction = generate_ready_tx();

    match client.send_transaction(&transaction) {
        Ok(_) => info!(" Sent ready transaction to the Nimiq PoW network"),
        Err(err) => {
            error!(" Failed sending ready transaction {}", err);
            exit(1);
        }
    }

    let mut validators_ready = false;

    while !validators_ready {
        let validators_status = check_validators_ready(&client);
        match validators_status {
            ValidatorsReadiness::NotReady => {
                info!("Validators are not ready yet");
                validators_ready = false;
                sleep(Duration::from_secs(10));
                continue;
            }
            ValidatorsReadiness::Ready(block_number) => info!(
                " We found a potential election block candidate {}",
                block_number
            ),
        }
    }
}
