use serde::{Deserialize, Serialize};

/// A generic entity stored in the PrivStack entity engine.
///
/// All plugin data flows through this type. The `data` field holds
/// arbitrary JSON whose structure is defined by the plugin's schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub entity_type: String,
    pub data: serde_json::Value,
    pub created_at: i64,
    pub modified_at: i64,
    pub created_by: String,
}

impl Entity {
    /// Extract a string value from `data` using a JSON pointer (e.g., "/title").
    pub fn get_str(&self, pointer: &str) -> Option<&str> {
        self.data.pointer(pointer).and_then(|v| v.as_str())
    }

    /// Extract a boolean value from `data` using a JSON pointer.
    pub fn get_bool(&self, pointer: &str) -> Option<bool> {
        self.data.pointer(pointer).and_then(|v| v.as_bool())
    }

    /// Extract a numeric value from `data` using a JSON pointer.
    pub fn get_number(&self, pointer: &str) -> Option<f64> {
        self.data.pointer(pointer).and_then(|v| v.as_f64())
    }
}
