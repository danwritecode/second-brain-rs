use pgvector::Vector;
use anyhow::Result;
use dotenv::dotenv;
use sqlx::{Pool, Postgres, postgres::PgRow, Row};

use crate::models::BrainMatter;

pub struct DbService {
    pool: Pool<Postgres>
}

impl DbService {
    pub async fn new() -> Result<Self> {
        dotenv().ok();
        let db_url = std::env::var("DATABASE_URL")?;
        let pool = sqlx::PgPool::connect(&db_url).await?;
        
        Ok(DbService {
            pool
        })
    }

    pub async fn read(&self, query: &str) -> Result<Vec<PgRow>> {
        let rows = sqlx::query(query)
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }

    pub async fn read_nearest_neighbor(&self, query: &str, embedding: Vec<f32>) -> Result<Vec<BrainMatter>> {
        let rows = sqlx::query(query)
            .bind(Vector::from(embedding))
            .map(|row: PgRow| {
                BrainMatter {
                    id: row.get("id"),
                    body: row.get("body"),
                    document_position: row.get("document_position"),
                    position_type: row.get("position_type"), 
                    document_url: row.get("document_url")
                }    
            })
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }
}
