use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppServerEvent {
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}
