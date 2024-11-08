CREATE TABLE IF NOT EXISTS l2_order_book   (
    id SERIAL PRIMARY KEY,                             
    asks JSONB,                                       
    bids JSONB,                                       
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP    
);

CREATE TABLE IF NOT EXISTS fills  (
    maker_hash TEXT NOT NULL,
    taker_hash TEXT NOT NULL,
    fill_amount NUMERIC(38,18),
    price NUMERIC(38,18),
    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
);


CREATE TABLE IF NOT EXISTS accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    trader_address VARCHAR(42) NOT NULL,
    ddx_balance NUMERIC NOT NULL,
    usd_balance NUMERIC NOT NULL
);

