use axum::{
    body::Bytes,
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use rust_decimal::Decimal;
use std::{str::FromStr, sync::atomic::Ordering};
use tracing::{error, info, instrument};

use crate::{
    api::redis,
    appstate::AppState,
    models::{self, data_range::DateRangeParams, summary::PaymentSummary},
};

pub async fn submit_work_handler(
    State(state): State<AppState>,
    body: Bytes,
) -> (StatusCode, String) {
    let counter = state.round_robin_counter.fetch_add(1, Ordering::Relaxed);
    let sender = &state.sender_queue[counter % state.sender_queue.len()];
    match sender.send(body) {
        Ok(_) => {
            info!("Pagamento recebido com sucesso!");
            (
                StatusCode::OK,
                "Pagamento recebido com sucesso!".to_string(),
            )
        }
        Err(e) => {
            error!("Falha ao enviar pagamento para a fila de trabalho: {}", e);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Falha ao enviar pagamento para a fila de trabalho: {}", e),
            )
        }
    }
}

#[instrument(skip(state))]
pub async fn purge_payments(State(state): State<AppState>) -> impl IntoResponse {
    info!("Recebida requisição para expurgar todos os pagamentos.");

    match redis::expurgar_todos_pagamentos(&state).await {
        Ok(_) => {
            info!("Banco de dados limpo com sucesso.");
            (
                StatusCode::OK,
                Json(
                    serde_json::json!({ "message": "Todos os dados foram removidos com sucesso." }),
                ),
            )
                .into_response()
        }
        Err(e) => {
            error!("Erro ao expurgar o banco de dados: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Falha ao limpar o banco de dados.".to_string(),
            )
                .into_response()
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
