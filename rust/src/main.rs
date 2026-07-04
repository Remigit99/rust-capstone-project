#![allow(unused)]
use bitcoin::hex::DisplayHex;
use bitcoincore_rpc::bitcoin::{Amount, Txid};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::io::Write;
use std::str::FromStr;

// Node access params
const RPC_URL: &str = "http://127.0.0.1:18443"; // Default regtest RPC port
const RPC_USER: &str = "alice";
const RPC_PASS: &str = "password";

// Helper function to connect to a specific wallet endpoint
fn get_wallet_client(wallet_name: &str) -> bitcoincore_rpc::Result<Client>{
    let url = format!("{}/wallet/{}", RPC_URL, wallet_name);
    Client::new(&url, Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()))
}

// Custom send wrapper using the generic RPC call
fn send(rpc: &Client, addr: &str, amount_btc: f64) -> bitcoincore_rpc::Result<String>{
    // Constructing a proper JSON map for the recipient argument
    let mut dest_map = serde_json::Map::new();
    dest_map.insert(addr.to_string(), json!(amount_btc));

    let args = [
        json!([dest_map]),
        json!(null),
        json!("unset"),
        json!(null),
        json!({"add_inputs": true}),
    ];

    #[derive(Deserialize)]
    struct SendResult {
        complete: bool,
        txid: String,
    }
    let send_result = rpc.call::<SendResult>("send", &args)?;
    assert!(send_result.complete, "Transaction was not complete!");
    Ok(send_result.txid)
}

fn main() -> bitcoincore_rpc::Result<()> {
    // Connect to Bitcoin Core RPC
    let rpc = Client::new(
        RPC_URL,
        Auth::UserPass(RPC_USER.to_owned(), RPC_PASS.to_owned()),
    )?;

    // Get blockchain info
    let blockchain_info = rpc.get_blockchain_info()?;
    println!("Blockchain Info: {:?}", blockchain_info);

    // Create/Load the wallets, named 'Miner' and 'Trader'. 
    // Ignore errors on create_wallet assuming they might exist
    let _ = rpc.create_wallet("Miner", None, None, None, None);
    let _ = rpc.create_wallet("Trader", None, None, None, None);

    //Load the wallets explicitly to ensure they are available
    let _ = rpc.load_wallet("Miner");
    let _ = rpc.load_wallet("Trader");

    // Initialize wallet-specific RPC clients
    let miner_rpc = get_wallet_client("Miner")?;
    let trader_rpc = get_wallet_client("Trader")?;

    // Generate spendable balances in the Miner wallet.
    let miner_address = miner_rpc.get_new_address(Some("Mining Reward"), None)?;

    let checked_miner_address = miner_address.assume_checked();
    println!("Miner Address: {}", checked_miner_address.clone().assume_checked());

    miner_rpc.generate_to_address(101, &Checked_miner_address)?;

    // Print Miner balance
    let miner_balance = miner_rpc.get_balance(None, None)?;
    println!("Miner Wallet Balance: {} BTC", miner_balance.to_btc());


    // Load Trader wallet and generate a new address
    let trader_address = trader_rpc.get_new_address(Some("Received"), None)?;
    let trader_addr_str = trader_address.clone().assume_checked().to_string();
    println!("Trader Address: {}", trader_addr_str);

    // Send 20 BTC from Miner to Trader
    println!("Sending 20 BTC from Miner to Trader...");
    let send_txid_str = Send(&miner_rpc, &trader_addr_str, 20.0)?;
    let txid = Txid::from_str(&send_txid_str).expect("Failed to parse txid");

    // Check transaction in mempool
    let mempool_entry = rpc.call::<serde_json::Value>("getmempoolentry", &[json!(send_txid_str)])?;
    println!("Mempool Entry successfully fetched: \n{:#?}", mempool_entry);

    // Mine 1 block to confirm the transaction
    println!("Mining 1 block to confirm the transaction...");
    miner_rpc.generate_to_address(1, &checked_miner_address)?;

    // Extract all required transaction details
    let raw_tx = miner_rpc.get_raw_transaction_info(&txid, None);

    // 1. Inputs (Miner's Input Address & Amount)
    let first_vin = &raw_tx.vin[0];
    let prev_txid = first_vin.txid.expect("Previous txid missing");
    let prev_vout = first_vin.vout.expect("Previous vout missing");

    // Fetching the transaction that funded our input mto find out the address and amount
    let prev_raw_tx = miner_rpc.get_raw_transaction_info(&prev_txid, None)?;
    let prev_output = &prev_raw_tx.vout[prev_vout as usize];

    let input_address = prev_output.script_pub_key.address.clone().expect("Failed to parse input address").assume_checked().to_string();
    let input_amount = prev_output.value.to_btc();


    // 2. Outputs (Trader's Output, Miner's Change Output)
    let mut output_amount = 0.0;
    let mut change_address = String::new();
    let mut change_amount = 0.0;

    for vout in raw_tx.vout{
        if let Some(addr) = &vout.script_pub_key.address{
            let add_str = addr.clone().assume_checked().to_string();
            if add_str == trader_addr_str{
                output_amount = vout.value.to_btc();
            }else{
                change_address = add_str;
                change_amount = vout.value.to_btc();
            }

        }
    }

    // 3. Fee
    // Extract fee directly from the transaction details
    let miner_tx_info = miner_rpc.get_transaction(&txid, None)?;
    let fee = miner_tx_info.details[0].fee.expect("Fee missing").to_btc();

    // 4. Block Data
    let blockhash = raw_tx.blockhash.expect("Transaction is not confirmed (missing blockhash)");
    let block_info = miner_rpc.get_block_info(&blockhash)?;
    let blockheight = block_info.height;

    // Write the data to ../out.txt in the specified format given in readme.md
    let out_path = "../out.txt";
    let mut file = File::create(out_path);

    writeln!(file, "{}", send_txid_str)?;
    writeln!(file, "{}", input_address)?;
    writeln!(file, "{}", input_amount)?;
    writeln!(file, "{}", trader_addr_str)?;
    writeln!(file, "{}", output_amount)?;
    writeln!(file, "{}", change_address)?;
    writeln!(file, "{}", change_amount)?;
    writeln!(file, "{}", fee)?;
    writeln!(file, "{}", blockheight)?;
    writeln!(file, "{}", blockhash)?;

    println!("Success! Extracted details and wrote to {}", out_path);
    Ok(())
}
