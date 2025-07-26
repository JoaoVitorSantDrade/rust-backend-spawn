use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Summary {
    #[serde(rename = "totalRequests")]
    pub total_requests: u64,
    #[serde(rename = "totalAmount")]
    pub total_amount: Decimal,
}

#[derive(Deserialize, Serialize)]
pub struct PaymentSummary {
    pub default: Summary,
    pub fallback: Summary,
}
