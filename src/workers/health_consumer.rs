use futures::StreamExt;

use crate::{appstate::AppState, models::processor::Processor};

pub async fn cria_worker_confere_saude(state: AppState) {
    let nats_client = state.nats_client;
    let mut sub = nats_client.subscribe("processor.*.status").await.unwrap();

    while let Some(message) = sub.next().await {
        if let Some(idx_str) = message
            .subject
            .strip_prefix("processor.")
            .and_then(|s| s.strip_suffix(".status"))
        {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(processor_lock) = state.processors.get(idx) {
                    let processor: Processor = serde_json::from_slice(&message.payload).unwrap();
                    let mut processor_guard = processor_lock.write().await;
                    processor_guard.failing = processor.failing;
                    processor_guard.min_response_time = processor.min_response_time;
                }
            }
        }
    }
}
