
use std::sync::Arc;
use std::sync::mpsc::Sender;


use serum_dex::state::{Market};
use solana_client::client_error::ClientError;
use solana_client::rpc_client::RpcClient;


use solana_program::instruction::{ Instruction};

use solana_sdk::signature::{Signature};

use crate::{MongoClient};
use crate::workers::base::{BotConfig, BotThread};
use crate::workers::message::{ThreadMessage, ThreadMessageCompiler, ThreadMessageSource};

pub struct SyncThread {
    pub stdout: Sender<ThreadMessage>,
    pub config: Arc<BotConfig>,
}

impl SyncThread {
}

impl ThreadMessageCompiler for SyncThread {}

impl BotThread for SyncThread {
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


    fn compile_ixs(&self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) -> Vec<Instruction> {
        vec![]
    }

    fn log_rpc_client_error_(&self, err: ClientError) {
        self.log_rpc_client_error(err);
    }

    fn log_transaction_logs_(&self, connection: &RpcClient, sig: &Signature) {
        self.log_transaction_logs(connection, sig)
    }
}