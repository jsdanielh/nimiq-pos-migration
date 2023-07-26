pub mod types;

use std::{fs, str::FromStr, time::Duration, vec};

use hex::FromHexError;
use nimiq_genesis_builder::config::{
    GenesisAccount, GenesisConfig, GenesisHTLC, GenesisValidator, GenesisVestingContract,
};
use nimiq_hash::Blake2bHash;
use nimiq_keys::{Address, AddressParseError};
use nimiq_primitives::coin::{Coin, CoinConvertError};
use nimiq_rpc::{
    primitives::{
        BasicAccount as PoWBasicAccount, Block, HTLCAccount as PoWHTLCAccount,
        VestingAccount as PoWVestingAccount,
    },
    Client,
};
use nimiq_transaction::account::htlc_contract::{AnyHash, AnyHash32, AnyHash64};
use nimiq_vrf::VrfSeed;
use thiserror::Error;
use time::{error::ComponentRange, OffsetDateTime};

use crate::types::GenesisAccounts;

// POW estimated block time in milliseconds
const POW_BLOCK_TIME_MS: u64 = 60 * 1000; // 1 min

/// Error types that can be returned
#[derive(Error, Debug)]
pub enum Error {
    /// RPC error
    #[error("RPC error: {0}")]
    Rpc(#[from] jsonrpc::Error),
    /// Unknown PoW block
    #[error("Unknown PoW block")]
    UnknownBlock,
    /// IO error
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    /// Serialization error
    #[error("Serialization: {0}")]
    Serialization(#[from] toml::ser::Error),
    /// Address parsing error
    #[error("Failed to parse Nimiq address")]
    Address(#[from] AddressParseError),
    /// Coin conversion error
    #[error("Failed to convert to coin")]
    Coin(#[from] CoinConvertError),
    /// Hex conversion error
    #[error("Failed to decode string as hex")]
    Hex(#[from] FromHexError),
    /// Invalid value
    #[error("Invalid value")]
    InvalidValue,
    /// Invalid time
    #[error("Invalid timestamp")]
    Timestamp(#[from] ComponentRange),
}

fn pos_basic_account_from_account(pow_account: &PoWBasicAccount) -> Result<GenesisAccount, Error> {
    let address = Address::from_user_friendly_address(&pow_account.address)?;
    let balance = Coin::try_from(pow_account.balance)?;
    Ok(GenesisAccount { address, balance })
}

fn pos_vesting_account_from_account(
    pow_account: &PoWVestingAccount,
    cutting_block: &Block,
    pos_genesis_ts: u64,
) -> Result<GenesisVestingContract, Error> {
    let owner = Address::from_user_friendly_address(&pow_account.owner_address)?;
    let address = Address::from_user_friendly_address(&pow_account.address)?;
    let balance = Coin::try_from(pow_account.balance)?;
    let start_time = if pow_account.vesting_start <= cutting_block.number {
        cutting_block.timestamp as u64
    } else {
        (pow_account.vesting_start - cutting_block.number) as u64 * POW_BLOCK_TIME_MS
            + pos_genesis_ts
    };
    let time_step = pow_account.vesting_step_blocks as u64 * POW_BLOCK_TIME_MS;
    let step_amount = Coin::try_from(pow_account.vesting_step_amount)?;
    let total_amount = Coin::try_from(pow_account.vesting_total_amount)?;
    Ok(GenesisVestingContract {
        address,
        owner,
        balance,
        start_time,
        step_amount,
        time_step,
        total_amount,
    })
}

fn pos_htlc_account_from_account(
    pow_account: &PoWHTLCAccount,
    cutting_block: &Block,
    pos_genesis_ts: u64,
) -> Result<GenesisHTLC, Error> {
    let address = Address::from_user_friendly_address(&pow_account.address)?;
    let recipient = Address::from_user_friendly_address(&pow_account.recipient_address)?;
    let sender = Address::from_user_friendly_address(&pow_account.sender_address)?;
    let balance = Coin::try_from(pow_account.balance)?;
    let hash_count = pow_account.hash_count;
    let timeout = if pow_account.timeout <= cutting_block.number {
        cutting_block.timestamp as u64
    } else {
        (pow_account.timeout - cutting_block.number) as u64 * POW_BLOCK_TIME_MS + pos_genesis_ts
    };
    let total_amount = Coin::try_from(pow_account.total_amount)?;
    let hash_root = pos_anyhash_from_hash_root(&pow_account.hash_root, pow_account.hash_algorithm)?;
    Ok(GenesisHTLC {
        address,
        recipient,
        sender,
        balance,
        hash_root,
        hash_count,
        timeout,
        total_amount,
    })
}

fn pos_anyhash_from_hash_root(hash_root: &str, algorithm: u8) -> Result<AnyHash, Error> {
    match algorithm {
        1u8 => Ok(AnyHash::Blake2b(AnyHash32::from_str(hash_root)?)),
        3u8 => Ok(AnyHash::Sha256(AnyHash32::from_str(hash_root)?)),
        4u8 => Ok(AnyHash::Sha512(AnyHash64::from_str(hash_root)?)),
        _ => Err(Error::InvalidValue),
    }
}

/// Gets the PoS genesis history root by getting all of the transactions from the
/// PoW chain and building a single history tree.
pub fn get_accounts(
    client: Client,
    cutting_block: &Block,
    pos_genesis_ts: u64,
) -> Result<GenesisAccounts, Error> {
    let mut genesis_accounts = GenesisAccounts {
        vesting_accounts: vec![],
        basic_accounts: vec![],
        htlc_accounts: vec![],
    };
    let mut start_prefix = "".to_string();
    loop {
        let chunk = client.get_accounts_tree_chunk(&cutting_block.hash, &start_prefix)?;
        if chunk.nodes.is_empty() || start_prefix == chunk.tail {
            break;
        }
        start_prefix = chunk.tail;
        log::debug!(size = chunk.nodes.len(), "Processing accounts tree chunk");
        for node in chunk.nodes {
            match node.account {
                nimiq_rpc::primitives::Account::Basic(pow_account) => {
                    let pos_basic_account = pos_basic_account_from_account(&pow_account)?;
                    genesis_accounts.basic_accounts.push(pos_basic_account);
                }
                nimiq_rpc::primitives::Account::Vesting(pow_account) => {
                    let pos_vesting_account = pos_vesting_account_from_account(
                        &pow_account,
                        cutting_block,
                        pos_genesis_ts,
                    )?;
                    genesis_accounts.vesting_accounts.push(pos_vesting_account);
                }
                nimiq_rpc::primitives::Account::HTLC(pow_account) => {
                    let pos_htlc_account =
                        pos_htlc_account_from_account(&pow_account, cutting_block, pos_genesis_ts)?;
                    genesis_accounts.htlc_accounts.push(pos_htlc_account);
                }
            }
        }
    }
    Ok(genesis_accounts)
}

/// Gets the PoS genesis history root by getting all of the transactions from the
/// PoW chain and building a single history tree.
pub fn get_validators(
    client: Client,
    cutting_block: Block,
) -> Result<Vec<GenesisValidator>, Error> {
    unimplemented!();
}

/// Gets the genesis config file
pub fn get_pos_genesis(
    client: Client,
    block_hash: String,
    block_number: u32,
    vrf_seed: &VrfSeed,
    genesis_delay: Duration,
) -> Result<GenesisConfig, Error> {
    // Get block according to arguments and check if it exists
    let cutting_block = client.get_block_by_hash(&block_hash, false)?;
    if cutting_block.number != block_number {
        log::error!(block_number, block_hash, "Could not find provided block");
        return Err(Error::UnknownBlock);
    }
    let pow_genesis = client.get_block_by_number(1, false)?;

    // The PoS genesis timestamp is the cutting block timestamp plus a custom delay
    let pos_genesis_ts = genesis_delay.as_secs() * 1000 + cutting_block.timestamp as u64;
    // The parent election hash of the PoS genesis is the hash of the PoW genesis block
    let parent_election_hash = Blake2bHash::from_str(&pow_genesis.hash)?;
    // The parent hash of the PoS genesis is the hash of cutting block
    let parent_hash = Blake2bHash::from_str(&cutting_block.hash)?;
    let genesis_accounts = get_accounts(client, &cutting_block, pos_genesis_ts)?;

    Ok(GenesisConfig {
        seed_message: Some("Albatross TestNet".to_string()),
        vrf_seed: Some(vrf_seed.clone()),
        parent_election_hash: Some(parent_election_hash),
        parent_hash: Some(parent_hash),
        timestamp: Some(OffsetDateTime::from_unix_timestamp(pos_genesis_ts as i64)?),
        validators: [].to_vec(),
        stakers: [].to_vec(),
        basic_accounts: genesis_accounts.basic_accounts,
        vesting_accounts: genesis_accounts.vesting_accounts,
        htlc_accounts: genesis_accounts.htlc_accounts,
    })
}

/// Write the genesis config file to a TOML file
pub fn write_pos_genesis(file_path: &str, genesis_config: GenesisConfig) -> Result<(), Error> {
    Ok(fs::write(file_path, toml::to_string(&genesis_config)?)?)
}
