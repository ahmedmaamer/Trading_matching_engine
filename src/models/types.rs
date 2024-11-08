use serde::{Serialize, Deserialize};
use ethereum_types::{H160, H256, U256};  // Common types for Ethereum-based projects

    pub type Address = H160;  // Ethereum address as a 20-byte hexadecimal type
pub type Hash = H256;     // 32-byte hash, often used for transaction or data hashes
pub type Decimal = U256;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Fill {
    pub maker_hash: Hash,
    pub taker_hash: Hash,
    pub fill_amount: Decimal,
    pub price: Decimal,
}



#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EIP712DomainSeparator {
    pub name: String,
    pub version: String,
}
impl EIP712DomainSeparator {
    pub fn new(name: String, version: String) -> Self {
        EIP712DomainSeparator { name, version }
    }
}