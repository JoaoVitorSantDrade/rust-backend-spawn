use crate::constantes;
use std::time::Duration;

pub async fn cria_cliente_nats() -> async_nats::Client {
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| constantes::NATS_URL.to_string());
    let max_tentativas = 5u8;
    let delay = 5;

    for tentativa in 1..=max_tentativas {
        match async_nats::connect(&nats_url).await {
            Ok(cliente) => {
                return cliente;
            }

            Err(_) => {
                if tentativa < max_tentativas {
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
