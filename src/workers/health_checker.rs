use std::{sync::Arc, time::Duration};

use reqwest::Version;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

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
            error!("Processador não teve seu tipo definido!");
            return;
        }
    };

    let processor_arc = state.processors[indice].clone();

    let address = {
        let processor_guard = processor_arc.read().await;
        processor_guard.address.clone() + "/payments/service-health"
    };

    info!(
        "[HEALTH-{:?}] Iniciando monitoramento de saúde para o endereço: {:?}",
        tipo, &address
    );

    loop {
        let min_response_time = processor_arc.read().await.min_response_time;

        match state
            .http_client
            .get(&address)
            .version(Version::HTTP_11)
            .timeout(Duration::from_millis(100 + min_response_time as u64))
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
                            info!(
                                "[HEALTH-{:?}] Status de saúde foi atualizado com sucesso.",
                                tipo
                            );
                            nats_client
                                .publish(
                                    format!("processor.{}.status", indice),
                                    serde_json::to_string(&json).unwrap().into(),
                                )
                                .await
                                .unwrap();
                        }
                        Err(e) => {
                            marcar_como_falho(&processor_arc).await;
                            warn!(
                                "[HEALTH-{:?}] Falha ao decodificar JSON do health check: {}.",
                                tipo, e
                            );
                        }
                    }
                } else {
                    marcar_como_falho(&processor_arc).await;
                    warn!(
                        "[HEALTH-{:?}] Servidor respondeu com status {}.",
                        tipo,
                        _response.status()
                    );
                }
            }
            Err(e) => {
                marcar_como_falho(&processor_arc).await;
                error!("[HEALTH-{:?}] Requisição não foi aceita. {}.", tipo, e);
            }
        };
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn marcar_como_falho(processor_arc: &Arc<RwLock<Processor>>) {
    let mut processor_guard = processor_arc.write().await;
    processor_guard.failing = true;
    processor_guard.min_response_time = 200;
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
