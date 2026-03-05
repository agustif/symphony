use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppServerEvent {
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<serde_json::Value>,
}
