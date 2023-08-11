use std::{fs, path::Path, process::exit, thread::sleep, time::Duration};

use clap::Parser;
use jsonrpc::serde_json;
use log::info;
use log::level_filters::LevelFilter;
use nimiq_database::mdbx::MdbxDatabase;
use nimiq_genesis_migration::{
    get_pos_genesis,
    types::{PoSRegisteredAgents, PoWRegistrationWindow},
    write_pos_genesis,
};
use nimiq_pow_monitor::{
    check_validators_ready, generate_ready_tx, get_ready_txns, send_tx,
    types::{ValidatorsReadiness, ACTIVATION_HEIGHT},
};
use nimiq_primitives::policy::Policy;
use nimiq_rpc::Client;
use nimiq_state_migration::{get_stakers, get_validators};
use serde::Deserialize;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};
use url::Url;

/// Command line arguments for the binary
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// TOML output file name
    #[arg(short, long)]
    config: String,
}

// Top level struct to hold the TOML data.
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Data {
    rpc_server: RpcServerSettings,
    block_windows: BlockWindows,
    genesis: Genesis,
    files: Files,
    validator: Validator,
}

// Config struct holds to data from the `[config]` section.
#[derive(Deserialize)]
struct RpcServerSettings {
    host: String,
}

// Config struct holds to data from the `[config]` section.
#[derive(Deserialize)]
struct BlockWindows {
    registration_start: u32,
    registration_end: u32,
    pre_stake_start: u32,
    pre_stake_end: u32,
    block_confirmations: u32,
}

#[derive(Deserialize)]
struct Genesis {
    vrf_seed: String,
}

#[derive(Deserialize)]
struct Files {
    db_path: String,
    genesis: String,
}

#[derive(Deserialize)]
struct Validator {
    validator_address: String,
}

fn initialize_logging() {
    let filter = Targets::new().with_default(LevelFilter::DEBUG);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_filter(filter),
        )
        .init();
}

#[tokio::main]
async fn main() {
    //    1 - Use the monitor library to send ready txn and determine if enough validators are ready
    //    2 - Once enough validators are ready we select the next genesis candidate and wait until that block is mined
    //    3 - When the genesis candidate is mined we start the genesis generation process
    //    4 - Monitor the PoW chain to detect if the genesis candidate is forked
    //    5 - After X confirmations, start the 2.0 client with the generated genesis block
    //    6 - If a fork is detected, go back to step 3 and repeat

    initialize_logging();

    let args = Args::parse();

    let contents = match fs::read_to_string(&args.config) {
        Ok(c) => c,

        Err(_) => {
            log::error!("Could not read file `{}`", args.config);
            exit(1);
        }
    };

    let config: Data = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(err) => {
            log::error!("Unable to read TOML from `{}`, error {}", args.config, err);
            exit(1);
        }
    };

    let validator_address = config.validator.validator_address.clone();
    info!(" This is our validator address: {}", validator_address);

    let vrf_seed = match serde_json::from_str(&format!(r#""{}""#, config.genesis.vrf_seed)) {
        Ok(value) => value,
        Err(_) => {
            log::error!("Invalid VRF seed");
            exit(1);
        }
    };

    let url = match Url::parse(&config.rpc_server.host) {
        Ok(url) => url,
        Err(error) => {
            log::error!(?error, "Invalid RPC URL");
            std::process::exit(1);
        }
    };
    let client = Client::new(url);

    loop {
        let status = client.consensus().await.unwrap();
        if status.eq("established") {
            info!(" Consensus is established");

            break;
        }
        info!(" Consensus has not been established yet..");
        info!(
            " Current block height: {}",
            client.block_number().await.unwrap()
        );
        sleep(Duration::from_secs(10));
    }

    // This tool is intended to be used past the pre-stake window
    if client.block_number().await.unwrap()
        < config.block_windows.pre_stake_end + config.block_windows.block_confirmations
    {
        log::error!("This tool is intended to be used during the activation period");
        exit(1);
    }

    // First we obtain the list of registered validators
    let registered_validators = match get_validators(
        &client,
        config.block_windows.registration_start..config.block_windows.registration_end,
    )
    .await
    {
        Ok(validators) => validators,
        Err(err) => {
            log::error!("Error {} obtaining the list of registered validators", err);
            exit(1)
        }
    };

    log::debug!("This is the list of registered validators: ");

    for validator in &registered_validators {
        log::debug!(
            "  Validator Address: {}",
            validator
                .validator
                .validator_address
                .to_user_friendly_address()
        );
    }

    // Now we obtain the stake distribution
    let (stakers, validators) = match get_stakers(
        &client,
        &registered_validators,
        config.block_windows.pre_stake_start..config.block_windows.pre_stake_end,
    )
    .await
    {
        Ok((stakers, validators)) => (stakers, validators),
        Err(err) => {
            log::error!("Error {} obtaining the list of stakers ", err);
            exit(1)
        }
    };

    log::debug!("This is the list of stakers: ");

    for staker in &stakers {
        log::debug!(
            " Staker Address {} Balance {}",
            staker.staker_address,
            staker.balance
        );
    }

    let mut reported_ready = false;
    loop {
        let current_height = client.block_number().await.unwrap();
        info!(" Current block height: {}", current_height);

        let next_election_block = Policy::election_block_after(current_height);
        let mut previous_election_block = Policy::election_block_before(current_height);

        if previous_election_block < ACTIVATION_HEIGHT {
            previous_election_block = ACTIVATION_HEIGHT;
        }

        if !reported_ready {
            // Obtain all the transactions that we have sent previously.
            // TODO: We need to check that this validator is part of the list of the registered validators!
            let transactions = get_ready_txns(
                &client,
                validator_address.clone(),
                previous_election_block..next_election_block,
            )
            .await;

            if transactions.is_empty() {
                log::info!(" We didnt find a ready transaction from our validator in this window");
                log::info!(
                    " Previous election block {}, Next election block {}",
                    previous_election_block,
                    next_election_block
                );
                //Report we are ready to the Nimiq PoW chain:
                let transaction = generate_ready_tx(validator_address.clone());

                match send_tx(&client, transaction).await {
                    Ok(_) => reported_ready = true,
                    Err(_) => exit(1),
                }
            } else {
                log::info!(" We found a ready transaction from our validator");
                reported_ready = true;
            }
        }

        // Check if we have enough validators ready at this point
        let validators_status = check_validators_ready(&client, validators.clone()).await;
        match validators_status {
            ValidatorsReadiness::NotReady(stake) => {
                info!(
                    "Not enough validators are ready yet, we have {} stake ready",
                    u64::from(stake)
                );
            }
            ValidatorsReadiness::Ready(stake) => {
                info!(
                    "Enough validators are ready to start the PoS chain, we have {} stake ready",
                    u64::from(stake)
                );
                break;
            }
        }

        sleep(Duration::from_secs(60));

        // If at this point we have a new nex_election_block, it means that we are in a new epoch, so we need to report we are ready again.
        if next_election_block != Policy::election_block_after(client.block_number().await.unwrap())
        {
            reported_ready = false;
        }
    }

    // Now that we have enough validators ready, we need to pick the next election block candidate
    let candidate = Policy::election_block_after(client.block_number().await.unwrap());

    info!("The next election candidate is {}", candidate);

    loop {
        if client.block_number().await.unwrap()
            >= candidate + config.block_windows.block_confirmations
        {
            info!("We are ready to start the migration process.. ");
            break;
        } else {
            info!(
                "Election candidate {}, current height {}",
                candidate,
                client.block_number().await.unwrap()
            );
            sleep(Duration::from_secs(60));
        }
    }
    // Obtain the genesis candidate block
    let block = client.get_block_by_number(candidate, false).await.unwrap();

    // Start the genesis generation process
    let pow_registration_window = PoWRegistrationWindow {
        pre_stake_start: config.block_windows.pre_stake_start,
        pre_stake_end: config.block_windows.pre_stake_end,
        validator_start: config.block_windows.registration_start,
        final_block: block.hash,
        confirmations: config.block_windows.block_confirmations,
    };

    // Create DB environment

    // TODO: move this to a configuration file
    let network_id = "test";
    let db_name = format!("{network_id}-history-consensus").to_lowercase();
    let db_path = Path::new(&config.files.db_path).join(db_name);
    let env = match MdbxDatabase::new_with_max_readers(
        db_path.clone(),
        100 * 1024 * 1024 * 1024,
        20,
        600,
    ) {
        Ok(db) => db,
        Err(e) => {
            log::error!("Failed to create database, error {}", e);
            exit(1);
        }
    };

    let genesis_config = match get_pos_genesis(
        &client,
        &pow_registration_window,
        &vrf_seed,
        env,
        Some(PoSRegisteredAgents {
            validators,
            stakers,
        }),
    )
    .await
    {
        Ok(config) => config,
        Err(err) => {
            log::error!("Failed to build PoS genesis: {}", err);
            exit(1);
        }
    };

    if let Err(err) = write_pos_genesis(&config.files.genesis, genesis_config) {
        log::error!("Could not write genesis config file: {}", err);
        exit(1);
    }
    // Start the nimiq 2.0 client with the generated genesis file
}
