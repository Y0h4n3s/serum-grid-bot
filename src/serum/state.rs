use serum_dex::matching::Side;

#[derive(Debug, Clone, Copy)]
pub struct Order {
    pub side: Side,
    pub price: u64,
    pub client_id: u64,
    pub owner: [u64; 4],
    pub order_id: u128,
}

