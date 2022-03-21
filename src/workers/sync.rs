use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::Duration;
use mongodb::bson::{doc, to_bson};
use mongodb::options::UpdateModifications;
use serum_dex::critbit::Slab;
use serum_dex::matching::Side;


use serum_dex::state::{Market};
use solana_client::client_error::ClientError;
use solana_client::rpc_client::RpcClient;
use solana_program::account_info::AccountInfo;


use solana_program::instruction::{ Instruction};
use solana_program::program_pack::Pack;
use solana_program::pubkey::Pubkey;
use solana_sdk::account::ReadableAccount;

use solana_sdk::signature::{Signature};

use crate::{MongoClient, str_to_pubkey};
use crate::mongodb::models::Trader;
use crate::serum::state::Order;
use crate::workers::base::{BotConfig, BotThread};
use crate::workers::message::{ThreadMessage, ThreadMessageCompiler, ThreadMessageSource};

pub struct SyncThread {
    pub stdout: Sender<ThreadMessage>,
    pub config: Arc<BotConfig>,
}

impl SyncThread {

    fn get_updated_trader(&self, mongo_client: &MongoClient, trader: &Trader) -> Trader {
        let trader_cursor = mongo_client.traders.find_one(doc! {
            "market_address": trader.market_address.clone(),
            "owner": trader.owner.clone(),
        }, None).unwrap();

        trader_cursor.unwrap()
    }
}

impl ThreadMessageCompiler for SyncThread {}

impl BotThread for SyncThread {
    fn setup(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) {
        sleep(Duration::from_secs(30));
        let mut trader = self.get_updated_trader(mongo_client, &self.config.trader);
        println!("[?] Trader In db: {}", trader.to_string());
        let open_orders_account_pubkey = str_to_pubkey(trader.serum_open_orders.get(0).unwrap());
        let open_orders_account = connection.get_account(&open_orders_account_pubkey).unwrap();
        let mut open_orders_account_clone = open_orders_account.clone();

        let open_orders_account_info = AccountInfo {
            key: &open_orders_account_pubkey,
            is_signer: false,
            is_writable: false,
            lamports: Rc::new(RefCell::new(&mut open_orders_account_clone.lamports)),
            data: Rc::new(RefCell::new(&mut open_orders_account_clone.data)),
            owner: &open_orders_account.owner().clone(),
            executable: false,
            rent_epoch: open_orders_account.rent_epoch,
        };
        let open_orders_result = serum_market
            .load_orders_mut(
                &open_orders_account_info,
                None,
                &self.config.serum_program,
                None,
                None,
            );

        if let Ok(open_orders) = open_orders_result {
            let base_wallet_account = connection.get_account(&str_to_pubkey(&trader.base_trader_wallet));
            let base_wallet = spl_token::state::Account::unpack(base_wallet_account.unwrap().data()).unwrap();
            trader.base_balance = base_wallet.amount + open_orders.native_coin_free;
            let quote_wallet_account = connection.get_account(&str_to_pubkey(&trader.quote_trader_wallet));
            let quote_wallet = spl_token::state::Account::unpack(quote_wallet_account.unwrap().data()).unwrap();
            trader.quote_balance = quote_wallet.amount + open_orders.native_pc_free;
        }

        let trader_bson = to_bson(&trader.clone()).unwrap();
        let trader_document = trader_bson.as_document().unwrap().to_owned();
        let update_result = mongo_client.traders.find_one_and_update(
            doc! {
            "market_address": trader.market_address.clone(),
            "owner": trader.owner.clone(),
        }, UpdateModifications::Document(doc! {
                "$set": trader_document
            }),
            None
        );
        update_result.unwrap();
    }

    fn get_config(&self) -> Arc<BotConfig> {
        self.config.clone()
    }


    fn get_stdout(&self) -> Sender<ThreadMessage> {
        self.stdout.clone()
    }


    fn get_source(&self) -> ThreadMessageSource {
        return ThreadMessageSource::Settler;
    }

    fn get_name(&self) -> String {
        "Sync".to_string()
    }


    fn compile_ixs(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) -> Vec<Instruction> {
        vec![]
    }

    fn log_rpc_client_error_(&self, err: ClientError) {
        self.log_rpc_client_error(err);
    }

    fn log_transaction_logs_(&self, connection: &RpcClient, sig: &Signature) {
        self.log_transaction_logs(connection, sig)
    }
}