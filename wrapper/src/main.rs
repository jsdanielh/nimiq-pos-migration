use clap::Parser;
use jsonrpc::serde_json;
use log::info;
use log::level_filters::LevelFilter;
use nimiq_database::mdbx::MdbxDatabase;
use nimiq_genesis_migration::{get_pos_genesis, types::PoWRegistrationWindow, write_pos_genesis};
use nimiq_pow_monitor::{
    check_validators_ready, generate_ready_tx, get_ready_txns, send_tx,
    types::{ValidatorsReadiness, ACTIVATION_HEIGHT},
};
use nimiq_primitives::policy::Policy;
use nimiq_rpc::Client;
use nimiq_state_migration::{get_stakers, get_validators};
use std::{path::Path, process::exit, thread::sleep, time::Duration};
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};
use url::Url;

/// Command line arguments for the binary
/// //TODO: Move this to a configuration file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// RPC connection URL to use
    #[arg(short, long)]
    rpc: String,

    /// The validator address
    #[arg(short, long)]
    validator: String,

    /// VrfSeed
    #[arg(long)]
    vrf: String,

    /// TOML output file name
    #[arg(short, long)]
    db_path: String,

    /// TOML output file name
    #[arg(short, long)]
    file: String,
}

/// TODO This should be specified in a configuration file?
const VALIDATOR_REGISTRATION_START: u32 = 2590000;
const VALIDATOR_REGISTRATION_END: u32 = 2660000;
const PRE_STAKE_START: u32 = VALIDATOR_REGISTRATION_END + 1;
const PRE_STAKE_END: u32 = PRE_STAKE_START + 2000;
const BLOCK_CONFIRMATIONS: u32 = 10;

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
    let validator_address = args.validator.clone();

    info!(" This is our validator address: {}", validator_address);

    let vrf_seed = match serde_json::from_str(&format!(r#""{}""#, args.vrf)) {
        Ok(value) => value,
        Err(_) => {
            log::error!("Invalid VRF seed");
            exit(1);
        }
    };

    let url = match Url::parse(&args.rpc) {
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
    if client.block_number().await.unwrap() < PRE_STAKE_END + BLOCK_CONFIRMATIONS {
        log::error!("This tool is intended to be used during the activation period");
        exit(1);
    }

    // First we obtain the list of registered validators
    let registered_validators = match get_validators(
        &client,
        VALIDATOR_REGISTRATION_START..VALIDATOR_REGISTRATION_END,
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
    let (_, validators) = match get_stakers(
        &client,
        &registered_validators,
        PRE_STAKE_START..PRE_STAKE_END,
    )
    .await
    {
        Ok((stakers, validators)) => (stakers, validators),
        Err(err) => {
            log::error!("Error {} obtaining the list of stakers ", err);
            exit(1)
        }
    };

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
            let transactions = get_ready_txns(
                &client,
                validator_address.clone(),
                previous_election_block..next_election_block,
            )
            .await;

            if transactions.is_empty() {
                // Report we are ready to the Nimiq PoW chain:
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
        if client.block_number().await.unwrap() >= candidate + BLOCK_CONFIRMATIONS {
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
        pre_stake_start: PRE_STAKE_START,
        pre_stake_end: PRE_STAKE_END,
        validator_start: VALIDATOR_REGISTRATION_START,
        final_block: block.hash,
        confirmations: BLOCK_CONFIRMATIONS,
    };

    // Create DB environment

    // TODO: move this to a configuration file
    let network_id = "test";
    let db_name = format!("{network_id}-history-consensus").to_lowercase();
    let db_path = Path::new(&args.db_path).join(db_name);
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

    let genesis_config =
        match get_pos_genesis(&client, &pow_registration_window, &vrf_seed, env).await {
            Ok(config) => config,
            Err(err) => {
                log::error!("Failed to build PoS genesis: {}", err);
                exit(1);
            }
        };

    if let Err(err) = write_pos_genesis(&args.file, genesis_config) {
        log::error!("Could not write genesis config file: {}", err);
        exit(1);
    }
    // Start the nimiq 2.0 client with the generated genesis file
}
