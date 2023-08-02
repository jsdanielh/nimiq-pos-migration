use std::fmt::Write;

use indicatif::{HumanDuration, ProgressBar, ProgressState, ProgressStyle};
use nimiq_blockchain::HistoryStore;
use nimiq_database::{
    traits::{Database, WriteTransaction},
    DatabaseProxy,
};
use nimiq_hash::{Blake2bHash, Hash};
use nimiq_keys::{Address, AddressParseError};
use nimiq_primitives::{
    account::AccountType,
    coin::{Coin, CoinConvertError},
    networks::NetworkId,
};
use nimiq_rpc::{
    primitives::{
        Block, TransactionDetails as PoWTransaction, TransactionSequence as PoWTransactionSequence,
    },
    Client,
};
use nimiq_transaction::{
    extended_transaction::ExtendedTransaction, ExecutedTransaction, Transaction, TransactionFlags,
};
use thiserror::Error;

/// Error types that can be returned
#[derive(Error, Debug)]
pub enum Error {
    /// RPC error
    #[error("RPC error: {0}")]
    Rpc(#[from] jsonrpc::Error),
    /// Unknown PoW block
    #[error("Unknown PoW block")]
    UnknownBlock,
    /// Address parsing error
    #[error("Failed to parse Nimiq address")]
    Address(#[from] AddressParseError),
    /// Coin conversion error
    #[error("Failed to convert to coin")]
    Coin(#[from] CoinConvertError),
    /// Hex decoding error
    #[error("Failed to decode HEX string")]
    Hex(#[from] hex::FromHexError),
    /// Invalid value
    #[error("Invalid value")]
    InvalidValue,
    /// Error calculating history root
    #[error("History root error")]
    HistoryRootError,
}

fn from_pow_network_id(pow_network_id: u8) -> Result<NetworkId, Error> {
    match pow_network_id {
        1u8 => Ok(NetworkId::Test),
        42u8 => Ok(NetworkId::Main),
        _ => Err(Error::InvalidValue),
    }
}

fn get_account_type(pow_account_type: &u8) -> Result<AccountType, Error> {
    match pow_account_type {
        0u8 => Ok(AccountType::Basic),
        1u8 => Ok(AccountType::Vesting),
        2u8 => Ok(AccountType::HTLC),
        _ => Err(Error::InvalidValue),
    }
}

fn from_pow_transaction(pow_transaction: &PoWTransaction) -> Result<Transaction, Error> {
    let sender = Address::from_user_friendly_address(&pow_transaction.from_address)?;
    let sender_type = get_account_type(&pow_transaction.from_type)?;
    let recipient = Address::from_user_friendly_address(&pow_transaction.to_address)?;
    let recipient_type = get_account_type(&pow_transaction.to_type)?;
    let value = Coin::try_from(pow_transaction.value)?;
    let fee = Coin::try_from(pow_transaction.fee)?;
    let data = if let Some(data) = &pow_transaction.data {
        hex::decode(data)?
    } else {
        [].to_vec()
    };
    // FixMe
    //let validity_start_height = u32::try_from(pow_transaction.valid)
    let validity_start_height = pow_transaction.validity_start_height;
    let network_id = from_pow_network_id(pow_transaction.network_id)?;
    let mut tx = Transaction::new_extended(
        sender,
        sender_type,
        [].to_vec(),
        recipient,
        recipient_type,
        data,
        value,
        fee,
        validity_start_height,
        network_id,
    );
    if let Some(proof) = &pow_transaction.proof {
        tx.proof = hex::decode(proof)?;
    }
    tx.flags =
        TransactionFlags::try_from(pow_transaction.flags).map_err(|_| Error::InvalidValue)?;
    Ok(tx)
}

/// Gets the PoS genesis history root by getting all of the transactions from the
/// PoW chain and building a single history tree.
pub fn get_history_root(
    client: &Client,
    cutting_pow_block: Block,
    env: DatabaseProxy,
) -> Result<Blake2bHash, Error> {
    let history_store = HistoryStore::new(env.clone());

    // Setup progress bar
    let pb = ProgressBar::new(cutting_pow_block.number as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] Block: {pos}, {percent}% (~{eta} remaining)",
        )
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
            write!(w, "{}", HumanDuration(state.eta())).unwrap()
        })
        .progress_chars("#>-"),
    );

    // Now get transactions of each block and add it to the PoS history store
    for block_height in 1..cutting_pow_block.number {
        // Fixme: This is currently not supported as it uses epoch_at from the block_height
        //if !history_store
        //    .get_block_transactions(block_height, None)
        //    .is_empty()
        //{
        //    continue;
        //};
        let mut transactions = vec![];
        let block = client.get_block_by_number(block_height, false)?;
        let mut network_id = NetworkId::Main;
        match block.transactions {
            PoWTransactionSequence::BlockHashes(hashes) => {
                if hashes.is_empty() {
                    continue;
                }
                for hash in hashes {
                    log::trace!(hash, "Processing transaction");
                    let pow_transaction = client.get_transaction_by_hash(&hash)?;
                    let pos_transaction = from_pow_transaction(&pow_transaction)?;
                    network_id = pos_transaction.network_id;

                    assert_eq!(
                        pow_transaction.hash,
                        pos_transaction.hash::<Blake2bHash>().to_hex()
                    );
                    transactions.push(ExecutedTransaction::Ok(pos_transaction));
                }
            }
            PoWTransactionSequence::Transactions(_) => panic!("Unexpected transaction type"),
        }
        let mut txn = env.write_transaction();
        history_store.add_to_history(
            &mut txn,
            0,
            &ExtendedTransaction::from(
                network_id,
                block_height,
                block.timestamp.into(),
                transactions,
                [].to_vec(),
            ),
        );
        txn.commit();
        pb.set_position(block_height as u64);
    }
    history_store
        .get_history_tree_root(0, None)
        .ok_or(Error::HistoryRootError)
}
