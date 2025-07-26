use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::processor::TipoProcessador;

#[derive(Deserialize, Serialize, Clone)]
pub struct Payment {
    #[serde(rename = "correlationId")]
    pub correlation_id: Uuid,
    pub amount: f64,
    #[serde(rename = "requestedAt")]
    #[serde(default)]
    pub requested_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub tipo: Option<TipoProcessador>,
}

#[derive(Deserialize, Serialize)]
pub struct PaymentRequest {
    #[serde(rename = "correlationId")]
    pub correlation_id: Uuid,
    pub amount: f64,
    #[serde(rename = "requestedAt")]
    #[serde(default)]
    pub requested_at: DateTime<Utc>,
}

impl Payment {
    pub fn update_date(&mut self) {
        self.requested_at.get_or_insert_with(Utc::now);
    }
    pub fn set_processador(&mut self, tipo: TipoProcessador) {
        self.tipo = Some(tipo);
    }
    pub fn to_payment_request(&self) -> PaymentRequest {
        PaymentRequest {
            correlation_id: self.correlation_id,
            amount: self.amount,
            requested_at: self.requested_at.unwrap(),
        }
    }
}
