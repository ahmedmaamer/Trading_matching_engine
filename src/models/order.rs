use serde::{Serialize, Deserialize};
use bigdecimal::BigDecimal;
use crate::models::types::Hash;
use sha3::{Digest, Keccak256}; // For keccak hashing
use ethereum_types::{H160, H256}; // Ethereum types
use crate::models::types::Address;
use crate::models::types::EIP712DomainSeparator;
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum OrderSide {
    Bid,
    Ask,
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl FromStr for OrderSide {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bid" => Ok(OrderSide::Bid),
            "ask" => Ok(OrderSide::Ask),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Order {
    pub amount: BigDecimal,  // Amount of asset
    pub nonce: Hash,         // Unique order identifier (nonce)
    pub price: BigDecimal,   // Price per unit
    pub side: OrderSide,     // 'Bid' or 'Ask'
    pub trader_address: Address, // Trader's Ethereum address
}

impl Order {
    // EIP-712 hash generation
    pub fn eip712_hash(&self, domain: &EIP712DomainSeparator) -> Hash {
        let domain_hash = self.get_domain_separator_hash(domain);
        let order_hash = self.get_order_hash();

        // Combine domain separator hash and order hash for final message hash
        let combined = [domain_hash.as_bytes(), order_hash.as_bytes()].concat();
        let final_hash = Keccak256::digest(&combined);

        H256::from_slice(&final_hash)
    }

    fn get_domain_separator_hash(&self, domain: &EIP712DomainSeparator) -> H256 {
        // Generate the domain separator as per EIP-712 standard
        let domain_data = format!("{}:{}", domain.name, domain.version);
        let domain_hash = Keccak256::digest(domain_data.as_bytes());

        H256::from_slice(&domain_hash)
    }

    fn get_order_hash(&self) -> H256 {
        // Serialize the order fields according to EIP-712
        let serialized_order = format!(
            "{}:{}:{}:{}:{}",
            self.amount.to_string(),
            self.nonce.to_string(),
            self.price.to_string(),
            self.side.to_string(),
            self.trader_address.to_string()
        );

        let order_hash = Keccak256::digest(serialized_order.as_bytes());

        H256::from_slice(&order_hash)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrderEntry {
    pub amount: BigDecimal,
    pub price: BigDecimal,
    pub trader_address: Address,
    pub eip712_hash: String,
}

impl OrderEntry {
    // EIP-712 hash generation for OrderEntry
    pub fn eip712_hash(&self, domain: &EIP712DomainSeparator) -> Hash {
        let domain_hash = self.get_domain_separator_hash(domain);
        let order_hash = self.get_order_hash();

        // Combine domain separator hash and order hash for final message hash
        let combined = [domain_hash.as_bytes(), order_hash.as_bytes()].concat();
        let final_hash = Keccak256::digest(&combined);

        H256::from_slice(&final_hash)
    }

    fn get_domain_separator_hash(&self, domain: &EIP712DomainSeparator) -> H256 {
        // Generate the domain separator as per EIP-712 standard
        let domain_data = format!("{}:{}", domain.name, domain.version);
        let domain_hash = Keccak256::digest(domain_data.as_bytes());

        H256::from_slice(&domain_hash)
    }

    fn get_order_hash(&self) -> H256 {
        // Serialize the order fields according to EIP-712
        let serialized_order = format!(
            "{}:{}:{}",
            self.amount.to_string(),
            self.price.to_string(),
            self.trader_address.to_string()
        );

        let order_hash = Keccak256::digest(serialized_order.as_bytes());

        H256::from_slice(&order_hash)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct L2OrderBook {
    pub bids: Vec<OrderEntry>, 
    pub asks: Vec<OrderEntry>, 
}

#[derive(Serialize, Deserialize)]
pub struct L2OrderBookGetResponse {
    pub best_asks: Vec<OrderEntry>,
    pub best_bids: Vec<OrderEntry>,
}

// Default implementation for Order
impl Default for Order {
    fn default() -> Self {
        Order {
            amount: BigDecimal::from(0),
            price: BigDecimal::from(0),
            side: OrderSide::Bid,
            nonce: H256::zero(),             // Default nonce to 0
            trader_address: H160::zero(),     // Default trader address to zero
        }
    }
}