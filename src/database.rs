use anyhow::Result;
use sqlx::{PgPool, Pool, Postgres};

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        Ok(Database { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        // TODO: Add database migrations
        // sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    // TODO: Add database methods
    // pub async fn insert_event(&self, event: &Event) -> Result<()> {}
    // pub async fn get_user_balance(&self, address: &str) -> Result<Option<Balance>> {}
    // pub async fn update_stats(&self, stats: &Stats) -> Result<()> {}
}
