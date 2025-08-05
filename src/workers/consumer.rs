use axum::body::Bytes;
use std::{sync::Arc, time::Duration};
use tokio::sync::{RwLock, mpsc::Receiver};

use crate::{
    api::redis::salvar_pagamento,
    appstate::AppState,
    models::{
        payment::Payment,
        processor::{Processor, TipoProcessador},
    },
};

pub async fn worker_processa_pagamento(state: AppState, mut receiver: Receiver<Bytes>) {
    while let Some(body_bytes) = receiver.recv().await {
        let payload: Payment = match simd_json::from_slice(&mut body_bytes.to_vec()) {
            Ok(p) => p,
            Err(_) => {
                continue;
            }
        };

        let payment = Payment {
            correlation_id: payload.correlation_id,
            amount: payload.amount,
            requested_at: None,
            tipo: None,
        };

        processa_pagamento(state.clone(), payment).await;
    }
}
async fn escolher_processador(
    state: &AppState,
    retry: u8,
    fallback_threshold: u8,
) -> (Option<Arc<RwLock<Processor>>>, TipoProcessador) {
    let is_default_failing = state.processors[0].read().await.failing;
    if !is_default_failing {
        return (Some(state.processors[0].clone()), TipoProcessador::Default);
    }

    if retry >= fallback_threshold {
        let is_fallback_failing = state.processors[1].read().await.failing;
        if !is_fallback_failing {
            return (Some(state.processors[1].clone()), TipoProcessador::Fallback);
        }
    }

    (None, TipoProcessador::None)
}

pub async fn processa_pagamento(state: AppState, mut payment: Payment) {
    let mut retry_delay = Duration::from_millis(100);
    let max_retry_delay = Duration::from_millis(700);
    let max_retry_times = 40u8;
    let mut retry_times = 0u8;
    let fallback_threshold =
        (max_retry_times as f32 * (state.retry_default_percentage / 100.0)).floor() as u8;
    payment.update_date();

    loop {
        if retry_times >= max_retry_times {
            return;
        }

        let (processor_opt, tipo) =
            escolher_processador(&state, retry_times, fallback_threshold).await;

        let processor_arc = match processor_opt {
            Some(p) => p,
            None => {
                tokio::time::sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(max_retry_delay);
                retry_times += 1;
                continue;
            }
        };

        let address = {
            let guard = processor_arc.read().await;
            guard.address.clone()
        };
        let payment_url = format!("{}/payments", address);

        let response_result = state
            .http_client
            .post(&payment_url)
            .json(&payment.to_payment_request())
            .send()
            .await;

        match response_result {
            Ok(response) if response.status().is_success() => {
                payment.set_processador(tipo);

                salvar_pagamento(&payment, &state).await;

                return;
            }

            Ok(_) => {
                processor_arc.write().await.failing = true;
            }

            Err(_) => {
                processor_arc.write().await.failing = true;
            }
        }

        tokio::time::sleep(retry_delay).await;
        retry_delay = (retry_delay * 2).min(max_retry_delay);
        retry_times += 1;
    }
}
