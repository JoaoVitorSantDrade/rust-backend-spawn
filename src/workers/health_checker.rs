use std::{sync::Arc, time::Duration};

use reqwest::Version;
use tokio::sync::RwLock;

use crate::{
    appstate::AppState,
    models::processor::{Processor, TipoProcessador},
};

pub async fn coleta_saude_processador(state: AppState, tipo: TipoProcessador) {
    let nats_client = state.nats_client;

    let indice = match tipo {
        TipoProcessador::Default => 0,
        TipoProcessador::Fallback => 1,
        TipoProcessador::None => {
            return;
        }
    };

    let processor_arc = state.processors[indice].clone();

    let address = {
        let processor_guard = processor_arc.read().await;
        processor_guard.address.clone() + "/payments/service-health"
    };

    loop {
        let min_response_time = processor_arc.read().await.min_response_time;

        match state
            .http_client
            .get(&address)
            .version(Version::HTTP_11)
            .send()
            .await
        {
            Ok(_response) => {
                if _response.status().is_success() {
                    match _response.json::<Processor>().await {
                        Ok(json) => {
                            let mut processor_guard = processor_arc.write().await;
                            processor_guard.failing = json.failing;
                            processor_guard.min_response_time = json.min_response_time;
                            drop(processor_guard);

                            nats_client
                                .publish(
                                    format!("processor.{}.status", indice),
                                    serde_json::to_string(&json).unwrap().into(),
                                )
                                .await
                                .unwrap();
                        }
                        Err(_) => {
                            marcar_como_falho(&processor_arc).await;
                        }
                    }
                } else {
                    marcar_como_falho(&processor_arc).await;
                }
            }
            Err(_) => {
                marcar_como_falho(&processor_arc).await;
            }
        };
        tokio::time::sleep(Duration::from_secs(5) + Duration::from_millis(min_response_time)).await;
    }
}

async fn marcar_como_falho(processor_arc: &Arc<RwLock<Processor>>) {
    let mut processor_guard = processor_arc.write().await;
    processor_guard.failing = true;
    processor_guard.min_response_time = 500;
    drop(processor_guard);
}

pub async fn cria_worker_coleta_saude(state: AppState) {
    tokio::spawn(Box::pin(coleta_saude_processador(
        state.clone(),
        TipoProcessador::Default,
    )));
    tokio::spawn(Box::pin(coleta_saude_processador(
        state.clone(),
        TipoProcessador::Fallback,
    )));
}
