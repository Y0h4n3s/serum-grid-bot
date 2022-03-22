use std::cell::RefCell;
use std::cmp::min;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::sleep;
use std::time::Duration;
use itertools::Itertools;
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

use solana_sdk::signature::{Signature, Signer};

use crate::{MongoClient, str_to_pubkey, TraderStatus};
use crate::mongodb::models::Trader;
use crate::serum::state::Order;
use crate::workers::base::{BotConfig, BotThread, MAX_IXS};
use crate::workers::message::{ThreadMessage, ThreadMessageCompiler, ThreadMessageSource};

pub struct CleanupThread {
    pub stdout: Sender<ThreadMessage>,
    pub config: Arc<BotConfig>,
}

impl CleanupThread {


}

impl ThreadMessageCompiler for CleanupThread {}

impl BotThread for CleanupThread {
    fn setup(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) {




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
        "Cleanup".to_string()
    }


    fn compile_ixs(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) -> Vec<Instruction> {
        let mut trader = self.get_updated_trader(mongo_client, &self.config.trader);
        if trader.status != TraderStatus::Decommissioned && trader.status != TraderStatus::Stopped  {
            return vec![]
        }
        let mut ixs = vec![];
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
            for mut i in 0..min(open_orders.orders.len(), MAX_IXS) {
                let order = open_orders.orders.get(i).unwrap();
                let grid_order = trader.grids.clone()
                    .clone()
                    .into_iter()
                    .find_position(|grid| grid.order.is_some() && grid.order.as_ref().unwrap().order_id == order.to_string());
                if let Some((grid_index, go)) = grid_order {
                    let match_ix = serum_dex::instruction::match_orders(
                        &self.config.serum_program,
                        &str_to_pubkey(&trader.market_address),
                        &self.bytes_to_pubkey(&serum_market.req_q),
                        &self.bytes_to_pubkey(&serum_market.bids),
                        &self.bytes_to_pubkey(&serum_market.asks),
                        &self.bytes_to_pubkey(&serum_market.event_q),
                        &str_to_pubkey(&trader.base_trader_wallet),
                        &str_to_pubkey(&trader.quote_trader_wallet),
                        5
                    ).unwrap();
                    let cancel_ix = serum_dex::instruction::cancel_order(
                        &self.config.serum_program,
                        &str_to_pubkey(&trader.market_address),
                        &self.bytes_to_pubkey(&serum_market.bids),
                        &self.bytes_to_pubkey(&serum_market.asks),
                        &open_orders_account_pubkey,
                        &self.config.fee_payer.pubkey(),
                        &self.bytes_to_pubkey(&serum_market.event_q),
                        go.order.unwrap().side,
                        *order
                    ).unwrap();

                    ixs.push(match_ix.clone());
                    ixs.push(cancel_ix);
                    ixs.push(match_ix);
                    i += 2;
                }
            }
        }
        ixs
    }

    fn log_rpc_client_error_(&self, err: ClientError) {
        self.log_rpc_client_error(err);
    }

    fn log_transaction_logs_(&self, connection: &RpcClient, sig: &Signature) {
        self.log_transaction_logs(connection, sig)
    }
}