use serde::{Deserialize, Serialize};
use bigdecimal::BigDecimal;
use crate::models::types::Address;
#[derive(Serialize, Deserialize, Debug)]
pub struct Account {
    pub trader_address: Address,  // Ethereum address (20 bytes)
    pub ddx_balance: BigDecimal,
    pub usd_balance: BigDecimal,
}
