use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Processor {
    pub failing: bool,
    #[serde(rename = "minResponseTime")]
    pub min_response_time: u64,
    #[serde(default = "default_address")]
    pub address: String,
    #[serde(default = "default_tipo")]
    pub tipo: TipoProcessador,
}

impl Processor {
    pub fn new_async(
        failing: bool,
        min_response_time: u64,
        address: String,
        tipo: TipoProcessador,
    ) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            failing,
            min_response_time,
            address,
            tipo,
        }))
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum TipoProcessador {
    Default,
    Fallback,
    None,
}

fn default_tipo() -> TipoProcessador {
    TipoProcessador::None
}

fn default_address() -> String {
    String::new()
}
