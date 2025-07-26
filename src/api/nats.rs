use crate::constantes;
use std::time::Duration;
use tracing::{info, warn};

pub async fn cria_cliente_nats() -> async_nats::Client {
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| constantes::NATS_URL.to_string());
    let max_tentativas = 5u8;
    let delay = 5;

    for tentativa in 1..=max_tentativas {
        info!(
            "Tentando conectar ao NATS (tentativa {}/{})...",
            tentativa, max_tentativas
        );
        match async_nats::connect(&nats_url).await {
            Ok(cliente) => {
                info!("✅ Conectado ao NATS com sucesso!");
                return cliente;
            }

            Err(e) => {
                warn!("Falha ao conectar ao NATS: {}", e);

                if tentativa < max_tentativas {
                    warn!("Tentando novamente em {} segundos...", &delay);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }
    }

    panic!(
        "❌ Não foi possível conectar ao NATS após {} tentativas.",
        max_tentativas
    );
}
