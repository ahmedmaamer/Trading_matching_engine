    use actix_web::{web, HttpResponse, Responder};
    use serde::{Serialize, Deserialize};
    use crate::models::order::{Order, L2OrderBook};
    use crate::models::types::{Fill, EIP712DomainSeparator, Hash};
    use std::sync::Mutex; // Mutex for thread safety
    use ethereum_types::{H160, H256};
    use std::str::FromStr;
    use crate::db::pool;
    use crate::services::order_service::{add_order_to_book}; // Import necessary service functions
    use crate::services::order_service::AppState as OtherAppState; // Import AppState from order_service
    use crate::services::order_service::get_order_entry_by_hash;
    use crate::services::order_service::delete_order_entry_by_hash; 
    use crate::services::order_service::get_order_book_snapshot;
    use sqlx::PgPool;
    use anyhow::Result;


    #[derive(Serialize, Deserialize)]
    pub struct CreateOrderRequest {
        pub side: String,  // Bid or Ask
        pub amount: String, // Amount as a string to handle large numbers
        pub price: String,  // Price as a string
        pub trader_address: String, // Ethereum address
    }

    #[derive(Serialize, Deserialize)]
    pub struct CreateOrderResponse {
        pub success: bool,
        pub message: String,
        pub fills: Option<Vec<Fill>>,
    }

    pub struct AppState {
        pub order_book: Mutex<L2OrderBook>, // Using Mutex to ensure thread safety for the order book
        pub domain_separator: EIP712DomainSeparator,
        pub db_pool: PgPool,
    }

    impl AppState {
        fn new(domain_separator: EIP712DomainSeparator , db_pool: PgPool) -> Self {
            AppState {
                order_book: Mutex::new(L2OrderBook { asks: Vec::new(), bids: Vec::new() }),
                domain_separator,
                db_pool,
            }
        }
    }

    // Route to create a new order
    pub async fn create_order(
        order_data: web::Json<CreateOrderRequest>,
        app_state: web::Data<AppState>,
    ) -> impl Responder {
        // Parse the request data and create an order object
        let order = Order {
            side: order_data.side.clone().parse().expect("Invalid side"), // Expect side to be valid
            amount: order_data.amount.clone().parse().expect("Invalid amount"),
            price: order_data.price.clone().parse().expect("Invalid price"),
            trader_address: H160::from_str(&order_data.trader_address).expect("Invalid Ethereum address"),
            nonce: H256::from_low_u64_le(0), // Convert integer 0 to H256
            
        };

        // Create a database pool
        let db_pool = pool::create_pool().await.expect("Failed to create DB pool");

        // Lock the Mutex to access the order book
        let mut order_book = app_state.order_book.lock().unwrap();
        let order_book_ref = &mut *order_book;

        // Add order to the order book and try matching
        let fills = add_order_to_book(order, &mut order_book, &app_state.domain_separator, &db_pool).await;
        // Return success or failure message
        if fills.is_empty() {
            HttpResponse::Created().json(CreateOrderResponse {
                success: true,
                message: "Order placed successfully".to_string(),
                fills: None,
            })
        } else {
            HttpResponse::Created().json(CreateOrderResponse {
                success: true,
                message: "Order matched and filled".to_string(),
                fills: Some(fills),
            })
        }
    }

    // Example of initializing EIP712DomainSeparator (ensure you fill in other required fields)
    pub fn initialize_app_state(db_pool: PgPool) -> AppState {
        let domain_separator = EIP712DomainSeparator {
            name: "DDX take-home".to_string(),
            version: "0.1.0".to_string(),
            // Add other required fields here...
        };
        
        AppState::new(domain_separator,db_pool)
    }

    pub async fn get_order_by_hash_route(hash: web::Path<String>, app_state: web::Data<AppState>) -> impl Responder {
        let db_pool = pool::create_pool().await.expect("Failed to create DB pool");
        let hash_str = hash.into_inner(); // Get the hash from the path

        match get_order_entry_by_hash(&db_pool, &H256::from_str(&hash_str).unwrap()).await {
            Ok(Some(order_entry)) => HttpResponse::Ok().json(order_entry),
            Ok(None) => HttpResponse::NotFound().body("Order not found"),
            Err(_) => HttpResponse::InternalServerError().body("Internal Server Error"),
        }
    }

    pub async fn delete_order_entry_by_hash_route(hash: web::Path<String>, app_state: web::Data<AppState>) -> impl Responder {
        let db_pool = pool::create_pool().await.expect("Failed to create DB pool");
        let hash_str = hash.into_inner(); // Get the hash from the path

        match delete_order_entry_by_hash(&db_pool, &H256::from_str(&hash_str).unwrap()).await {
            Ok(deleted) if deleted => HttpResponse::Ok().body("Order deleted successfully"),
            Ok(_) => HttpResponse::NotFound().body("Order not found"),
            Err(_) => HttpResponse::InternalServerError().body("Internal Server Error"),
        }
    }


    pub async fn get_order_book(app_state: web::Data<AppState>) -> impl Responder {
        let db_pool = pool::create_pool().await.expect("Failed to create DB pool");
        match get_order_book_snapshot(&db_pool).await {
            Ok(order_book) => HttpResponse::Ok().json(order_book),
            Err(e) => {
                // Print error message and backtrace if available
                eprintln!("Error fetching order book: {:?}", e);
                if let backtrace = e.backtrace() {
                    eprintln!("Backtrace: {:?}", backtrace);
                }
    
                // Print debug information about `e`
                dbg!(&e);
    
                HttpResponse::InternalServerError().finish()
            }
        }
    }