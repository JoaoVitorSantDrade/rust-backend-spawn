use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Summary {
    #[serde(rename = "totalRequests")]
    pub total_requests: u64,
    #[serde(rename = "totalAmount")]
    pub total_amount: f64,
}

#[derive(Deserialize, Serialize)]
pub struct PaymentSummary {
    pub default: Summary,
    pub fallback: Summary,
}
impl Summary {
    pub fn default() -> Self {
        Self {
            total_amount: 0.0,
            total_requests: 0,
        }
    }
}

impl PaymentSummary {
    pub fn default() -> Self {
        Self {
            default: Summary::default(),
            fallback: Summary::default(),
        }
    }
    pub fn build(default: Summary, fallback: Summary) -> Self {
        Self { default, fallback }
    }
}
