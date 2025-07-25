use futures::StreamExt;

use crate::{appstate::AppState, models::processor::Processor};

pub async fn cria_worker_confere_saude(state: AppState) {
    let nats_client = state.nats_client;
    let mut sub = nats_client.subscribe("processor.*.status").await.unwrap();

    while let Some(message) = sub.next().await {
        if message.subject.as_str() == "processor.0.status" {
            let processor: Processor = serde_json::from_slice(&message.payload).unwrap();
            let mut processor_guard = state.processors[0].write().await;
            processor_guard.failing = processor.failing;
            processor_guard.min_response_time = processor.min_response_time;
            drop(processor_guard);
        } else if message.subject.as_str() == "processor.1.status" {
            let processor: Processor = serde_json::from_slice(&message.payload).unwrap();
            let mut processor_guard = state.processors[1].write().await;
            processor_guard.failing = processor.failing;
            processor_guard.min_response_time = processor.min_response_time;
            drop(processor_guard);
        }
    }
}
