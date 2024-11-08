use actix_web::{App, HttpServer, web};
use actix_cors::Cors;
use crate::routes::account_routes::create_account;
use crate::routes::account_routes::get_account;
use crate::routes::account_routes::delete_account;
use crate::routes::account_routes::update_account;
use crate::routes::order_routes::create_order;
use crate::routes::order_routes::initialize_app_state; // Import the AppState initialization function
use crate::routes::order_routes::get_order_by_hash_route;
use crate::routes::order_routes::delete_order_entry_by_hash_route;
use crate::routes::order_routes::get_order_book;
use sqlx::PgPool;
use dotenv::dotenv;
mod routes;
mod services;
mod db;
mod models;




#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();

    let db_pool = db::pool::create_pool().await.unwrap();

    let app_state = web::Data::new(initialize_app_state(db_pool));

    HttpServer::new(move || {
        App::new()
            .app_data(web::JsonConfig::default().limit(4096)) // Increase if needed
            .wrap(Cors::permissive())
            .app_data(app_state.clone()) // Pass the application state
            .route("/accounts", web::post().to(create_account))
            .route("/accounts/{trader_address}", web::get().to(get_account))
            .route("/accounts/{trader_address}", web::delete().to(delete_account)) 
            .route("/orders", web::post().to(create_order)) // Route for adding an order
            .route("/update_account", web::put().to(update_account))
            .route("/orders/{hash}", web::get().to(get_order_by_hash_route)) // Route for getting an order by its hash
            .route("/orders/{hash}", web::delete().to(delete_order_entry_by_hash_route)) // Route for getting an order by its hash
            .route("/book", web::get().to(get_order_book)) // Add the new route

    })
    .bind("127.0.0.1:4321")?
    .run()
    .await
}
