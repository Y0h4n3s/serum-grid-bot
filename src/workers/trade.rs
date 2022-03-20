
use std::sync::Arc;
use std::sync::mpsc::Sender;


use serum_dex::state::{Market};
use solana_client::client_error::ClientError;
use solana_client::rpc_client::RpcClient;

use solana_program::instruction::{ Instruction};

use solana_program::pubkey::Pubkey;

use solana_sdk::signature::{Signature};

use crate::mongodb::client::MongoClient;
use crate::mongodb::models::Trader;
use crate::str_to_pubkey;
use crate::workers::base::{BotConfig, BotThread};
use crate::workers::error::{TradeBotResult};
use crate::workers::message::{ThreadMessage, ThreadMessageCompiler, ThreadMessageSource};

pub struct TraderThread {
    pub stdout: Sender<ThreadMessage>,
    pub config: Arc<BotConfig>,
}

impl TraderThread {
    //fn load_filtered_traders(&self, _connection: &RpcClient) -> Vec<(Trader, Pubkey)> {
    //    vec![]
   // }



}

impl ThreadMessageCompiler for TraderThread {}

impl BotThread for TraderThread {
    fn get_config(&self) -> Arc<BotConfig> {
        self.config.clone()
    }

    fn get_stdout(&self) -> Sender<ThreadMessage> {
        self.stdout.clone()
    }

    fn get_source(&self) -> ThreadMessageSource {
        return ThreadMessageSource::Trader;
    }

    fn get_name(&self) -> String {
        "Trader".to_string()
    }

    fn compile_ixs(&self, connection: &RpcClient, _serum_market: &Market, mongo_client: &MongoClient) -> Vec<Instruction> {
        return vec![]
    }




    fn log_rpc_client_error_(&self, err: ClientError) {
        self.log_rpc_client_error(err);
    }

    fn log_transaction_logs_(&self, connection: &RpcClient, sig: &Signature) {
        self.log_transaction_logs(connection, sig)
    }
}