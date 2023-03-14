use bitcoin::util::psbt::PartiallySignedTransaction as Psbt;
use bitcoin::util::psbt::PsbtSighashType;
use bitcoin::{
    Address, Amount, EcdsaSighashType, OutPoint, Script, Sequence, Transaction, TxIn, TxOut, Txid,
    Witness,
};
use bitcoincore_rpc::bitcoincore_rpc_json::SigHashType;
use bitcoincore_rpc::{bitcoincore_rpc_json, RpcApi};
use bitcoincore_rpc_json::ListUnspentResultEntry;
use std::str::FromStr;
mod helpers;

struct InscriptionData {
    inscription_txid: Txid,
    inscription_index: u32,
    inscription_owner: Address,
}

impl From<(String, String, String)> for InscriptionData {
    fn from(tuple: (String, String, String)) -> Self {
        Self {
            inscription_txid: Txid::from_str(&tuple.0).unwrap(),
            inscription_index: tuple.1.parse::<u32>().unwrap(),
            inscription_owner: Address::from_str(&tuple.2).unwrap(),
        }
    }
}
fn main() {
    let seller_rpc = helpers::initalize_client("ord");
    let buyer_rpc = helpers::initalize_client("buyer");
    let seller_inscription_id = "123";
    let inscription_data =
        InscriptionData::from(helpers::get_inscription_data(&seller_inscription_id));
    let listing_price = Amount::from_btc(0.0001234).unwrap();
    let inscription_listing_price = &listing_price;
    let inscription_utxo = seller_rpc
        .get_raw_transaction(&inscription_data.inscription_txid, None)
        .unwrap();

    let transaction = Transaction {
        version: 2,
        lock_time: bitcoin::PackedLockTime(0),
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: inscription_data.inscription_txid,
                vout: inscription_data.inscription_index,
            },
            script_sig: Script::new(),
            sequence: Sequence::MAX,
            witness: Witness::default(),
        }],
        output: vec![TxOut {
            value: inscription_listing_price.to_sat(),
            script_pubkey: inscription_data.inscription_owner.script_pubkey(),
        }],
    };
    let mut psbt = Psbt::from_unsigned_tx(transaction).unwrap();
    psbt.inputs[0].non_witness_utxo = Some(inscription_utxo.clone());
    psbt.inputs[0].sighash_type = Some(PsbtSighashType::from(
        EcdsaSighashType::SinglePlusAnyoneCanPay,
    ));

    let processed_seller_psbt = seller_rpc
        .wallet_process_psbt(
            &psbt.to_string(),
            Some(true),
            Some(SigHashType::from(EcdsaSighashType::SinglePlusAnyoneCanPay)),
            None,
        )
        .unwrap();

    println!(
        "seller signed for listing at price: {:?} btc",
        &inscription_listing_price.to_btc()
    );

    if buyer_rpc.get_balance(None, None).unwrap() < listing_price {
        println!("buyer doesn't have enough funds");
        return;
    }

    let unspent_utxos = buyer_rpc
        .list_unspent(None, None, None, Some(true), None)
        .unwrap();

    let mut sorted_spendable_utxos = unspent_utxos
        .into_iter()
        .filter(|x| helpers::is_utxo_inscription(x) == false)
        .collect::<Vec<_>>();
    sorted_spendable_utxos.sort_by_key(|x| x.amount);

    if sorted_spendable_utxos.len() == 0 {
        println!("buyer doesn't have any spendable utxos");
        return;
    }

    let dummy_utxo = helpers::retrieve_dummy_utxo(&buyer_rpc, &sorted_spendable_utxos);
    let buyer_address = dummy_utxo.clone().address.unwrap();
    println!("buyer address that will be used: {:?}", &buyer_address);

    let seller_psbt = Psbt::from_str(&processed_seller_psbt.psbt).unwrap();
    let seller_psbt_extracted_tx = seller_psbt.clone().extract_tx();
    let reversed_sorted_utxos = sorted_spendable_utxos
        .clone()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut purchase_tx = Transaction {
        version: 2,
        lock_time: bitcoin::PackedLockTime(0),
        input: vec![
            TxIn {
                previous_output: OutPoint {
                    txid: dummy_utxo.txid,
                    vout: dummy_utxo.vout,
                },
                script_sig: Script::new(),
                sequence: Sequence::MAX,
                witness: Witness::default(),
            },
            TxIn {
                previous_output: seller_psbt_extracted_tx.input[0].previous_output.clone(),
                script_sig: seller_psbt_extracted_tx.input[0].script_sig.clone(),
                sequence: seller_psbt_extracted_tx.input[0].sequence.clone(),
                witness: Witness::default(),
            },
        ],

        output: vec![
            TxOut {
                value: inscription_utxo.output[0].value + dummy_utxo.amount.to_sat(),
                script_pubkey: buyer_address.script_pubkey(),
            },
            seller_psbt_extracted_tx.output[0].clone(),
        ],
    };

    let mut payment_utxos_value = 0;
    let required_payment_value = inscription_listing_price.to_sat() + 1000 + 180 * 2 + 3 * 34 + 10;
    let mut selected_payment_utxos: Vec<ListUnspentResultEntry> = Vec::new();

    for utxo in reversed_sorted_utxos {
        selected_payment_utxos.push(utxo.clone());
        purchase_tx.input.push(TxIn {
            previous_output: OutPoint {
                txid: utxo.txid,
                vout: utxo.vout,
            },
            script_sig: Script::new(),
            sequence: Sequence::MAX,
            witness: Witness::default(),
        });
        payment_utxos_value += utxo.amount.to_sat();
        if payment_utxos_value >= required_payment_value {
            break;
        }
    }
    if payment_utxos_value < inscription_listing_price.to_sat() {
        println!("buyer doesn't have enough funds");
        return;
    }

    purchase_tx.output.push(TxOut {
        value: 1000,
        script_pubkey: buyer_address.script_pubkey(),
    });

    purchase_tx.output.push(TxOut {
        value: payment_utxos_value - required_payment_value,
        script_pubkey: buyer_address.script_pubkey(),
    });

    let mut buyer_psbt = Psbt::from_unsigned_tx(purchase_tx.clone()).unwrap();
    buyer_psbt.inputs[0].non_witness_utxo = Some(
        buyer_rpc
            .get_raw_transaction(&dummy_utxo.txid, None)
            .unwrap(),
    );

    buyer_psbt.inputs[1] = seller_psbt.inputs[0].clone();
    selected_payment_utxos
        .iter()
        .enumerate()
        .for_each(|(i, utxo)| {
            buyer_psbt.inputs[i + 2].non_witness_utxo =
                Some(buyer_rpc.get_raw_transaction(&utxo.txid, None).unwrap());
        });

    let processed_buyer_psbt = buyer_rpc
        .wallet_process_psbt(&buyer_psbt.to_string(), Some(true), None, None)
        .unwrap();

    let raw_buying_tx = buyer_rpc
        .finalize_psbt(&processed_buyer_psbt.psbt, None)
        .unwrap()
        .hex
        .unwrap();

    let buying_txid = buyer_rpc.send_raw_transaction(&raw_buying_tx).unwrap();
    println!(
        "inscription buying tx was succesfully send: {:?}",
        &buying_txid
    );
}
