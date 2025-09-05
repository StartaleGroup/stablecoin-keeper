use anyhow::Result;
use axum::{
    response::Json,
    routing::get,
    Router,
};
use serde_json::Value;

pub struct Server {
    // TODO: Add fields
    // database: Database,
    // config: Config,
}

impl Server {
    pub fn new() -> Self {
        Server {}
    }

    pub async fn start(&self) -> Result<()> {
        let app = Router::new()
            .route("/health", get(health_check))
            .route("/api/v1/users/:address/portfolio", get(get_user_portfolio))
            .route("/api/v1/vaults/stats", get(get_vault_stats));
            // TODO: Add more routes

        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
        println!("API Server running on http://0.0.0.0:3000");
        
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn health_check() -> Json<Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "stablecoin-backend"
    }))
}

async fn get_user_portfolio() -> Json<Value> {
    // TODO: Implement user portfolio endpoint
    Json(serde_json::json!({
        "message": "TODO - implement user portfolio logic"
    }))
}

async fn get_vault_stats() -> Json<Value> {
    // TODO: Implement vault stats endpoint
    Json(serde_json::json!({
        "message": "TODO - implement vault stats logic"
    }))
}

// TODO: Add more API endpoints
// async fn get_pps_history() -> Json<Value> {}
// async fn get_claimable_amount() -> Json<Value> {}
