#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

mod api;
mod appstate;
mod constantes;
mod models;
mod workers;
use crate::{
    api::{
        handler, http::cria_cliente_http, nats::cria_cliente_nats, redis::estabelecer_pool_conexao,
    },
    appstate::AppState,
    models::{
        payment::Payment,
        processor::{Processor, TipoProcessador},
    },
    workers::{consumer, health_checker, health_consumer},
};
use axum::{
    Router,
    error_handling::HandleErrorLayer,
    routing::{get, post},
};
use reqwest::StatusCode;
use std::{
    env,
    sync::{Arc, atomic::AtomicUsize},
};
use tokio::sync::mpsc;
use tower::{BoxError, ServiceBuilder, buffer::BufferLayer, limit::ConcurrencyLimitLayer};
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    match env::var("AMBIENTE").as_deref() {
        Ok("PROD") => {
            let subscriber = FmtSubscriber::builder()
                .with_env_filter(EnvFilter::from_default_env())
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .expect("setting default subscriber failed");
            info!("Modo de produção ativado!");
        }
        _ => {
            console_subscriber::init();
            info!("Modo de desenvolvedor ativado!");
        }
    }

    let processador_default = Processor::new_async(
        false,
        100,
        env::var("URL_DEFAULT").unwrap_or_else(|_| constantes::URL_DEFAULT.to_string()),
        TipoProcessador::Default,
    );
    let processador_fallback = Processor::new_async(
        false,
        100,
        env::var("URL_FALLBACK").unwrap_or_else(|_| constantes::URL_FALLBACK.to_string()),
        TipoProcessador::Fallback,
    );

    let vc_proc = vec![processador_default, processador_fallback];

    let num_workers = (env::var("NUM_CONSUMER")
        .unwrap_or_else(|_| constantes::NUM_CONSUMER.to_string()))
    .parse()
    .unwrap();

    let mut senders = Vec::with_capacity(num_workers);
    let mut receivers = Vec::with_capacity(num_workers);

    for _ in 0..num_workers {
        let (sender, receiver) = mpsc::unbounded_channel::<Payment>();
        senders.push(sender);
        receivers.push(receiver);
    }

    let nats_client = cria_cliente_nats().await;
    let app_state = AppState {
        http_client: cria_cliente_http(),
        processors: vc_proc,
        redis_pool: estabelecer_pool_conexao().await,
        nats_client: nats_client,
        sender_queue: Arc::new(senders),
        round_robin_counter: Arc::new(AtomicUsize::new(0)),
    };

    info!("Iniciando {} workers consumidores...", num_workers);
    for (i, receiver) in receivers.into_iter().enumerate() {
        tokio::spawn(Box::pin(consumer::worker_processa_pagamento(
            i as u32,
            app_state.clone(),
            receiver,
        )));
    }

    match env::var("ROLE")
        .unwrap_or_else(|_| "LIDER".to_string())
        .as_str()
    {
        "LIDER" => {
            tokio::spawn(health_checker::cria_worker_coleta_saude(app_state.clone()));
        }
        _ => {
            tokio::spawn(health_consumer::cria_worker_confere_saude(
                app_state.clone(),
            ));
        }
    }

    let high_priority_router = Router::new()
        .route("/payments-summary", get(handler::get_payment_summary))
        .route("/purge-payments", post(handler::purge_payments))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_tower_error))
                .layer(BufferLayer::new(64))
                .layer(ConcurrencyLimitLayer::new(20)),
        );

    let low_priority_router = Router::new()
        .route("/payments", post(handler::submit_work_handler))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_tower_error))
                .layer(BufferLayer::new(1024)),
        );
    let app = high_priority_router
        .merge(low_priority_router)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9999").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn handle_tower_error(err: BoxError) -> (StatusCode, String) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        format!("Serviço sobrecarregado ou falha interna: {}", err),
    )
}
