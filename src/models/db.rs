use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct BrainMatter {
    pub id: i32,
    pub body: String,
    pub document_position: Option<i16>,
    pub position_type: Option<String>,
    pub document_url: Option<String>
}
