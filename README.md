# Extra-Ordinaire

A simple implementation of Bitcoin inscriptions trading based on PSBT written in Rust.

## Specs

The setup assumes two bitcoin core wallets, one for the buyer named `buyer` and one for the seller named `ord`.
The seller lists an Inscription it owns for sale by creating a PSBT with the Inscription as an output.

## Usage

* create an `.env` file and provide both:
  * `COOKIE`: Bitcoin core cookie file
  * `ORD_EXPLORER`: Ord explorer url example: https://ordinals.com/
 
* Specify `seller_inscription_id` to sell (main.rs file)
* `cargo run`

## Acknowledgements

This implementation is based on [Casey's suggestion](https://github.com/casey/ord/issues/802) and inspired by [OpenOrdex implementation](https://github.com/orenyomtov/openordex)

## Disclaimer

This is a proof of concept and should not be used in production. It is not audited and has not been tested in a real world scenario. Use at your own risk.
