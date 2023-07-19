use log::info;
use nimiq_rpc::Client;
use simple_logger::SimpleLogger;
use std::{thread::sleep, time::Duration};

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
fn _generate_ready_tx() {
    let _tx = nimiq_rpc::primitives::OutgoingTransaction {
        from: "Any address".to_string(),
        to: "Validator address".to_string(),
        value: 100, //Lunas
        fee: 0,
        //data: "signature(address)"
    };
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
}
