use std::cell::RefCell;
use std::cmp::min;
use std::convert::TryFrom;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::Duration;
use mongodb::bson::doc;
use serum_dex::critbit::{Slab, SlabView};
use serum_dex::matching::Side;

use serum_dex::state::Market;
use solana_client::client_error::{ClientError};
use solana_client::rpc_client::RpcClient;

use solana_program::account_info::AccountInfo;
use solana_program::instruction::{Instruction};
use solana_program::message::Message;
use solana_program::pubkey::Pubkey;



use solana_sdk::account::ReadableAccount;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::{Keypair, Signature};
use solana_sdk::signature::{Signer};
use solana_sdk::transaction::Transaction;


use crate::mongodb::client::{MongoClient};
use crate::workers::message::{ThreadMessage};
use crate::mongodb::models::Trader;
use crate::serum::state::{Order};
use crate::str_to_pubkey;
use crate::workers::message::ThreadMessageSource;

pub trait BotThread {
    fn worker(&mut self) {
        println!("Started {} Thread", self.get_name());
        let config = self.get_config();
        let connection = solana_client::rpc_client::RpcClient::new_with_timeout_and_commitment(
            config.rpc_url.clone(),
            Duration::from_secs(10),
            CommitmentConfig::finalized(),
        );

        let account = connection.get_account(&str_to_pubkey(&config.trader.market_address)).unwrap();
        let mut account_clone = account.clone();

        let serum_market_account_info = AccountInfo {
            key: &str_to_pubkey(&config.trader.market_address),
            is_signer: false,
            is_writable: false,
            lamports: Rc::new(RefCell::new(&mut account_clone.lamports)),
            data: Rc::new(RefCell::new(&mut account_clone.data)),
            owner: &account.owner().clone(),
            executable: false,
            rent_epoch: account.rent_epoch,
        };
        let serum_market = serum_dex::state::Market::load(
            &serum_market_account_info,
            &config.serum_program,
            true,
        )
            .unwrap();
        let mongo_client = MongoClient::new();

        loop {
            self.setup(&connection, &serum_market, &mongo_client);
            let ix = self.compile_ixs(&connection, &serum_market, &mongo_client);
            if ix.len() > 0 {
                let message = Message::new(&ix.clone(), Some(&config.fee_payer.pubkey().clone()));
                let mut tx = Transaction::new_unsigned(message);
                let latest_block_hash = connection.get_latest_blockhash();
                if let Ok(block_hash) = latest_block_hash {
                    println!("[?] Sending Transaction");
                    tx.sign(&[&config.fee_payer], block_hash);
                    let sig1 = connection.send_and_confirm_transaction(&tx);

                    match sig1 {
                        Ok(sig) => {
                            println!("[+] Transaction Successful: {:?}", sig);
                            self.cleanup(&connection, &serum_market, &mongo_client);

                        }
                        Err(e) => {
                            eprintln!("[-] An Error Occurred: {:?}", e )

                        }
                    }
                } else {
                    eprintln!("[-] An Error Occurred: {:?}", latest_block_hash.unwrap_err());

                }
            } else {
                self.cleanup(&connection, &serum_market, &mongo_client);
            }
        }
    }

    fn send_message(&self, mes: ThreadMessage) {
        match self.get_stdout().send(mes) {
            Ok(_) => {}
            Err(send_error) => {
                eprintln!("{:?}", send_error)
            }
        }
    }
    fn load_trader_accounts_on_market(
        &self,
        market: &Pubkey,
        _connection: &RpcClient,
        mongodb: &MongoClient
    ) -> Vec<Trader> {
        let mut traders = mongodb.traders.find(Some(doc! {"market_address": market.to_string()}), None);
        return if let Ok(mut account_infos) = traders {
            account_infos
                .filter(|trader| {
                    trader.is_ok()
                })
                .map(|trader | {
                    trader.unwrap()
                })
                .collect()
        } else {
            vec![]
        }
    }

    fn bytes_to_pubkey(&self, bytes: &[u64; 4]) -> Pubkey {
        Pubkey::new(&self.nums_to_bytes(bytes)[0..32])
    }

    fn nums_to_bytes(&self, nums: &[u64]) -> Vec<u8> {
        let mut bytes_u8 = vec![];
        nums.iter().for_each(|b| {
            b.to_le_bytes()
                .iter()
                .for_each(|c| bytes_u8.push(c.clone()))
        });
        return bytes_u8;
    }

    fn parse_order_book_for_owner(&self, side: Side, slab: &Slab, owner: &[u64; 4]) -> Vec<Order> {
        let mut filtered: Vec<Order> = vec![];
        for i in 0..slab.capacity() {
            match slab.get(i as u32) {
                Some(node) => match node.as_leaf() {
                    Some(n) => {
                        if &n.owner() == owner {
                            filtered.push(Order {
                                side,
                                price: u64::try_from(n.price()).unwrap(),
                                client_id: n.client_order_id(),
                                owner: n.owner(),
                                order_id: n.order_id(),
                            })
                        }
                    }
                    None => {}
                },
                None => {}
            }
        }
        filtered
    }

    fn parse_order_book(&self, side: serum_dex::matching::Side, slab: &Slab) -> Vec<Order> {
        let mut filtered: Vec<Order> = vec![];
        for i in 0..slab.capacity() {
            match slab.get(i as u32) {
                Some(node) => match node.as_leaf() {
                    Some(n) => filtered.push(Order {
                        side,
                        price: u64::try_from(n.price()).unwrap(),
                        client_id: n.client_order_id(),
                        owner: n.owner(),
                        order_id: n.order_id(),
                    }),
                    None => {}
                },
                None => {}
            }
        }
        filtered
    }

    fn setup(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) {}
    fn cleanup(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) {}

    fn get_config(&self) -> Arc<BotConfig>;
    fn get_stdout(&self) -> Sender<ThreadMessage>;
    fn get_source(&self) -> ThreadMessageSource;
    fn get_name(&self) -> String;

    fn compile_ixs(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) -> Vec<Instruction>;
    fn log_rpc_client_error_(&self, err: ClientError);
    fn log_transaction_logs_(&self, connection: &RpcClient, sig: &Signature);
}


#[derive(Debug)]
pub struct BotConfig {
    pub serum_program: Pubkey,
    pub token_program: Pubkey,

    pub associated_token_program: Pubkey,
    pub trader: Trader,
    pub rpc_url: String,
    pub fee_payer: Keypair,
}