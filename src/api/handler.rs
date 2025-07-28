use axum::{
    body::Bytes,
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use rust_decimal::Decimal;
use std::{str::FromStr, sync::atomic::Ordering};
use tracing::{error, info, instrument, warn};

use crate::{
    api::redis,
    appstate::AppState,
    models::{self, data_range::DateRangeParams, payment::Payment, summary::PaymentSummary},
};

pub async fn submit_work_handler(State(state): State<AppState>, body: Bytes) -> StatusCode {
    let semaphore = state.fast_furious.clone();

    if let Ok(permit) = semaphore.try_acquire_owned() {
        info!("Executando no caminho rápido (fast path)...");

        let mut bytes_vec = body.to_vec();
        let payload: Payment = match simd_json::from_slice(&mut bytes_vec) {
            Ok(p) => p,
            Err(e) => {
                warn!("Payload inválido no caminho rápido: {}", e);
                return StatusCode::BAD_REQUEST;
            }
        };

        let payment = Payment {
            correlation_id: payload.correlation_id,
            amount: payload.amount,
            requested_at: None,
            tipo: None,
        };

        tokio::spawn(async move {
            let _permit = permit;
            crate::workers::consumer::processa_pagamento(state.clone(), payment).await;
        });

        StatusCode::OK
    } else {
        info!("Caminho rápido ocupado. Usando a fila...");

        let counter = state.round_robin_counter.fetch_add(1, Ordering::Relaxed);
        let sender = &state.sender_queue[counter % state.sender_queue.len()];

        match sender.send(body).await {
            Ok(_) => StatusCode::OK,

            Err(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

#[instrument(skip(state))]
pub async fn purge_payments(State(state): State<AppState>) -> StatusCode {
    info!("Recebida requisição para expurgar todos os pagamentos.");

    match redis::expurgar_todos_pagamentos(&state).await {
        Ok(_) => {
            info!("Banco de dados limpo com sucesso.");
            StatusCode::OK
        }
        Err(e) => {
            error!("Erro ao expurgar o banco de dados: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[instrument(skip(state))]
pub async fn get_payment_summary(
    State(state): State<AppState>,
    Query(params): Query<DateRangeParams>,
) -> impl IntoResponse {
    info!("Iniciando a coleta de sumário de pagamentos.");

    let from_ts = params
        .from
        .map(|dt| dt.timestamp_micros() as u64)
        .unwrap_or(0);
    let to_ts = params
        .to
        .map(|dt| dt.timestamp_micros() as u64)
        .unwrap_or(u64::MAX);

    match redis::coletar_entre_timestamp(&state, from_ts, to_ts).await {
        Ok((default_reqs, default_amt_str, fallback_reqs, fallback_amt_str)) => {
            info!("Sumário agregado recebido do Redis com sucesso.");

            // Constrói o sumário a partir do tuple, usando Decimal::from_str
            let summary = PaymentSummary {
                default: models::summary::Summary {
                    total_requests: default_reqs,
                    total_amount: Decimal::from_str(&default_amt_str).unwrap_or(Decimal::ZERO),
                },
                fallback: models::summary::Summary {
                    total_requests: fallback_reqs,
                    total_amount: Decimal::from_str(&fallback_amt_str).unwrap_or(Decimal::ZERO),
                },
            };
            (StatusCode::OK, Json(summary)).into_response()
        }
        Err(e) => {
            error!("Erro ao coletar sumário de pagamentos: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Falha ao buscar o sumário de pagamentos.".to_string(),
            )
                .into_response()
        }
    }
}

pub async fn handle_tower_error(err: tower::BoxError) -> StatusCode {
    StatusCode::SERVICE_UNAVAILABLE
}
