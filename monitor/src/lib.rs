pub mod types;

use std::collections::HashMap;

use log::{error, info};
use nimiq_primitives::policy::Policy;
use nimiq_rpc::{
    primitives::{OutgoingTransaction, TransactionDetails},
    Client,
};

use types::{Error, ValidatorsReadiness, ACTIVATION_HEIGHT, BURN_ADDRESS};

// Sends a transaction to the Nimiq PoW chain to report that we are ready
// The transaction format is defined as follow:
//   Sender: Validator address
//   Recipient: Burn address
//   Value: 100 Lunas
//   Data: TBD
//
//
pub fn generate_ready_tx(validator: String) -> OutgoingTransaction {
    info!(" Generating ready transaction, from {} ", validator);
    let tx = OutgoingTransaction {
        from: validator,
        to: BURN_ADDRESS.to_string(),
        value: 1, //Lunas
        fee: 0,
    };

    tx
}

// Checks if we have seen a ready transaction from a validator in the specified range
pub fn get_ready_txns(
    client: &Client,
    validator: String,
    start_block: u32,
    end_block: u32,
) -> Vec<TransactionDetails> {
    if let Ok(transactions) = client.get_transactions_by_address(&validator, 10) {
        let filtered_txns: Vec<TransactionDetails> = transactions
            .into_iter()
            .filter(|txn| {
                // Here we filter by current epoch
                (txn.block_number > start_block)
                    && (txn.block_number < end_block)
                    && (txn.to_address == BURN_ADDRESS.to_string())
                    && txn.value == 1
            })
            .collect();
        filtered_txns
    } else {
        Vec::new()
    }
}

// Sends a transaction into the Nimiq PoW chain
pub fn send_tx(client: &Client, transaction: OutgoingTransaction) -> Result<(), Error> {
    match client.send_transaction(&transaction) {
        Ok(_) => {
            info!(" Sent transaction to the Nimiq PoW network");
            Ok(())
        }
        Err(err) => {
            error!(" Failed sending transaction, error: {}", err);
            Err(Error::Rpc)
        }
    }
}

// Checks if enough validators are ready
// If thats the case, the number of slots which are ready are returned
pub fn check_validators_ready(client: &Client) -> ValidatorsReadiness {
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
            let filtered_txns: Vec<TransactionDetails> = transactions
                .into_iter()
                .filter(|txn| {
                    // Here we filter by the readiness criteria, TBD
                    (txn.block_number > ACTIVATION_HEIGHT)
                        && (txn.to_address == BURN_ADDRESS.to_string())
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
