use bitcoin::psbt::{Input, Psbt};
use bitcoin::{Address, Amount, OutPoint, Script, Sequence, Transaction, TxIn, TxOut, Witness};
use bitcoincore_rpc::{bitcoincore_rpc_json, Auth, Client, RpcApi};
use bitcoincore_rpc_json::{CreateRawTransactionInput, ListUnspentResultEntry};
use regex::Regex;

pub fn initalize_client(wallet_name: &str) -> bitcoincore_rpc::Client {
    let base_path = "http://localhost:38332/wallet/".to_owned();
    let wallet_path = [base_path, wallet_name.to_owned()].join("");
    let auth = bitcoincore_rpc::Auth::CookieFile(std::env::var("COOKIE").unwrap().into());
    bitcoincore_rpc::Client::new(&wallet_path, auth).unwrap()
}

pub fn retrieve_dummy_utxo(
    buyer_rpc: &Client,
    utxos: &Vec<ListUnspentResultEntry>,
) -> ListUnspentResultEntry {
    let potential_dummy_utxos = &utxos
        .iter()
        .filter(|utxo| utxo.amount <= Amount::from_sat(1000))
        .collect::<Vec<&ListUnspentResultEntry>>();

    let dummy_utxo = if potential_dummy_utxos.len() == 0 {
        let dummy_address = utxos[0].clone().address.unwrap();

        let mut dummy_psbt = Psbt::from_unsigned_tx(Transaction {
            version: 2,
            lock_time: bitcoin::PackedLockTime(0),
            input: vec![TxIn {
                previous_output: OutPoint {
                    txid: utxos[0].txid,
                    vout: utxos[0].vout,
                },
                script_sig: Script::new(),
                sequence: Sequence::MAX,
                witness: Witness::default(),
            }],
            output: vec![
                TxOut {
                    value: 1000,
                    script_pubkey: dummy_address.script_pubkey(),
                },
                TxOut {
                    value: utxos[0].amount.to_sat() - 1000 - 258,
                    script_pubkey: dummy_address.script_pubkey(),
                },
            ],
        })
        .unwrap();

        dummy_psbt.inputs[0].non_witness_utxo =
            Some(buyer_rpc.get_raw_transaction(&utxos[0].txid, None).unwrap());

        let dummy_psbt_string = &dummy_psbt.to_string();
        let processed_dummy_psbt = buyer_rpc
            .wallet_process_psbt(dummy_psbt_string, Some(true), None, None)
            .unwrap();
        let processed_dummy_psbt_string = &processed_dummy_psbt.psbt;
        let dummy_raw_tx = buyer_rpc
            .finalize_psbt(processed_dummy_psbt_string, None)
            .unwrap()
            .hex
            .unwrap();

        let dummy_txid = buyer_rpc.send_raw_transaction(&dummy_raw_tx).unwrap();
        println!("created dummy {:?}", &dummy_txid);
        let unspent_utxos = buyer_rpc
            .list_unspent(None, None, None, Some(true), None)
            .unwrap();
        let mut sorted_utxos = unspent_utxos.clone();
        sorted_utxos.sort_by_key(|x| x.amount);
        let potential_dummy_utxos = &sorted_utxos
            .iter()
            .filter(|utxo| utxo.amount <= Amount::from_sat(1000))
            .collect::<Vec<&ListUnspentResultEntry>>();
        potential_dummy_utxos[0].clone()
    } else {
        potential_dummy_utxos[0].clone()
    };

    dummy_utxo
}

pub fn is_utxo_inscription(utxo: &ListUnspentResultEntry) -> bool {
    let explorer_url = std::env::var("ORD_EXPLORER").unwrap()
        + "output/"
        + &utxo.txid.to_string()
        + ":"
        + &utxo.vout.to_string();
    let resp = reqwest::blocking::get(explorer_url)
        .unwrap()
        .text()
        .unwrap();
    if resp.contains("inscription") {
        true
    } else {
        false
    }
}

pub fn get_inscription_data(inscription_number: &str) -> (String, String, String) {
    let explorer_url = std::env::var("ORD_EXPLORER").unwrap();
    let resp = reqwest::blocking::get(explorer_url.clone() + "inscriptions/" + &inscription_number)
        .unwrap()
        .text()
        .unwrap();

    let inscription_id = Regex::new(r"/inscription/(.*?)>")
        .unwrap()
        .captures(&resp)
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .to_string();

    let resp = reqwest::blocking::get(explorer_url + "inscription/" + &inscription_id)
        .unwrap()
        .text()
        .unwrap();

    let inscription_owner = Regex::new(r"address</dt>\n\s+<dd class=monospace>(.*?)</dd>")
        .unwrap()
        .captures(&resp)
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .to_string();

    let inscription_output = Regex::new(r"/output/(.*?)>")
        .unwrap()
        .captures(&resp)
        .unwrap()
        .get(1)
        .unwrap()
        .as_str()
        .to_string();

    println!("inscription_output: {:?}", &inscription_output);
    let (inscription_tx, inscription_index) = (
        inscription_output.split(":").collect::<Vec<&str>>()[0],
        inscription_output.split(":").collect::<Vec<&str>>()[1],
    );

    (
        inscription_tx.to_string(),
        inscription_index.to_string(),
        inscription_owner.to_string(),
    )
}
