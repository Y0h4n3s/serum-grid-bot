use std::time::SystemTime;

use solana_client::client_error::{ClientError, ClientErrorKind};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcTransactionConfig;
use solana_client::rpc_request::{RpcError, RpcResponseErrorData};
use solana_client::rpc_response::RpcSimulateTransactionResult;
use solana_program::instruction::InstructionError;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::TransactionError;
use solana_transaction_status::EncodedConfirmedTransaction;

use crate::workers::base::BotThread;

pub enum ThreadMessageSource {
    Trader,
    Settler,
    EventConsumer,
    Cleanup,
    Sync
}

pub enum ThreadLogLevel {
    Info,
    Warn,
    Error,
}

pub struct ThreadLog {
    pub(crate) log: String,
    pub(crate) time: SystemTime,
    pub(crate) level: ThreadLogLevel,
}

pub enum ThreadMessageKind {
    Log(ThreadLog),
}

pub struct ThreadMessage {
    pub(crate) source: ThreadMessageSource,
    pub(crate) kind: ThreadMessageKind,
}

impl ThreadMessage {
    pub fn compile_log_message(
        source: ThreadMessageSource,
        log_message: String,
        level: ThreadLogLevel,
    ) -> Self {
        let now = std::time::SystemTime::now();
        ThreadMessage {
            source,
            kind: ThreadMessageKind::Log(ThreadLog {
                log: log_message,
                time: now,
                level,
            }),
        }
    }
}


pub trait ThreadMessageCompiler: BotThread {
    fn compile_log_message(&self, log_message: String, level: ThreadLogLevel) -> ThreadMessage {
        let now = std::time::SystemTime::now();
        ThreadMessage {
            source: self.get_source(),
            kind: ThreadMessageKind::Log(ThreadLog {
                log: log_message,
                time: now,
                level,
            }),
        }
    }

    fn log_rpc_client_error(&self, e: ClientError) {
        let (logs, level) = self.rpc_client_error_message(e);
        println!("{}", logs);
        let mes = self.compile_log_message(logs, level);
        self.send_message(mes);
    }

    fn log_str(&self, log: &str, level: ThreadLogLevel) {
        let mes = self.compile_log_message(log.to_string(), level);
        self.send_message(mes);
    }


    fn rpc_client_error_message(&self, e: ClientError) -> (String, ThreadLogLevel) {
        let mut logs = "".to_string();
        let mut level = ThreadLogLevel::Error;
        match e.kind {
            ClientErrorKind::TransactionError(err) => {
                logs.push_str(&format!("{:?}\n", err));
            }
            ClientErrorKind::RpcError(err) => match err {
                RpcError::RpcRequestError(err) => logs.push_str(&format!("\nRequest error{:?}\n", err)),
                RpcError::RpcResponseError {
                    code,
                    message,
                    data,
                } => {
                    logs.push_str(&format!(
                        "\nResponse Error\nError Code {}\nMessage: {}",
                        code, message
                    ));
                    match data {
                        RpcResponseErrorData::Empty => {}
                        RpcResponseErrorData::SendTransactionPreflightFailure(err) => {
                            match err {
                                RpcSimulateTransactionResult {
                                    err,
                                    accounts: _,
                                    logs: prog_logs,
                                    units_consumed,
                                } => {
                                    if let Some(error) = err {
                                        match error {
                                            TransactionError::InstructionError(_o, instruction_err) => {
                                                if let InstructionError::Custom(ix_err) =
                                                instruction_err
                                                {
                                                    logs.push_str(&format!(
                                                        "\nUnknown Error: {:?}",
                                                        ix_err
                                                    ))
                                                } else {
                                                    logs.push_str(&format!(
                                                        "\nInstruction Error: {:?}",
                                                        instruction_err
                                                    ))
                                                }
                                            }
                                            _ => logs
                                                .push_str(&format!("\nTransaction Error: {:?}", error)),
                                        }
                                    }
                                    if let Some(program_logs) = prog_logs {
                                        logs.push_str(&format!("\nProgram Logs"));
                                        for log in program_logs {
                                            logs.push_str(&format!("\n{}", log))
                                        }
                                    }
                                    if let Some(units) = units_consumed {
                                        logs.push_str(&format!("\nConsumed {} compute units", units))
                                    }
                                }
                            }
                        }
                        RpcResponseErrorData::NodeUnhealthy { .. } => {
                            logs.push_str(&format!("\nNode is behind"))
                        }
                    }
                }
                RpcError::ParseError(err) => logs.push_str(&format!("\nParse error: {:?}\n", err)),
                RpcError::ForUser(err) => logs.push_str(&format!("\nUser for me: {:?}\n", err)),
            },
            ClientErrorKind::Io(err) => logs.push_str(&format!("\nIo Error\n{:?}", err)),
            ClientErrorKind::Reqwest(err) => logs.push_str(&format!("\nReqwest Error\n{:?}", err)),
            ClientErrorKind::SerdeJson(err) => logs.push_str(&format!("\nSerde Error\n{:?}", err)),
            ClientErrorKind::SigningError(err) => {
                logs.push_str(&format!("\nSignature Error\n{:?}", err))
            }
            ClientErrorKind::FaucetError(err) => logs.push_str(&format!("\nFaucet Error\n{:?}", err)),
            ClientErrorKind::Custom(err) => logs.push_str(&format!("\nProgram Error: {:?}", err)),
        }
        logs.push_str("\n");
        return (logs, level);
    }

    fn log_transaction_logs(&self, connection: &RpcClient, signature: &Signature) {
        let (mut logs, level) = self.transaction_logs(connection, signature);
        logs.push_str(&format!("\nSignature: {}", signature.to_string()));
        println!("{} {}", logs, signature);
        let mes = self.compile_log_message(logs, level);
        self.send_message(mes);
    }


    fn transaction_logs(&self, connection: &RpcClient, signature: &Signature) -> (String, ThreadLogLevel) {
        let mut logs = "".to_string();
        let mut level = ThreadLogLevel::Info;
        let tx_result = connection
            .get_transaction_with_config(
                signature,
                RpcTransactionConfig {
                    encoding: None,
                    commitment: None,
                },
            );
        if let Ok(tx) = tx_result {
        match tx {
            EncodedConfirmedTransaction {
                slot: _,
                transaction,
                block_time: _,
            } => match transaction.meta {
                None => {}
                Some(meta) => {
                    if let Some(program_logs) = meta.log_messages {
                        logs.push_str("Program Logs\n");

                        for log in program_logs {
                            logs.push_str(&log);
                            logs.push_str("\n")
                        }
                    }
                }
            },
        }
        } else {
            level = ThreadLogLevel::Error;
            logs.push_str(&format!("\nClient Error: {:?}", tx_result.unwrap_err()))
        }
        logs.push_str("\n");
        return (logs, level);
    }
}