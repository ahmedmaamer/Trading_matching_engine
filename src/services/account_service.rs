use crate::db::pool::create_pool; // Assuming pool.rs is in the db module
use crate::models::account::Account;
use sqlx::{PgPool};
use bigdecimal::BigDecimal;
use uuid::Uuid;
use ethereum_types::H160;
use std::str::FromStr;
pub async fn create_account_in_db(account: &Account) -> Result<(), sqlx::Error> {
    let pool = create_pool().await?;

    // Generate a unique ID (UUID) for the account
    let account_id = Uuid::new_v4();

    // Check if an account with the same trader address already exists
    let existing_account = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM accounts
        WHERE trader_address = $1
        "#,
        format!("{:?}", account.trader_address) // Ensure the full address is checked
    )
    .fetch_one(&pool)
    .await?;

    // If an account already exists, return an error
    if existing_account.count.unwrap_or(0) > 0 {
        return Err(sqlx::Error::RowNotFound); // Or create a custom error type
    }

    // Insert the new account into the database
    sqlx::query!(
        r#"
        INSERT INTO accounts (id, trader_address, ddx_balance, usd_balance)
        VALUES ($1, $2, $3, $4)
        "#,
        account_id,
        format!("{:?}", account.trader_address), // Ensure the full address is stored
        account.ddx_balance,
        account.usd_balance,
    )
    .execute(&pool)
    .await?;

    Ok(())
}

pub async fn get_account_from_db(address: &H160) -> Result<Account, sqlx::Error> {
    let pool = create_pool().await?;

    // Convert H160 address to string format for querying
    let address_str = format!("{:?}", address); // Ensure correct format: 0x1234567890abcdef1234567890abcdef12345678

    log::info!("Querying for account with address: {}", address_str);

    // Fetch the account from the database
    let row = sqlx::query!(
        r#"
        SELECT trader_address, ddx_balance, usd_balance
        FROM accounts
        WHERE LOWER(trader_address) = LOWER($1)
        "#,
        address_str // Use the string representation of the address for querying
    )
    .fetch_one(&pool)
    .await;

    match row {
        Ok(row) => {
            log::info!("Row fetched: trader_address = {}, ddx_balance = {}, usd_balance = {}",
                       row.trader_address, row.ddx_balance, row.usd_balance);

            // Convert the address back to H160 from string
            let trader_address = H160::from_str(&row.trader_address).map_err(|_| {
                sqlx::Error::ColumnDecode {
                    index: "trader_address".into(),
                    source: Box::new(sqlx::error::Error::Decode("H160 decode error".into())),
                }
            })?;

            // Return the account
            Ok(Account {
                trader_address,
                ddx_balance: row.ddx_balance,
                usd_balance: row.usd_balance,
            })
        }
        Err(_) => {
            log::info!("Account with address {} not found in the database", address_str);
            Err(sqlx::Error::RowNotFound)
        }
    }
}


pub async fn delete_account_from_db(address: &H160) -> Result<(), sqlx::Error> {
    let pool = create_pool().await?;
    
    // Convert H160 address to string format for querying (full address representation)
    let address_str = format!("{:?}", address); // Ensure correct format: 0x1234567890abcdef1234567890abcdef12345678

    let result = sqlx::query!(
        r#"
        DELETE FROM accounts
        WHERE LOWER(trader_address) = LOWER($1)
        "#,
        address_str
    )
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }

    Ok(())
}



// In src/services/account_service.rs

pub async fn update_account_in_db(account: &Account) -> Result<(), sqlx::Error> {
    let pool = create_pool().await?;

    // Update the account in the database
    let result = sqlx::query!(
        r#"
        UPDATE accounts
        SET ddx_balance = $1, usd_balance = $2
        WHERE LOWER(trader_address) = LOWER($3)
        "#,
        account.ddx_balance,
        account.usd_balance,
        format!("{:?}", account.trader_address) // Ensure the full address is stored
    )
    .execute(&pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }

    Ok(())
}






