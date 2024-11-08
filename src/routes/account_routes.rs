use actix_web::{web, HttpResponse};
use ethereum_types::H160;
use std::str::FromStr;
use crate::services::account_service;
use crate::models::account::Account;
use crate::services::account_service::get_account_from_db;
use crate::services::account_service::delete_account_from_db;

pub async fn create_account(account: web::Json<Account>) -> HttpResponse {
    let account_inner = account.into_inner();
    match account_service::create_account_in_db(&account_inner).await {
        Ok(_) => HttpResponse::Created().json(account_inner),
        Err(_) => HttpResponse::InternalServerError().body("Failed to create account"),
    }
}

pub async fn get_account(trader_address: web::Path<String>) -> HttpResponse {
    let trader_address_str = trader_address.into_inner(); // Extract trader_address

    // Convert trader_address from String to H160
    let trader_address_h160 = match H160::from_str(&trader_address_str) {
        Ok(address) => address,
        Err(_) => return HttpResponse::BadRequest().body("Invalid Ethereum address"),
    };

    match get_account_from_db(&trader_address_h160).await {
        Ok(account) => HttpResponse::Ok().json(account), // Return the account as JSON
        Err(_) => HttpResponse::NotFound().body("Account not found"),
    }
}

pub async fn delete_account(trader_address: web::Path<String>) -> HttpResponse {
    let trader_address_str = trader_address.into_inner(); // Extract trader_address

    // Convert trader_address from String to H160
    let trader_address_h160 = match H160::from_str(&trader_address_str) {
        Ok(address) => address,
        Err(_) => return HttpResponse::BadRequest().body("Invalid Ethereum address"),
    };

    match delete_account_from_db(&trader_address_h160).await {
        Ok(_) => HttpResponse::NoContent().finish(), // Return no content on success
        Err(_) => HttpResponse::NotFound().body("Account not found"),
    }
}



pub async fn update_account(account: web::Json<Account>) -> HttpResponse {
    let account_inner = account.into_inner();
    
    // Convert the trader_address from Account back to H160 for validation
    let trader_address_h160 = match H160::from_str(&format!("{:?}", account_inner.trader_address)) {
        Ok(address) => address,
        Err(_) => return HttpResponse::BadRequest().body("Invalid Ethereum address"),
    };

    // Update the account in the database
    match account_service::update_account_in_db(&account_inner).await {
        Ok(_) => HttpResponse::Ok().json(account_inner), // Return the updated account as JSON
        Err(sqlx::Error::RowNotFound) => HttpResponse::NotFound().body("Account not found"),
        Err(_) => HttpResponse::InternalServerError().body("Failed to update account"),
    }
}