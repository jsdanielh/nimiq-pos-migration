pub mod types;
use percentage::Percentage;
use std::{collections::HashMap, ops::Range};

use log::{error, info};
use nimiq_keys::Address;
use nimiq_primitives::coin::Coin;
use nimiq_rpc::{
    primitives::{OutgoingTransaction, TransactionDetails},
    Client,
};
use nimiq_state_migration::types::GenesisValidator;

use types::{Error, ValidatorsReadiness, ACTIVATION_HEIGHT};

/// Stake percentage that is considered to indicate that the validators are ready
pub const READY_PERCENTAGE: u8 = 80;

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
    OutgoingTransaction {
        from: validator,
        to: Address::burn_address().to_user_friendly_address(),
        value: 1, //Lunas
        fee: 0,
    }
}

// Checks if we have seen a ready transaction from a validator in the specified range
pub async fn get_ready_txns(
    client: &Client,
    validator: String,
    block_window: Range<u32>,
) -> Vec<TransactionDetails> {
    if let Ok(transactions) = client.get_transactions_by_address(&validator, 10).await {
        let filtered_txns: Vec<TransactionDetails> = transactions
            .into_iter()
            .filter(|txn| {
                // Here we filter by current epoch
                (txn.block_number > block_window.start)
                    && (txn.block_number < block_window.end)
                    && (txn.to_address == Address::burn_address().to_user_friendly_address())
                    && txn.value == 1
            })
            .collect();
        filtered_txns
    } else {
        Vec::new()
    }
}

// Sends a transaction into the Nimiq PoW chain
pub async fn send_tx(client: &Client, transaction: OutgoingTransaction) -> Result<(), Error> {
    match client.send_transaction(&transaction).await {
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
// The validators_allocation is a HashMap from Validator to number of slots owned by that validator
pub async fn check_validators_ready(
    client: &Client,
    validators: Vec<GenesisValidator>,
) -> ValidatorsReadiness {
    // First calculate the total amount of stake
    let total_stake: Coin = validators.iter().map(|validator| validator.balance).sum();

    log::debug!(" The total registered stake is {}", total_stake);

    // First we need to obtain the validator list, along with the slot allocation for the first epoch.
    let mut validator_list = HashMap::new();

    // This is a mock list for testing purposes(for now)
    // The validator address and the slots assigned to each address
    validator_list.insert(
        "NQ28 GSPY V07Q DJTK Y8TG DFYD KR5Q 9KBF HV5A".to_string(),
        100u16,
    );

    validator_list.insert(
        "NQ56 7L0M GQPS GNCU VGGT LV4S 4HHN F701 2DEF".to_string(),
        412u16,
    );

    let mut ready_validators = Vec::new();

    log::info!("Starting to collect transactions from validators...");

    // Now we need to collect all the transations for each validator
    for validator in validators {
        let address = validator
            .validator
            .validator_address
            .to_user_friendly_address();
        if let Ok(transactions) = client.get_transactions_by_address(&address, 10).await {
            info!(
                "There are {} transactions from {}",
                transactions.len(),
                address
            );
            // We only keep the ones past the activation window that met the activation criteria
            let filtered_txns: Vec<TransactionDetails> = transactions
                .into_iter()
                .filter(|txn| {
                    // Here we filter by the readiness criteria, TBD
                    (txn.block_number > ACTIVATION_HEIGHT)
                        && (txn.to_address == Address::burn_address().to_user_friendly_address())
                        && txn.value == 1
                })
                .collect();
            info!(
                "Transactions that met the readiness criteria: {}",
                filtered_txns.len()
            );
            if !filtered_txns.is_empty() {
                ready_validators.push(validator);
            }
        }
    }

    // Now we need to see if we have enough stake ready
    let mut ready_stake = Coin::ZERO;

    for ready_validator in ready_validators {
        ready_stake += ready_validator.balance;

        info!(
            " Validator {} is ready with {} stake.",
            ready_validator
                .validator
                .validator_address
                .to_user_friendly_address(),
            ready_validator.balance
        );
    }

    info!(" We have {} total stake ready", u64::from(ready_stake));
    let percent = Percentage::from(READY_PERCENTAGE);

    let needed_stake = percent.apply_to(u64::from(total_stake));

    info!(" We need at least {} stake to be ready", needed_stake);

    if u64::from(ready_stake) >= needed_stake {
        info!(" Enough validators are ready to start the PoS Chain! ");
        ValidatorsReadiness::Ready(ready_stake)
    } else {
        info!(
            " Not enough validators are ready, we need at least {} stake ",
            needed_stake
        );
        ValidatorsReadiness::NotReady(ready_stake)
    }
}
