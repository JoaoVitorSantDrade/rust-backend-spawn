use axum::{
    body::Bytes,
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use rust_decimal::Decimal;
use std::{str::FromStr, sync::atomic::Ordering};

use crate::{
    api::redis,
    appstate::AppState,
    models::{self, data_range::DateRangeParams, summary::PaymentSummary},
};

pub async fn submit_work_handler(State(state): State<AppState>, body: Bytes) -> StatusCode {
    let counter = state.round_robin_counter.fetch_add(1, Ordering::Relaxed);
    let sender = &state.sender_queue[counter % state.sender_queue.len()];

    match sender.send(body).await {
        Ok(_) => StatusCode::OK,

        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

pub async fn purge_payments(State(state): State<AppState>) -> StatusCode {
    match redis::expurgar_todos_pagamentos(&state).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub async fn get_payment_summary(
    State(state): State<AppState>,
    Query(params): Query<DateRangeParams>,
) -> impl IntoResponse {
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
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Falha ao buscar o sumário de pagamentos.".to_string(),
        )
            .into_response(),
    }
}

pub async fn handle_tower_error(err: tower::BoxError) -> StatusCode {
    StatusCode::SERVICE_UNAVAILABLE
}
