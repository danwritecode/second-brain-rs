use anyhow::Result;

use super::DbService;
use super::EmbeddingService;
use crate::models::BrainMatter;

pub struct SearchService {
    db: DbService,
    embedding: EmbeddingService
}

impl SearchService {
    pub async fn new() -> Result<Self> {
        let db = DbService::new().await?;
        let embedding = EmbeddingService::new()?;

        Ok(SearchService {
            db,
            embedding
        })
    }

    pub async fn search(&self, query: &str, limit: i32) -> Result<Vec<BrainMatter>> {
        let search_embeddings = self.embedding.generate_embedding(query).await?;
        let results = self.execute_search(search_embeddings, limit).await?;

        Ok(results)
    }

    async fn execute_search(&self, embedding: Vec<f32>, limit: i32) -> Result<Vec<BrainMatter>> {
        // let query = "SELECT * FROM brain_matter ORDER BY embedding <-> $1 LIMIT 5";
        let query = format!("
            SELECT *
            FROM (
                SELECT *, embedding <-> $1::vector as \"distance\" 
                FROM brain_matter
            ) as I
            WHERE I.distance <= 0.7
            ORDER BY I.distance ASC
            LIMIT {};
        ", limit);
        let results = self.db.read_nearest_neighbor(&query, embedding).await?;
        Ok(results)
    }
}
