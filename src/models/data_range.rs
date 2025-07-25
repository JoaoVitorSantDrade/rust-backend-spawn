use chrono::{DateTime, Utc};
use serde::Deserialize;
#[derive(Deserialize, Debug)]
pub struct DateRangeParams {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}
