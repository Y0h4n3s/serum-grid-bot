use serde::{Serialize, Deserialize};
use serum_dex::matching::Side;


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
    pub decimals: u64,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Trader {
    pub market_address: String,
    pub base_token_info: TokenInfo,
    pub quote_token_info: TokenInfo,
    pub trader_keypair: String,
    pub base_trader_wallet: String,
    pub quote_trader_wallet: String,
    pub serum_open_orders: Vec<String>,
    pub owner: String,
    pub grids_count: u64,
    pub grids: Vec<GridPosition>,
    pub amount_per_grid: u64,
    pub upper_price_range: u64,
    pub lower_price_range: u64,
    pub stopping_price_high: Option<u64>,
    pub stopping_price_low: Option<u64>,
    pub starting_price_buy: u64,
    pub starting_price_sell: u64,
    pub starting_base_balance: u64,
    pub starting_quote_balance: u64,
    pub deposited_base_balance: Option<u64>,
    pub deposited_quote_balance: Option<u64>,
    pub withdrawn_base_balance: Option<u64>,
    pub withdrawn_quote_balance: Option<u64>,
    pub starting_value: u64,
    pub base_balance: u64,
    pub quote_balance: u64,
    pub value: u64,
    pub total_txs: u64,
    pub register_date: u64,
    pub status: TraderStatus,
    pub orders: Vec<Order>
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderPair {
    buy: Order,
    sell: Order
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Order {
    pub price: u64,
    pub side: Side,
    pub client_order_id: u64,
    pub is_filled: bool,
    pub owner: String,
    pub order_id: String
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TraderStatus {
    Registered,
    Initialized,
    Decommissioned,
    Stopped
}



#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum GridStatus {
    Violated,
    Idle,
    AwaitingBuy,
    AwaitingSell,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GridPosition {
    pub price: u64,
    pub status: GridStatus,
    pub order: Option<Order>
}

use std::fmt;
use std::fmt::format;

impl fmt::Display for Trader {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut str = "".to_string();
        str.push_str(&format!("[+] BaseBalance: {} Ui: {}\n", self.base_balance, self.base_balance / (10 as u64).pow(self.base_token_info.decimals as u32)));
        str.push_str(&format!("[+] QuoteBalance: {} Ui: {}\n",  self.quote_balance, self.quote_balance / (10 as u64).pow(self.quote_token_info.decimals as u32)));
        str.push_str("[+] Grids\n");
        for grid in &self.grids {
            str.push_str(&format!("[ Price: {}, Status: {:?} ]\n", grid.price, grid.status));

        }
        fmt.write_str(&str).unwrap();
        Ok(())
    }
}
