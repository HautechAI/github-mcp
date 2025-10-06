use serde::{Deserialize, Serialize};

// Canonical shared rate metadata type used across HTTP and tools layers.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RateMeta {
    pub remaining: Option<i32>,
    pub used: Option<i32>,
    pub reset_at: Option<String>,
}
