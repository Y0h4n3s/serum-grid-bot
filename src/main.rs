use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use chrono::DateTime;
use rust_base58::FromBase58;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use crate::mongodb::client::MongoClient;
use crate::mongodb::models::TraderStatus;
use crate::workers::base::{BotConfig, BotThread};
use crate::workers::cleanup::CleanupThread;
use crate::workers::message::{ThreadLogLevel, ThreadMessage, ThreadMessageKind, ThreadMessageSource};
use crate::workers::sync::SyncThread;
use crate::workers::trade::{TraderData, TraderThread};

pub mod workers;
pub mod mongodb;
pub mod serum;

const RPC_URL: &str = "https://hedgehog.rpcpool.com"; //"https://mango.devnet.rpcpool.com";
fn main() {




    let mongo_client = MongoClient::new();
    let registered_traders_cursor = mongo_client.traders.find(None, None).unwrap();
    let (thread_message_tx, thread_message_rx) = std::sync::mpsc::channel::<ThreadMessage>();

    for trader_result in registered_traders_cursor {
        match trader_result {
            Ok(trader) => {
                let payer = Keypair::from_base58_string(&trader.trader_keypair);
                let serum_program: Pubkey = str_to_pubkey("9xQeWvG816bUx9EPjHmaT23yvVM2ZWbrrpZb9PusVFin"/*"73A1rYyFwTpRzEsGjJc1P45ee7qMo8vXuMZUDC42Wzwe"*/);
                let market: Pubkey = str_to_pubkey(&trader.market_address);
                let token_program: Pubkey = str_to_pubkey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
                let spl_associated_token_account_program_id: Pubkey = str_to_pubkey(
                    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
                );
                let bot_config = BotConfig {
                    serum_program,
                    token_program,
                    associated_token_program: spl_associated_token_account_program_id,
                    trader: trader.clone(),
                    rpc_url: RPC_URL.to_string(),
                    fee_payer: payer,
                };
                let safe_bot_config = Arc::new(bot_config);

                match trader.status {
                    TraderStatus::Registered | TraderStatus::Initialized => {
                        let _trader_thread_message_tx = thread_message_tx.clone();
                        let mut trader = TraderThread {
                            stdout: _trader_thread_message_tx,
                            config: safe_bot_config.clone(),
                            data:None,
                            trader: trader.clone()
                        };
                        let _trader_thread = std::thread::spawn(move || trader.worker());
                        let sync_thread_message_tx = thread_message_tx.clone();
                        let mut sync = SyncThread {
                            stdout: sync_thread_message_tx,
                            config: safe_bot_config.clone(),
                        };
                        let _sync_thread = std::thread::spawn(move || sync.worker());

                    }
                    TraderStatus::Decommissioned | TraderStatus::Stopped => {

                        let cleanup_thread_message_tx = thread_message_tx.clone();
                        let mut clean_up = CleanupThread {
                            stdout: cleanup_thread_message_tx,
                            config: safe_bot_config.clone(),
                        };
                        let _cleanup_thread = std::thread::spawn(move || clean_up.worker());

                    }
                }


                // let settler_thread_message_tx = thread_message_tx.clone();
                // let settler = SettlerThread {
                //     stdout: settler_thread_message_tx,
                //     config: safe_bot_config.clone(),
                // };
                // let _settle_thread = std::thread::spawn(move || settler.worker());



            }
            Err(_) => {

            }
        }
    }
    let logs_dir = Path::new("logs");
    if !logs_dir.exists() {
        std::fs::create_dir(logs_dir).unwrap();
    }
    for received in thread_message_rx {
        match received.kind {
            ThreadMessageKind::Log(log) => {
                let logs_dir = match received.source {
                    ThreadMessageSource::Trader => {
                        logs_dir.to_str().unwrap().to_owned() + &*"/trader".to_owned()
                    }
                    ThreadMessageSource::Settler => {
                        logs_dir.to_str().unwrap().to_owned() + &*"/settler".to_owned()
                    }
                    ThreadMessageSource::EventConsumer => {
                        logs_dir.to_str().unwrap().to_owned() + &*"/event_consumer".to_owned()
                    }
                    ThreadMessageSource::Cleanup => {
                        logs_dir.to_str().unwrap().to_owned() + &*"/cleanup".to_owned()
                    }
                    ThreadMessageSource::Sync => {
                        logs_dir.to_str().unwrap().to_owned() + &*"/sync".to_owned()
                    }
                };
                let logs_dir_path = Path::new(&logs_dir);
                if !logs_dir_path.exists() {
                    std::fs::create_dir(logs_dir_path).unwrap();
                }
                let log_file = get_log_file(log.level, logs_dir_path);
                let path = Path::new(&log_file);
                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(path)
                    .unwrap();

                let formatted_time = DateTime::<chrono::Local>::from(log.time);
                let mut compiled_message: String = "\n".to_string();
                let log_header = "=================================================  ".to_owned() + &*formatted_time.to_string() + &*"   ===============================================\n".to_owned();
                compiled_message.push_str(&log_header);
                compiled_message.push_str(&log.log);
                file.write(compiled_message.as_bytes()).expect("Error writing logs");
            }
        }
    }

    // let safe_bot_config = Arc::new(bot_config);
    // let (thread_message_tx, thread_message_rx) = std::sync::mpsc::channel::<ThreadMessage>();
    // let _trader_thread_message_tx = thread_message_tx.clone();
    // let trader = TraderThread {
    //     stdout: _trader_thread_message_tx,
    //     config: safe_bot_config.clone(),
    // };
    // let _trader_thread = std::thread::spawn(move || trader.worker());

    // let settler_thread_message_tx = thread_message_tx.clone();
    // let settler = SettlerThread {
    //     stdout: settler_thread_message_tx,
    //     config: safe_bot_config.clone(),
    // };
    // let _settle_thread = std::thread::spawn(move || settler.worker());

    // let sync_thread_message_tx = thread_message_tx.clone();
    // let sync = SyncThread {
    //     stdout: sync_thread_message_tx,
    //     config: safe_bot_config.clone(),
    // };
    // let _sync_thread = std::thread::spawn(move || sync.worker());


    // let event_consumer_thread_message_tx = thread_message_tx.clone();
    // let event_consumer = EventConsumer {
    //     stdout: event_consumer_thread_message_tx,
    //     config: safe_bot_config.clone(),
    // };
    // let _event_consumer_thread = std::thread::spawn(move || event_consumer.worker());

    // let cleanup_thread_message_tx = thread_message_tx.clone();
    // let clean_up = CleanUpThread {
    //     stdout: cleanup_thread_message_tx,
    //     config: safe_bot_config.clone(),
    // };
    // let _cleanup_thread = std::thread::spawn(move || clean_up.worker());



    println!("Hello, world!");
}

fn get_log_file(level: ThreadLogLevel, logs_dir: &Path) -> String {
    let path_dir = match level {
        ThreadLogLevel::Info => {
            logs_dir.to_str().unwrap().to_owned() + &*"/info.log".to_owned()
        }
        ThreadLogLevel::Warn => {
            logs_dir.to_str().unwrap().to_owned() + &*"/warn.log".to_owned()
        }
        ThreadLogLevel::Error => {
            logs_dir.to_str().unwrap().to_owned() + &*"/error.log".to_owned()
        }
    };
    return path_dir;
}

//
// fn account_subscribe(account: &Pubkey) -> Result<AccountSubscription, PubsubClientError> {
//     return solana_client::pubsub_client::PubsubClient::account_subscribe(
//         "wss://ninja.genesysgo.net",
//         account,
//         Some(RpcAccountInfoConfig {
//             encoding: Some(UiAccountEncoding::JsonParsed),
//             data_slice: None,
//             commitment: Some(CommitmentConfig::finalized()),
//         }),
//     );
// }

fn str_to_pubkey(address: &str) -> Pubkey {
    let bytes = FromBase58::from_base58(address).unwrap();
    return Pubkey::new(bytes.as_slice());
}


// #[derive(BorshSerialize, BorshDeserialize, Eq, PartialEq, Debug, Copy, Clone)]
// pub struct EventQueueHeader {
//     blob: [u8; 5],
//     account_flags: u32,
//     blob1: [u8; 4],
//     head: u32,
//     blob2: [u8; 4],
//     count: u32,
//     blob3: [u8; 4],
//     seq_num: u32,
//     blob4: [u8; 4],
// }
//
// #[derive(BorshSerialize, BorshDeserialize, Eq, PartialEq, Debug, Copy, Clone)]
// pub struct Event {
//     pub event_flag: u8,
//     pub open_orders_slot: u8,
//     pub fee_tier: u8,
//     pub blob: [u8; 5],
//     pub native_quantity_released: u64,
//     pub native_quantity_paid: u64,
//     pub native_fee_or_rebate: u64,
//     pub order_id: u128,
//     pub open_orders: [u64; 4],
//     pub client_order_id: u64,
// }
//
// #[derive(BorshSerialize, BorshDeserialize, Eq, PartialEq, Debug, Copy, Clone)]
// pub enum EventFlags {
//     Fill = 0x1,
//     Out = 0x2,
//     Bid = 0x4,
//     Maker = 0x8,
//     ReleaseFunds = 0x10,
// }
//
// pub struct EventQueue {
//     header: EventQueueHeader,
//     events: Vec<Event>,
// }
//
// impl EventQueue {
//     pub fn from_buffer(buffer: &[u8], history: u32) -> Self {
//         let data_length = buffer.len();
//         let header_span = 37;
//         let event_span = 88;
//         let header = EventQueueHeader::try_from_slice(&buffer[0..header_span]).unwrap();
//         let alloc_len: u32 = (((data_length - header_span) / event_span) as f32).floor() as u32;
//         let mut nodes: Vec<Event> = vec![];
//         for i in 0..min(history, alloc_len) {
//             let node_index: usize = ((header.head + header.count + alloc_len - 1 - i) % alloc_len)
//                 .try_into()
//                 .unwrap();
//             let starting_index = header_span + (node_index * event_span);
//             let event = Event::try_from_slice(&buffer[starting_index..starting_index + event_span])
//                 .unwrap();
//             nodes.push(event);
//         }
//         return EventQueue {
//             header,
//             events: nodes,
//         };
//     }
// }
//
// impl Event {
//     fn is_fill(&self) -> bool {
//         return &self.event_flag == &(0x2 as u8) || &self.event_flag == &(0x5 as u8);
//     }
// }
