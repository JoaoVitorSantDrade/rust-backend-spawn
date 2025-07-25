use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use tracing::{error, info, instrument};

use crate::{
    api::redis,
    appstate::AppState,
    models::{
        data_range::DateRangeParams, payment::Payment, processor::TipoProcessador,
        summary::PaymentSummary,
    },
};

pub async fn submit_work_handler(
    State(state): State<AppState>,
    Json(payload): Json<Payment>,
) -> (StatusCode, String) {
    info!(
        "Recebida requisição para processar pagamento: {:?}",
        payload.correlation_id
    );

    match state.sender_queue.send(payload) {
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

    let from_ts = params.from.map(|dt| dt.timestamp() as u64).unwrap_or(0);
    let to_ts = params
        .to
        .map(|dt| dt.timestamp() as u64)
        .unwrap_or(u64::MAX);

    match redis::coletar_entre_timestamp(&state, from_ts, to_ts).await {
        Ok(pagamentos) => {
            info!(
                "Construindo sumário a partir de {} pagamentos recuperados.",
                pagamentos.len()
            );
            let mut summary = PaymentSummary::default();

            for p in pagamentos {
                if let Some(tipo) = p.tipo {
                    match tipo {
                        TipoProcessador::Default => {
                            summary.default.total_requests += 1;
                            summary.default.total_amount += p.amount;
                        }
                        TipoProcessador::Fallback => {
                            summary.fallback.total_requests += 1;
                            summary.fallback.total_amount += p.amount;
                        }
                        TipoProcessador::None => {
                            // Ignora pagamentos com tipo 'None'
                        }
                    }
                }
            }

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
