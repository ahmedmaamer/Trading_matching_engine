use serde::{Serialize, Deserialize};
use bigdecimal::{ ToPrimitive, FromPrimitive, Signed}; // Import Signed for is_negative
use ethereum_types::{H160, H256, U256};
use crate::models::types::{Address, Hash, Fill}; 
use crate::models::order::{Order, OrderSide, L2OrderBook , L2OrderBookGetResponse}; 
use sqlx::{query, PgPool}; 
use serde_json::json;
use bigdecimal::{BigDecimal, num_bigint::ToBigInt};
use crate::models::order::OrderEntry;
use crate::models::types::EIP712DomainSeparator;
use crate::services::account_service;
use std::str::FromStr;
use sqlx::Error;
use serde_json::Value;




/// Converts U256 to BigDecimal
fn u256_to_bigdecimal(value: U256) -> BigDecimal {
    let value_str = value.to_string();
    BigDecimal::parse_bytes(value_str.as_bytes(), 10)
        .expect("Failed to convert U256 to BigDecimal")
}

fn bigdecimal_to_u256(bd: &BigDecimal) -> U256 {
    let big_int = bd.to_bigint().unwrap();
    let bytes = big_int.to_bytes_le().1;
    let mut array = [0u8; 32];
    array[..bytes.len()].copy_from_slice(&bytes);
    U256::from_little_endian(&array)
}
pub async fn match_order(
    order: &Order,
    order_book: &mut L2OrderBook,
    domain: &EIP712DomainSeparator,
    db: &PgPool,
) -> Vec<Fill> {
    let mut fills: Vec<Fill> = Vec::new();
    let mut remaining_amount = order.amount.clone();

    // Identify the opposite and same sides of the order book
    let (opposite_side, same_side) = if order.side == OrderSide::Bid {
        (&mut order_book.asks, &mut order_book.bids)
    } else {
        (&mut order_book.bids, &mut order_book.asks)
    };

    let mut i = 0;
    while i < opposite_side.len() && remaining_amount > BigDecimal::from(0) {
        let existing_order = &mut opposite_side[i];

        // Check if the price conditions match
        let is_opposite_side = (order.side == OrderSide::Bid && existing_order.price <= order.price)
            || (order.side == OrderSide::Ask && existing_order.price >= order.price);

        if is_opposite_side {
            if existing_order.trader_address == order.trader_address {
                // Prevent self-matching
                println!("Self-matching detected: Order from the same trader. No fill created.");
                return Vec::new();
            }

            // Determine the fill amount as the minimum of the remaining and existing order amounts
            let fill_amount = remaining_amount.clone().min(existing_order.amount.clone());

            // Update remaining amount of incoming order and the matched order
            remaining_amount -= fill_amount.clone();
            existing_order.amount -= fill_amount.clone();

            // Create the Fill entry
            let fill = Fill {
                maker_hash: existing_order.eip712_hash(domain),
                taker_hash: order.eip712_hash(domain),
                fill_amount: bigdecimal_to_u256(&fill_amount),
                price: bigdecimal_to_u256(&existing_order.price),
            };

            // Insert the fill into the database
            if let Err(e) = insert_fill(db, &fill).await {
                eprintln!("Failed to insert fill into database: {}", e);
            }

            fills.push(fill);

            // Balance updates for both sides
            if order.side == OrderSide::Bid {
                // Update the bidder's USD balance
                let account_result = account_service::get_account_from_db(&order.trader_address).await;
                if let Ok(mut account) = account_result {
                    let new_usd_balance = account.usd_balance - (fill_amount.clone() * order.price.clone());
                    let new_ddx_balance = account.ddx_balance + fill_amount.clone() ;
                    account.usd_balance = new_usd_balance;
                    account.ddx_balance = new_ddx_balance;
                    if let Err(e) = account_service::update_account_in_db( &account).await {
                        eprintln!("Failed to update bidder's USD balance: {}", e);
                    }
                }

                // Update the seller's DDX balance (opposite side)
                let account_result = account_service::get_account_from_db(&existing_order.trader_address).await;
                if let Ok(mut account) = account_result {
                    let new_ddx_balance = account.ddx_balance - fill_amount.clone();
                    let new_usd_balance = account.usd_balance + (fill_amount * order.price.clone());
                    account.ddx_balance = new_ddx_balance;
                    account.usd_balance = new_usd_balance;
                    if let Err(e) = account_service::update_account_in_db( &account).await {
                        eprintln!("Failed to update seller's DDX balance: {}", e);
                    }
                }
            } else if order.side == OrderSide::Ask {
                // Update the asker's DDX balance
                let account_result = account_service::get_account_from_db(&order.trader_address).await;
                if let Ok(mut account) = account_result {
                    let new_ddx_balance = account.ddx_balance - fill_amount.clone();
                    let new_usd_balance = account.usd_balance + (fill_amount.clone() * existing_order.price.clone());
                    account.ddx_balance = new_ddx_balance;
                    account.usd_balance= new_usd_balance;
                    if let Err(e) = account_service::update_account_in_db(&account).await {
                        eprintln!("Failed to update asker's DDX balance: {}", e);
                    }
                }

                // Update the buyer's USD balance (opposite side)
                let account_result = account_service::get_account_from_db(&existing_order.trader_address).await;
                if let Ok(mut account) = account_result {
                    let new_usd_balance = account.usd_balance - (fill_amount.clone() * existing_order.price.clone());
                    let new_ddx_balance = account.ddx_balance + fill_amount;
                    account.ddx_balance = new_ddx_balance;
                    account.usd_balance = new_usd_balance;
                    if let Err(e) = account_service::update_account_in_db(&account).await {
                        eprintln!("Failed to update buyer's USD balance: {}", e);
                    }
                }
            }

            // Remove fully filled existing orders from opposite side
            if existing_order.amount <= BigDecimal::from(0) {
                opposite_side.remove(i);
            } else {
                i += 1; // Move to the next order if the current one isn't fully filled
            }
        } else {
            i += 1; // Move to the next order if price conditions aren't met
        }
    }

    // If the incoming order is only partially matched, update the same side
    if remaining_amount > BigDecimal::from(0) {
        if let Some(position) = same_side.iter_mut().position(|x| x.trader_address == order.trader_address && x.price == order.price) {
            same_side[position].amount = remaining_amount.clone();
        }
    } else {
        // If fully matched, remove the incoming order from the same side
        if let Some(position) = same_side.iter().position(|x| x.trader_address == order.trader_address && x.price == order.price) {
            same_side.remove(position);
        }
    }

    fills
}




fn validate_order(order: &Order) -> Result<(), String> {
    if order.amount <= BigDecimal::from(0) {
        return Err("Amount must be greater than zero".to_string());
    }
    if order.price <= BigDecimal::from(0) {
        return Err("Price must be greater than zero".to_string());
    }
    if order.side != OrderSide::Bid && order.side != OrderSide::Ask {
        return Err("Invalid order side".to_string());
    }
    Ok(())
}

pub async fn add_order_to_book(
    order: Order,
    order_book: &mut L2OrderBook,
    domain: &EIP712DomainSeparator,
    db: &PgPool,
) -> Vec<Fill> {
    // Ensure the order book is initialized
    if let Err(e) = ensure_empty_order_book(db).await {
        eprintln!("Error ensuring empty order book: {}", e);
    }

    // Validate the order
    if let Err(error) = validate_order(&order) {
        println!("Order validation failed: {}", error);
        return Vec::new();
    }

    // Retrieve the account by trader address
    let trader_address_h160 = H160::from_str(&format!("{:?}", order.trader_address)).unwrap(); // Convert to H160
    let account_result = account_service::get_account_from_db(&trader_address_h160).await;

    // Check if the account exists and has sufficient balance
    let account = match account_result {
        Ok(acc) => acc,
        Err(_) => {
            println!("Account not found for trader address: {:?}", order.trader_address);
            return Vec::new();
        }
    };

    // Check if the trader has enough balance for the order
    if order.side == OrderSide::Bid {
        let amount = order.amount.clone();  
        let price = order.price.clone();
        if account.usd_balance < amount * price {
            println!("Insufficient USD balance for trader: {:?}", order.trader_address);
            return Vec::new();
        }
    } else {
        // For Ask orders, check USD balance
        if account.ddx_balance < order.amount {
            println!("Insufficient DDX balance for trader: {:?}", order.trader_address);
            return Vec::new();
        }
    }
    let hashhh = order.eip712_hash(domain).to_string();
    println!("{}",format!("{:?}",order.eip712_hash(domain)));
    // Create an OrderEntry from the incoming order
    let order_entry = OrderEntry {
        amount: order.amount.clone(),
        price: order.price.clone(),
        trader_address: order.trader_address.clone(),
        eip712_hash: format!("{:?}",order.eip712_hash(domain)),  // Calculate and store the EIP712 hash
    };

    // Insert the order into the order book before matching
    if order.side == OrderSide::Bid {
        let pos = order_book
            .bids
            .iter()
            .position(|x| x.price < order.price)
            .unwrap_or(order_book.bids.len());
        order_book.bids.insert(pos, order_entry); 
    } else {
        let pos = order_book
            .asks
            .iter()
            .position(|x| x.price > order.price)
            .unwrap_or(order_book.asks.len());
        order_book.asks.insert(pos, order_entry); 
    }

    // Perform matching now that the order is in the book
    let fills = match_order(&order, order_book, domain, db).await;
    
    // After matching, if the order has remaining amount, we leave it in the book
    if fills.is_empty() || order.amount > BigDecimal::from(0) {
        // The order is left in the book (no full match), so we update the order book in the DB
        let updated_order_book = L2OrderBook {
            asks: order_book.asks.clone(),
            bids: order_book.bids.clone(),
        };

        if let Err(e) = update_order_book(db, &updated_order_book).await {
            eprintln!("Failed to update order book in database: {}", e);
        }
    }

    fills
}



pub async fn insert_fill(db: &PgPool, fill: &Fill) -> sqlx::Result<()> {
    

    // Convert U256 to String for database compatibility
    let fill_amount =u256_to_bigdecimal( fill.fill_amount); // Convert U256 to String
    let price = u256_to_bigdecimal(fill.price); // Convert U256 to String

    query!(
        "INSERT INTO fills (maker_hash, taker_hash, fill_amount, price) 
         VALUES ($1, $2, $3, $4)",
         format!("{:?}", fill.maker_hash),
         format!("{:?}", fill.taker_hash),
        fill_amount,
        price
    )
    .execute(db)
    .await?; 

    Ok(())
}

pub async fn update_order_book(db: &PgPool, order_book: &L2OrderBook) -> sqlx::Result<()> {
    let asks_json = serde_json::to_value(
        order_book.asks.iter().map(|entry| json!({ "amount": entry.amount, "price": entry.price, "trader_address": entry.trader_address , "eip712_hash":entry.eip712_hash})).collect::<Vec<_>>()
    ).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

    let bids_json = serde_json::to_value(
        order_book.bids.iter().map(|entry| json!({ "amount": entry.amount, "price": entry.price, "trader_address": entry.trader_address , "eip712_hash":entry.eip712_hash})).collect::<Vec<_>>()
    ).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

    query!(
        "UPDATE l2_order_book SET asks = $1, bids = $2 WHERE id = 1",
        asks_json,
        bids_json,
    )
    .execute(db)
    .await?; 

    Ok(())
}

pub async fn ensure_empty_order_book(db: &PgPool) -> sqlx::Result<()> {
    // Check if the table is empty (no records with id = 1)
    let result = query!("SELECT COUNT(*) FROM l2_order_book WHERE id = 1")
        .fetch_one(db)
        .await?;

    // If no record is found, insert an empty order book
    if result.count.unwrap_or(0) == 0 {
        query!(
            "INSERT INTO l2_order_book (id, asks, bids) VALUES (1, $1, $2)",
            json!([]), // Empty asks array
            json!([]), // Empty bids array
        )
        .execute(db)
        .await?;
    }

    Ok(())
}



pub struct AppState {
    pub order_book: L2OrderBook, // No synchronization needed
    pub domain_separator: EIP712DomainSeparator, // Domain separator if needed
}
impl AppState {
    pub fn new(domain_separator: EIP712DomainSeparator) -> Self {
        AppState {
            order_book: L2OrderBook { asks: Vec::new(), bids: Vec::new() },
            domain_separator : EIP712DomainSeparator {
                name: "DDX take-home".to_string(),
                version: "0.1.0".to_string(),
                // Add other required fields here...
            }, // Initialize domain_separator
        }
    }
}

#[derive(sqlx::FromRow)]
struct OrderBookRow {
    asks: Option<Value>,
    bids: Option<Value>,
}

pub async fn get_order_entry_by_hash(db: &PgPool, eip712_hash: &H256) -> Result<Option<OrderEntry>, Error> {
    // Convert H256 hash to string format for querying
    let hash_str = format!("{:?}", eip712_hash);

    // Query the l2_order_book table for the order entry with the specified EIP-712 hash
    let row: OrderBookRow = sqlx::query_as!(
        OrderBookRow,
        r#"
        SELECT asks, bids
        FROM l2_order_book
        WHERE id = 1 -- Assuming you're querying the default order book entry
        "#
    )
    .fetch_one(db)
    .await?;

    // Check if we got any rows back
    if let (Some(asks), Some(bids)) = (row.asks, row.bids) {
        // Search in asks
        if let Some(order_entry) = find_order_entry_in_json(&asks, &hash_str) {
            return Ok(Some(order_entry));
        }
        
        // Search in bids
        if let Some(order_entry) = find_order_entry_in_json(&bids, &hash_str) {
            return Ok(Some(order_entry));
        }
    }

    // If no order entry found, return None
    Ok(None)
}
// Helper function to find an order entry in a JSON array by EIP-712 hash
fn find_order_entry_in_json(json_array: &Value, eip712_hash: &str) -> Option<OrderEntry> {
    if let Some(orders) = json_array.as_array() {
        for order in orders {
            if let Some(hash) = order.get("eip712_hash").and_then(Value::as_str) {
                if hash == eip712_hash {
                    // Extract fields and create an OrderEntry instance
                    return Some(OrderEntry {
                        amount: BigDecimal::from_str(order.get("amount")?.as_str()?).ok()?, // Convert as needed
                        price: BigDecimal::from_str(order.get("price")?.as_str()?).ok()?,   // Convert as needed
                        trader_address: H160::from_str(order.get("trader_address")?.as_str()?).ok()?, // Convert string to H160
                        eip712_hash: hash.to_string(),
                    });
                }
            }
        }
    }
    
    None // Return None if not found
}

pub async fn delete_order_entry_by_hash(db: &PgPool, eip712_hash: &H256) -> Result<bool, anyhow::Error> {
    // Convert H256 hash to string format for querying

    let hash_str = format!("{:?}", eip712_hash);

    // Fetch the current order book
    let row: OrderBookRow = sqlx::query_as!(
        OrderBookRow,
        r#"
        SELECT asks, bids
        FROM l2_order_book
        WHERE id = 1 -- Assuming you're querying the default order book entry
        "#
    )
    .fetch_one(db)
    .await?;

    // Check if asks and bids are Some and deserialize them
    let mut asks: Value = row.asks.ok_or_else(|| anyhow::anyhow!("Asks not found"))?;
    let mut bids: Value = row.bids.ok_or_else(|| anyhow::anyhow!("Bids not found"))?;

    // Flag to check if the order was found and deleted
    let mut order_found = false;

    // Remove the order from asks
    if let Some(orders) = asks.as_array_mut() {
        orders.retain(|order| {
            if let Some(hash) = order.get("eip712_hash").and_then(Value::as_str) {
                if hash == hash_str {
                    order_found = true; // Mark as found
                    return false; // Remove this order
                }
            }
            true // Keep this order
        });
    }

    // Remove the order from bids
    if let Some(orders) = bids.as_array_mut() {
        orders.retain(|order| {
            if let Some(hash) = order.get("eip712_hash").and_then(Value::as_str) {
                if hash == hash_str {
                    order_found = true; // Mark as found
                    return false; // Remove this order
                }
            }
            true // Keep this order
        });
    }

    // If no order was found, return false
    if !order_found {
        return Ok(false);
    }

    // Update the order book in the database
    sqlx::query!(
        r#"
        UPDATE l2_order_book
        SET asks = $1, bids = $2
        WHERE id = 1 -- Assuming you're updating the default order book entry
        "#,
        serde_json::to_value(asks)?, // Serialize back to JSON
        serde_json::to_value(bids)?  // Serialize back to JSON
    )
    .execute(db)
    .await?;

    Ok(true) // Return true indicating the order was deleted
}



pub async fn get_order_book_snapshot(db: &PgPool) -> Result<L2OrderBookGetResponse, anyhow::Error> {
    // Query the current order book
    let row: OrderBookRow = sqlx::query_as!(
        OrderBookRow,
        r#"
        SELECT asks, bids
        FROM l2_order_book
        WHERE id = 1
        "#
    )
    .fetch_one(db)
    .await
    .map_err(|e| {
        eprintln!("Database query failed: {:?}", e);
        anyhow::anyhow!("Database query error")
    })?;

    // Deserialize the JSON arrays
    let asks: Value = row.asks.unwrap_or_default();
    let bids: Value = row.bids.unwrap_or_default();

    // Create the L2OrderBook from the raw data
    let order_book = L2OrderBook {
        asks: extract_best_orders(&asks, 50),
        bids: extract_best_orders(&bids, 50),
    };

    // Create the L2OrderBookGetResponse
    let response = L2OrderBookGetResponse {
        best_asks: order_book.asks,
        best_bids: order_book.bids,
    };

    Ok(response)
}
fn extract_best_orders(json_array: &Value, limit: usize) -> Vec<OrderEntry> {
    if let Some(orders) = json_array.as_array() {
        let mut order_vec: Vec<OrderEntry> = orders.iter()
            .filter_map(|order| {
                // Attempt to get and parse each field, logging issues if they occur
                let price_str = order.get("price")?.as_str()?;
                let amount_str = order.get("amount")?.as_str()?;

                // Convert strings to BigDecimal
                let price = BigDecimal::from_str(price_str).ok()?;
                let amount = BigDecimal::from_str(amount_str).ok()?;

                let eip712_hash = order.get("eip712_hash")?.as_str()?.to_string();
                let trader_address_str = order.get("trader_address")?.as_str()?;
                let trader_address = H160::from_str(trader_address_str).ok()?;

                Some(OrderEntry {
                    price,
                    amount,
                    eip712_hash,
                    trader_address,
                })
            })
            .collect();

        // Sort by price (ascending for asks, descending for bids)
        order_vec.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());

        // Return the top `limit` orders
        return order_vec.into_iter().take(limit).collect();
    }

    Vec::new()
}
