use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatWsRequest {
    pub chat: String,
    pub context: Vec<String>,
    #[serde(rename = "HEADERS")]
    pub headers: ChatWsRequestHeaders,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatWsRequestHeaders {
    #[serde(rename = "HX-Request")]
    pub hx_request: String,
    #[serde(rename = "HX-Trigger")]
    pub hx_trigger: String,
    #[serde(rename = "HX-Trigger-Name")]
    pub hx_trigger_name: Value,
    #[serde(rename = "HX-Target")]
    pub hx_target: String,
    #[serde(rename = "HX-Current-URL")]
    pub hx_current_url: String,
}

