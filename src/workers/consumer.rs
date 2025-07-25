use std::{sync::Arc, time::Duration};

use tokio::{
    sync::{RwLock, mpsc::UnboundedReceiver},
    task,
};
use tracing::{error, info, warn};

use crate::{
    api::redis::salvar_pagamento,
    appstate::AppState,
    models::{
        payment::Payment,
        processor::{Processor, TipoProcessador},
    },
};

pub async fn cria_worker_consumidor(receptor: UnboundedReceiver<Payment>, state: AppState) {
    info!("[WORKER CONSUMIDOR] Iniciado!");
    let mut rx = receptor;
    while let Some(payment) = rx.recv().await {
        info!("📦 Pagamento recebido: ID {}", payment.correlation_id);
        tokio::spawn(processa_pagamento(state.clone(), payment));
        info!("🧵 Task criada para lidar com pagamento");
    }
}
async fn escolher_processador(
    state: &AppState,
) -> (Option<Arc<RwLock<Processor>>>, TipoProcessador) {
    let is_default_failing = state.processors[0].read().await.failing;
    if !is_default_failing {
        return (Some(state.processors[0].clone()), TipoProcessador::Default);
    }

    let is_fallback_failing = state.processors[1].read().await.failing;
    if !is_fallback_failing {
        return (Some(state.processors[1].clone()), TipoProcessador::Fallback);
    }

    (None, TipoProcessador::None)
}

async fn processa_pagamento(state: AppState, mut payment: Payment) {
    let task_id = task::id();
    let mut retry_delay = Duration::from_secs(1);
    let max_retry_delay = Duration::from_secs(4);
    let max_retry_times = 5u8;
    let mut retry_times = 0u8;

    loop {
        if retry_times >= max_retry_times {
            error!(
                "[TASK {}] - [PROCESSAMENTO-{}] Número máximo de {} tentativas atingido. ❌ Encerrando task",
                task_id, payment.correlation_id, max_retry_times
            );
            return;
        }

        let (processor_opt, tipo) = escolher_processador(&state).await;

        let processor_arc = match processor_opt {
            Some(p) => {
                info!(
                    "[TASK {}] - [PROCESSAMENTO-{}] Processador {:?} escolhido.",
                    task_id, payment.correlation_id, tipo
                );
                p
            }
            None => {
                warn!(
                    "[TASK {}] - [PROCESSAMENTO-{}] Nenhum processador disponível. Tentando novamente em {:?}...",
                    task_id, payment.correlation_id, retry_delay
                );
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
        payment.update_date();
        let response_result = state
            .http_client
            .post(&payment_url)
            .json(&payment.to_payment_request())
            .send()
            .await;

        match response_result {
            Ok(response) if response.status().is_success() => {
                info!(
                    "[TASK {}] - [PROCESSAMENTO-{}] Pagamento enviado com sucesso para o processador {:?}.",
                    task_id, payment.correlation_id, tipo
                );
                payment.set_processador(tipo);
                salvar_pagamento(&payment, &state).await;
                info!(
                    "[TASK {}] - [PROCESSAMENTO-{}] Pagamento salvo no Redis.",
                    task_id, payment.correlation_id
                );
                return;
            }

            Ok(failed_response) => {
                let status = failed_response.status();

                let error_body = failed_response.text().await.unwrap_or_default();
                warn!(
                    "[TASK {}] - [PROCESSAMENTO-{}] Processador {:?} respondeu com erro: {}. Corpo: '{}'. Marcando como falho.",
                    task_id, payment.correlation_id, tipo, status, error_body
                );
                processor_arc.write().await.failing = true;
            }

            Err(e) => {
                error!(
                    "[TASK {}] - [PROCESSAMENTO-{}] Erro de requisição para o processador {:?}: {}. Marcando como falho.",
                    task_id, payment.correlation_id, tipo, e
                );
                processor_arc.write().await.failing = true;
            }
        }

        warn!(
            "[TASK {}] - [PROCESSAMENTO-{}] Falha na tentativa. Esperando {:?} para tentar novamente.",
            task_id, payment.correlation_id, retry_delay
        );
        tokio::time::sleep(retry_delay).await;
        retry_delay = (retry_delay * 2).min(max_retry_delay);
        retry_times += 1;
    }
}
