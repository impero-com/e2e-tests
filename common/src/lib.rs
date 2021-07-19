use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PayloadCookies {
    pub message: String,
    pub count: i32,
}
