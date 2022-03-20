use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TokenInfo {
    pub symbol: String,
    pub address: String,
    pub decimals: u64,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Trader {
    pub market_address: String,
    pub base_token_info: TokenInfo,
    pub quote_token_info: TokenInfo,
    pub trader_keypair: String,
    pub base_trader_wallet: String,
    pub quote_trader_wallet: String,
    pub serum_open_orders: Vec<String>,
    pub owner: String,
    pub grids: u64,
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
    pub orders: Vec<OrderPair>
}
#[derive(Serialize, Deserialize, Debug)]
pub struct OrderPair {
    buy: Order,
    sell: Order
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Order {
    pub price: u64,
    pub size: u64,
    pub side: String,
    pub client_order_id: u64,
    pub is_filled: bool,
    pub pair_price: u64,
    pub owner: String,
    pub order_id: u128
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TraderStatus {
    Registered,
    Initialized,
    Decommissioned,
    Stopped
}

