use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::cmp::{min, Reverse};
use std::num::NonZeroU64;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use itertools::Itertools;
use mongodb::bson::{doc, to_bson};
use mongodb::options::{FindOneAndUpdateOptions, UpdateModifications};
use serde::Serialize;
use serum_dex::critbit::Slab;
use serum_dex::instruction::{NewOrderInstructionV3, SelfTradeBehavior};
use serum_dex::matching::{OrderType, Side};
use serum_dex::state::{Market, ToAlignedBytes};
use solana_client::client_error::ClientError;
use solana_client::rpc_client::RpcClient;
use solana_program::account_info::AccountInfo;
use solana_program::instruction::Instruction;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::SysvarId;
use solana_sdk::account::ReadableAccount;
use solana_sdk::signature::{Signature, Signer};

use crate::mongodb::client::MongoClient;
use crate::mongodb::models::{GridPosition, GridStatus, Order as OrderDb, Trader};
use crate::serum::state::Order;
use crate::str_to_pubkey;
use crate::workers::base::{BotConfig, BotThread};
use crate::workers::error::TradeBotResult;
use crate::workers::message::{ThreadMessage, ThreadMessageCompiler, ThreadMessageSource};

const MAX_NEW_ORDER_IXS: usize = 10;
const CLIENT_ORDER_ID: u64 = 66935256;

pub struct Price {
    pub buy: u64,
    pub sell: u64,
}

pub struct TraderData {
    pub last_price: Option<Price>,
}

pub struct TraderThread {
    pub stdout: Sender<ThreadMessage>,
    pub config: Arc<BotConfig>,
    pub data: Option<TraderData>,
    pub trader: Trader,
}

impl TraderThread {
    fn get_updated_trader(&self, mongo_client: &MongoClient, trader: &Trader) -> Trader {
        let trader_cursor = mongo_client.traders.find_one(doc! {
            "market_address": trader.market_address.clone(),
            "owner": trader.owner.clone(),
        }, None).unwrap();

        trader_cursor.unwrap()
    }

    fn make_new_order_ix(&self, serum_market: &Market, trader: &Trader, side: Side, price: u64, qty: u64) -> Instruction{
        serum_dex::instruction::new_order(
            &str_to_pubkey(&trader.market_address),
            &str_to_pubkey(trader.serum_open_orders.get(0).unwrap()),
            &self.bytes_to_pubkey(&serum_market.req_q),
            &self.bytes_to_pubkey(&serum_market.event_q),
            &self.bytes_to_pubkey(&serum_market.bids),
            &self.bytes_to_pubkey(&serum_market.asks),
            &str_to_pubkey(if side == Side::Bid{&trader.quote_trader_wallet} else {&trader.base_trader_wallet}),
            &self.config.fee_payer.pubkey(),
            &self.bytes_to_pubkey(&serum_market.coin_vault),
            &self.bytes_to_pubkey(&serum_market.pc_vault),
            &spl_token::id(),
            &Rent::id(),
            None,
            &self.config.serum_program,
            side,
            NonZeroU64::new(price).unwrap(),
            NonZeroU64::new(qty).unwrap(),
            OrderType::Limit,
            CLIENT_ORDER_ID,
            SelfTradeBehavior::AbortTransaction,
            0,
            NonZeroU64::new(if side == Side::Bid{qty} else {1}).unwrap(),
        ).unwrap()
    }
}

impl ThreadMessageCompiler for TraderThread {}

#[derive(PartialEq)]
pub enum Movement {
    Downward,
    Upward
}
impl BotThread for TraderThread {
    fn setup(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) {
        let config = self.get_config();
        let mut trader = self.get_updated_trader(mongo_client, &config.trader);


        let bids_account_pubkey = self.bytes_to_pubkey(&serum_market.bids);
        let asks_account_pubkey = self.bytes_to_pubkey(&serum_market.asks);
        let open_orders_account_pubkey = str_to_pubkey(trader.serum_open_orders.get(0).unwrap());

        let bids_account = connection.get_account(&bids_account_pubkey).unwrap();
        let mut bids_account_clone = bids_account.clone();

        let bids_account_info = AccountInfo {
            key: &bids_account_pubkey,
            is_signer: false,
            is_writable: false,
            lamports: Rc::new(RefCell::new(&mut bids_account_clone.lamports)),
            data: Rc::new(RefCell::new(&mut bids_account_clone.data)),
            owner: &bids_account.owner().clone(),
            executable: false,
            rent_epoch: bids_account.rent_epoch,
        };
        let all_bids = serum_market.load_bids_mut(
            &bids_account_info,
        )
            .unwrap();

        let asks_account = connection.get_account(&asks_account_pubkey).unwrap();
        let mut asks_account_clone = asks_account.clone();

        let asks_account_info = AccountInfo {
            key: &asks_account_pubkey,
            is_signer: false,
            is_writable: false,
            lamports: Rc::new(RefCell::new(&mut asks_account_clone.lamports)),
            data: Rc::new(RefCell::new(&mut asks_account_clone.data)),
            owner: &asks_account.owner().clone(),
            executable: false,
            rent_epoch: asks_account.rent_epoch,
        };
        let all_asks = serum_market.load_asks_mut(
            &asks_account_info,
        )
            .unwrap();

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
        let mut bids = self.parse_order_book(Side::Bid, all_bids.deref().clone());
        let mut asks = self.parse_order_book(Side::Ask, all_asks.deref().clone());

        let mut my_bids = self.parse_order_book_for_owner(Side::Bid, all_bids.deref().clone(), &open_orders_account_info.key.to_aligned_bytes());
        let mut my_asks = self.parse_order_book_for_owner(Side::Ask, all_asks.deref().clone(), &open_orders_account_info.key.to_aligned_bytes());

        let mut my_orders = vec![my_bids, my_asks];
        let mut my_orders_flat: Vec<Order> = my_orders.into_iter().flatten().collect::<Vec<Order>>();

        let awaiting_grids: Vec<GridPosition> = trader.grids.clone().into_iter().filter(|grid| grid.status == GridStatus::AwaitingBuy || grid.status == GridStatus::AwaitingSell).collect();

        for grid in awaiting_grids {
            let open_order = my_orders_flat
                .clone()
                .into_iter()
                .find_position(|order| grid.price == order.price);
            if let Some(order) = open_order {
                // the order is still active
            } else {
                // the order has been filled mark as violated
                let grid_order = trader.grids.clone()
                    .clone()
                    .into_iter()
                    .find_position(|order| grid.price == order.price);
                if let Some((grid_index, mut grid)) = grid_order {
                    if grid.order.is_some() {
                        trader.grids.get_mut(grid_index).unwrap().status = GridStatus::Violated;
                    } else {
                        trader.grids.get_mut(grid_index).unwrap().status = GridStatus::Idle;

                    }
                }
            }
        }

        // reassign false violations
        for (grid, index) in trader.grids.clone().into_iter().zip(0..trader.grids.clone().len()) {
            if grid.order.is_none() {
                trader.grids.get_mut(index).unwrap().status = GridStatus::Idle
            }
        }

        let open_orders_result = serum_market
            .load_orders_mut(
                &open_orders_account_info,
                None,
                &self.config.serum_program,
                None,
                None,
            );
        if let Ok(open_orders) = open_orders_result {
            for mut order in my_orders_flat {
                let grid_order = trader.grids.clone()
                    .clone()
                    .into_iter()
                    .find_position(|grid| grid.price == order.price);
                if let Some((grid_index, mut grid)) = grid_order {

                    trader.grids.get_mut(grid_index).unwrap().status = if order.side == Side::Bid {GridStatus::AwaitingBuy} else {GridStatus::AwaitingSell};
                    trader.grids.get_mut(grid_index).unwrap().order = Some(OrderDb {
                        price: grid.price,
                        side: order.side,
                        client_order_id: order.client_id,
                        is_filled: false,
                        owner: self.bytes_to_pubkey(&order.owner).to_string(),
                       // order_id: order.order_id,
                    });
                } else {
                    println!("Unknown order")
                }
            }

        }
        bids.sort_by_key(|k| Reverse(k.price));
        asks.sort_by_key(|k| k.price);
        match &mut self.data {
            Some(data) => {
                if let (highest_bid, highest_ask) = (bids.get(0), asks.get(0)) {
                    (*data).borrow_mut().last_price = Some(Price {
                        buy: bids.get(0).unwrap().price,
                        sell: asks.get(0).unwrap().price,
                    })
                } else {
                    (*data).borrow_mut().last_price = None
                }
            }
            None => {
                if let (highest_bid, highest_ask) = (bids.get(0), asks.get(0)) {
                    self.data = Some(TraderData {
                        last_price: Some(Price {
                            buy: bids.get(0).unwrap().price,
                            sell: asks.get(0).unwrap().price,
                        })
                    })
                } else {
                    self.data = None
                }
            }
        }
        self.trader = trader;
        self.trader.grids.sort_by_key(|grid| Reverse(grid.price))
    }

    fn cleanup(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) {
        let trader_bson = to_bson(&self.trader.clone()).unwrap();
        let trader_document = trader_bson.as_document().unwrap().to_owned();
        let update_result = mongo_client.traders.find_one_and_update(
            doc! {
            "market_address": self.trader.market_address.clone(),
            "owner": self.trader.owner.clone(),
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
        return ThreadMessageSource::Trader;
    }

    fn get_name(&self) -> String {
        "Trader".to_string()
    }


    fn compile_ixs(&mut self, connection: &RpcClient, serum_market: &Market, mongo_client: &MongoClient) -> Vec<Instruction> {
        let mut idleGrids: Vec<GridPosition> = self.trader.grids.clone().into_iter().filter(|grid| {
            return grid.status == GridStatus::Idle || grid.status == GridStatus::Violated;
        }).collect();
        let mut ixs: Vec<Instruction> = vec![];
        println!("[?] Using Trader: {}", self.trader.to_string());

        if let Some(data) = &self.data {
            if let Some(price) = &data.last_price {
                let spread_price = (price.buy + price.sell) / 2;
                if spread_price > self.trader.upper_price_range || spread_price < self.trader.lower_price_range {
                    println!("[?] Price went over bounds {}", spread_price);
                    return ixs
                }

                for mut i in 0..min(idleGrids.len(), MAX_NEW_ORDER_IXS) {
                    let grid_position = idleGrids.get(i).unwrap();
                    let grid_order = self.trader.grids.clone()
                        .clone()
                        .into_iter()
                        .find_position(|grid| grid.price == grid_position.price);
                    match grid_position.status {
                        GridStatus::Idle => {
                            let base_size = (self.trader.amount_per_grid / grid_position.price) * serum_market.coin_lot_size / serum_market.pc_lot_size;
                            let base_size_lots = base_size / serum_market.coin_lot_size;
                            let quote_size_lots = base_size_lots * serum_market.pc_lot_size * grid_position.price;

                            if spread_price > grid_position.price {
                                //buy
                                let new_order_ix = self.make_new_order_ix(serum_market, &self.trader, Side::Bid, grid_position.price, quote_size_lots);

                                ixs.push(new_order_ix);

                                if let Some((grid_index, mut grid)) = grid_order {
                                    self.trader.grids.get_mut(grid_index).unwrap().status = GridStatus::AwaitingBuy;
                                    self.trader.grids.get_mut(grid_index).unwrap().order = Some(OrderDb {
                                        price: grid_position.price,
                                        side: Side::Bid,
                                        client_order_id: CLIENT_ORDER_ID,
                                        is_filled: false,
                                        owner: "".to_string()
                                    })
                                }
                            } else {
                                //sell

                                if base_size_lots == 0 {
                                    println!("[?] Size Below Miniumum");
                                    continue;
                                }
                                let new_order_ix = self.make_new_order_ix(serum_market, &self.trader, Side::Ask, grid_position.price, base_size_lots);
                                ixs.push(new_order_ix);
                                if let Some((grid_index, mut grid)) = grid_order {
                                    self.trader.grids.get_mut(grid_index).unwrap().status = GridStatus::AwaitingSell;
                                    self.trader.grids.get_mut(grid_index).unwrap().order = Some(OrderDb {
                                        price: grid_position.price,
                                        side: Side::Ask,
                                        client_order_id: CLIENT_ORDER_ID,
                                        is_filled: false,
                                        owner: "".to_string()
                                    })
                                }
                            }
                        }

                        GridStatus::Violated => {
                            // place order on next grid if it doesnt exist
                            // if the violated grid was a buy order sell on the next higher grid if no orders exist there vice versa if it was a sell order

                            let mut buy_indexes: Vec<usize> = vec![];
                            let mut sell_indexes: Vec<usize> = vec![];
                            if let Some(order) = grid_position.clone().order {
                                if let Some((grid_index, mut grid)) = grid_order {

                                match order.side {
                                    // TODO: recalculate size
                                    Side::Ask  => {

                                            if  grid_index == 0 {
                                                println!("[-] I don't know how this happened, should have never happened");
                                                continue
                                            }
                                            if let Some(next_grid) = self.trader.grids.get(grid_index + 1) {
                                                if next_grid.status == GridStatus::Violated {
                                                    let base_size = (self.trader.amount_per_grid / next_grid.price) * serum_market.coin_lot_size / serum_market.pc_lot_size;
                                                    let base_size_lots = base_size / serum_market.coin_lot_size;
                                                    let quote_size_lots = base_size_lots * serum_market.pc_lot_size * next_grid.price;
                                                    // place buy order for previous closed order
                                                    let new_order_ix = self.make_new_order_ix(serum_market, &self.trader, Side::Bid, next_grid.price, quote_size_lots);
                                                    ixs.push(new_order_ix);
                                                    buy_indexes.push(grid_index + 1);
                                                    self.trader.grids.get_mut(grid_index + 1).unwrap().order = Some(OrderDb {
                                                        price: next_grid.price,
                                                        side: Side::Bid,
                                                        client_order_id: CLIENT_ORDER_ID,
                                                        is_filled: false,
                                                        owner: "".to_string()
                                                    })
                                                } else {
                                                    println!("[-] Next grid is still awaiting filling, skipping buy");

                                                }
                                            }


                                    }
                                    Side::Bid => {

                                            if grid_index == self.trader.grids.len() - 1{
                                                println!("[-] I don't know how this happened, should have never happened");
                                                continue
                                            }
                                        if let Some(next_grid) = self.trader.grids.get(grid_index  - 1) {
                                            if next_grid.status == GridStatus::Violated {
                                                // place sell order for previous closed order
                                                let base_size = (self.trader.amount_per_grid / next_grid.price) * serum_market.coin_lot_size / serum_market.pc_lot_size;
                                                let base_size_lots = base_size / serum_market.coin_lot_size;
                                                let new_order_ix = self.make_new_order_ix(serum_market, &self.trader, Side::Ask, next_grid.price, base_size_lots);
                                                ixs.push(new_order_ix);
                                                sell_indexes.push(grid_index-1);
                                                self.trader.grids.get_mut(grid_index - 1).unwrap().order = Some(OrderDb {
                                                    price: next_grid.price,
                                                    side: Side::Ask,
                                                    client_order_id: CLIENT_ORDER_ID,
                                                    is_filled: false,
                                                    owner: "".to_string()
                                                })
                                            }
                                            else {
                                                println!("[-] Next grid is still awaiting filling, skipping sell");

                                            }
                                        }

                                        }
                                    }
                                }
                            }
                            for grid_index in buy_indexes {
                                self.trader.grids.get_mut(grid_index).unwrap().status = GridStatus::AwaitingBuy
                            }
                            for grid_index in sell_indexes {
                                self.trader.grids.get_mut(grid_index).unwrap().status = GridStatus::AwaitingSell
                            }
                        }
                        _ => {}
                    }
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