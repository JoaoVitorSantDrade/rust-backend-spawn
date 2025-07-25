mod api;
mod appstate;
mod constantes;
mod models;
mod workers;
use axum::{
    Router,
    routing::{get, post},
};
use std::{env, sync::Arc};
use tokio::sync::{Mutex, mpsc};
use tracing::info;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

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

#[tokio::main()]
async fn main() {
    match env::var("AMBIENTE").as_deref() {
        // Se a variável de ambiente for "PROD", configure o logger padrão.
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
    let (sender, receiver) = mpsc::unbounded_channel::<Payment>();
    let app_state = AppState {
        http_client: cria_cliente_http(),
        processors: vc_proc,
        redis_pool: estabelecer_pool_conexao().await,
        nats_client: cria_cliente_nats().await,
        sender_queue: sender,
    };

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

    let shared_receiver = Arc::new(Mutex::new(receiver));
    let num_workers = 40;
    info!("Iniciando {} workers consumidores...", num_workers);
    for i in 0..num_workers {
        tokio::spawn(Box::pin(consumer::worker_processa_pagamento(
            i, // Passa um ID para o worker para logging
            app_state.clone(),
            Arc::clone(&shared_receiver),
        )));
    }

    let app = Router::new()
        .route("/payments", post(handler::submit_work_handler))
        .route("/payments-summary", get(handler::get_payment_summary))
        .route("/purge-payments", post(handler::purge_payments))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9999").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
