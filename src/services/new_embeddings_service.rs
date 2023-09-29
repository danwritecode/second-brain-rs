use anyhow::Result;

use super::DbService;
use super::EmbeddingService;
use crate::models::BrainMatter;

pub struct GenerateEmbeddingsService {
    db: DbService,
    embedding: EmbeddingService
}

impl GenerateEmbeddingsService {
    pub async fn new() -> Result<Self> {
        let db = DbService::new().await?;
        let embedding = EmbeddingService::new()?;

        Ok(GenerateEmbeddingsService {
            db,
            embedding
        })
    }

    pub async fn generate(&self, doc_title: String, text: String) -> Result<()> {
        let title = doc_title.split(" ").map(|w| w.to_lowercase()).collect::<Vec<String>>().join("_");
        let chunks = text.lines().map(|l| l.to_string()).collect::<Vec<String>>();

        Ok(())
    }

    async fn generate_embedding(&self, query: &str, limit: i32) -> Result<Vec<BrainMatter>> {
        let search_embeddings = self.embedding.generate_embedding(query).await?;
        let results = self.execute_search(search_embeddings, limit).await?;

        Ok(results)
    }

    async fn execute_search(&self, embedding: Vec<f32>, limit: i32) -> Result<Vec<BrainMatter>> {
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
