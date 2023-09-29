use openai::{embeddings::{Embeddings, Embedding}, set_key};
use dotenv::dotenv;
use anyhow::Result;


pub struct EmbeddingService {}

impl EmbeddingService {
    pub fn new() -> Result<Self> {
        dotenv().ok();
        set_key(std::env::var("OPENAI_KEY")?);

        Ok(EmbeddingService { })
    }

    pub async fn generate_embedding(&self, vals: &str) -> Result<Vec<f32>> {
        let result = Embedding::create("text-embedding-ada-002", vals, "dan").await?;
        let embedding:Vec<f32> = result.vec
            .into_iter()
            .map(|e| e as f32)
            .collect();

        Ok(embedding)
    }

    pub async fn generate_embeddings(&self, vals: Vec<&str>) -> Result<Embeddings> {
        let result = Embeddings::create("text-embedding-ada-002", vals, "dan").await?;
        Ok(result)
    }
}
