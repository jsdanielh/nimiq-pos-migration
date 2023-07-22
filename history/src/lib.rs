use nimiq_blockchain::HistoryStore;
use nimiq_database::{
    traits::{Database, WriteTransaction},
    volatile::VolatileDatabase,
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
        Account, Block, TransactionDetails as PoWTransaction,
        TransactionSequence as PoWTransactionSequence,
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

fn get_account_type(client: &Client, address: &str) -> Result<AccountType, Error> {
    let account = client.get_account(address)?;
    let pow_account_type = match account {
        Account::Basic(account) => account.r#type,
        Account::Vesting(account) => account.r#type,
        Account::HTLC(account) => account.r#type,
    };
    match pow_account_type {
        0u8 => Ok(AccountType::Basic),
        1u8 => Ok(AccountType::Vesting),
        2u8 => Ok(AccountType::HTLC),
        _ => Err(Error::InvalidValue),
    }
}

fn from_pow_transaction(
    client: &Client,
    pow_transaction: &PoWTransaction,
) -> Result<Transaction, Error> {
    let sender = Address::from_user_friendly_address(&pow_transaction.from_address)?;
    let sender_type = get_account_type(client, &pow_transaction.from_address)?;
    let recipient = Address::from_user_friendly_address(&pow_transaction.to_address)?;
    let recipient_type = get_account_type(client, &pow_transaction.to_address)?;
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
        recipient,
        recipient_type,
        value,
        fee,
        data,
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
pub fn get_history_root(client: &Client, cutting_pow_block: Block) -> Result<String, Error> {
    let env = VolatileDatabase::new(20).unwrap();
    let history_store = HistoryStore::new(env.clone());
    let mut txn = env.write_transaction();
    for block_height in 1..cutting_pow_block.number {
        let mut transactions = vec![];
        let block = client.get_block_by_number(block_height, false)?;
        let mut network_id = NetworkId::Main;
        match block.transactions {
            PoWTransactionSequence::BlockHashes(hashes) => {
                for hash in hashes {
                    let pow_transaction = client.get_transaction_by_hash(&hash)?;
                    let pos_transaction = from_pow_transaction(client, &pow_transaction)?;
                    network_id = pos_transaction.network_id;

                    assert_eq!(
                        pow_transaction.block_hash,
                        pos_transaction.hash::<Blake2bHash>().to_hex()
                    );
                    transactions.push(ExecutedTransaction::Ok(pos_transaction));
                }
            }
            PoWTransactionSequence::Transactions(_) => panic!("Unexpected transaction type"),
        }
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
    }
    txn.commit();
    history_store
        .get_history_tree_root(0, None)
        .ok_or(Error::HistoryRootError)
        .map(|hash| hash.to_hex())
}
