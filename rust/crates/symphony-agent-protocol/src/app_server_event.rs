use serde::{Deserialize, Serialize};

use crate::{ProtocolMethodCategory, ProtocolMethodKind};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppServerEvent {
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<serde_json::Value>,
}

impl AppServerEvent {
    pub fn method_kind(&self) -> ProtocolMethodKind {
        ProtocolMethodKind::from_method(self.method.as_str())
    }

    pub fn method_category(&self) -> ProtocolMethodCategory {
        self.method_kind().category()
    }
}
