use deadpool_redis::{
    Config, Manager, PoolConfig, Runtime,
    redis::{RedisError, Script},
};
use tracing::{info, warn};

use std::{env, time::Duration};

use crate::{appstate::AppState, constantes, models};

pub async fn estabelecer_pool_conexao()
-> deadpool::managed::Pool<Manager, deadpool_redis::Connection> {
    let db_url = env::var("DB_URL").unwrap_or_else(|_| constantes::REDIS_URL.to_string());
    let pool_qtd: usize = (env::var("NUM_CONSUMER")
        .unwrap_or_else(|_| constantes::NUM_CONSUMER.to_string()))
    .parse()
    .unwrap();

    let max_tentativas = 5u8;
    let delay_secs = 5u64;

    for tentativa in 1..=max_tentativas {
        let mut cfg = Config::from_url(&db_url);
        cfg.pool = Some(PoolConfig::new(pool_qtd * 2));

        info!(
            "Tentando criar pool de conexões Redis (tentativa {}/{})...",
            tentativa, max_tentativas
        );

        match cfg.create_pool(Some(Runtime::Tokio1)) {
            Ok(pool) => {
                info!("✅ Pool de conexões Redis criado com sucesso!");
                return pool; // Renomeado de 'cliente' para 'pool' para clareza
            }

            Err(e) => {
                warn!("Falha ao criar pool de conexões Redis: {}", e); // CORREÇÃO: Mensagem de log

                if tentativa < max_tentativas {
                    warn!("Tentando novamente em {} segundos...", delay_secs);
                    tokio::time::sleep(Duration::from_secs(delay_secs)).await;
                }
            }
        }
    }
    panic!(
        "❌ Não foi possível criar o pool de conexões Redis após {} tentativas.",
        max_tentativas
    );
}
pub async fn salvar_pagamento(pagamento: models::payment::Payment, state: &AppState) {
    let max_tentativas = 50u8;
    let retry_delay = tokio::time::Duration::from_millis(1);

    let pagamento_chave = format!("payment:{}", &pagamento.correlation_id);
    let pagamento_tempo = pagamento.requested_at.unwrap().timestamp_micros() as u64;
    let pagamento_json =
        match tokio::task::spawn_blocking(move || serde_json::to_string(&pagamento)).await {
            Ok(Ok(json)) => json,
            Ok(Err(e)) => {
                tracing::error!("Falha ao serializar pagamento: {}", e);
                return;
            }
            Err(e) => {
                tracing::error!("Task de serialização falhou (panic): {}", e);
                return;
            }
        };
    for tentativa in 1..=max_tentativas {
        let mut conn = match state.redis_pool.get().await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    "[Redis Save] Falha ao pegar conexão (tentativa {}/{}): {}. Tentando novamente...",
                    tentativa,
                    max_tentativas,
                    e
                );
                tokio::time::sleep(retry_delay).await;
                continue;
            }
        };

        let sorted_set_key = "payments_by_date";

        let result: Result<(), redis::RedisError> = redis::pipe()
            .atomic()
            .set(&pagamento_chave, &pagamento_json)
            .expire(&pagamento_chave, 80)
            .zadd(sorted_set_key, &pagamento_chave, &pagamento_tempo)
            .query_async(&mut conn)
            .await;

        match result {
            Ok(_) => {
                return;
            }
            Err(e) => {
                tracing::warn!(
                    "[Redis Save] Falha ao executar pipeline (tentativa {}/{}): {}. Tentando novamente...",
                    tentativa,
                    max_tentativas,
                    e
                );
                tokio::time::sleep(retry_delay).await;
            }
        }
    }
}

pub async fn coletar_entre_timestamp(
    state: &AppState,
    from: u64,
    to: u64,
) -> Result<(u64, String, u64, String), RedisError> {
    // Pega uma conexão do pool de forma segura, propagando o erro se falhar.
    let mut conn = state.redis_pool.get().await.unwrap();

    let sorted_set_key = "payments_by_date";

    let script = Script::new(
        r#"
        local keys = redis.call('ZRANGEBYSCORE', KEYS[1], ARGV[1], ARGV[2])
        if #keys == 0 then
            return {0, 0, 0, 0}
        end

        local default_reqs = 0
        local default_amt = 0.0
        local fallback_reqs = 0
        local fallback_amt = 0.0
        
        -- Processa as chaves em lotes de 5000 para evitar limites do Lua
        local chunk_size = 5000
        for i = 1, #keys, chunk_size do
            -- Cria uma subtabela (chunk) para a chamada MGET
            local chunk_keys = {}
            for j = i, math.min(i + chunk_size - 1, #keys) do
                table.insert(chunk_keys, keys[j])
            end

            -- Busca os valores para o lote atual de chaves
            local values = redis.call('MGET', unpack(chunk_keys))

            for _, json_str in ipairs(values) do
                if json_str then
                    -- cjson é o parser de JSON embutido no Redis
                    local data = cjson.decode(json_str)
                    if data.tipo == 'Default' then
                        default_reqs = default_reqs + 1
                        default_amt = default_amt + data.amount
                    elseif data.tipo == 'Fallback' then
                        fallback_reqs = fallback_reqs + 1
                        fallback_amt = fallback_amt + data.amount
                    end
                end
            end
        end

        return {
            default_reqs,
            string.format('%.4f', default_amt),
            fallback_reqs,
            string.format('%.4f', fallback_amt)
        }
    "#,
    );

    let summary_data: (u64, String, u64, String) = script
        .key(sorted_set_key)
        .arg(from)
        .arg(to)
        .invoke_async(&mut conn)
        .await?;

    Ok(summary_data)
}

pub async fn expurgar_todos_pagamentos(state: &AppState) -> Result<(), RedisError> {
    let mut conn = state.redis_pool.get().await.unwrap();
    let () = redis::cmd("FLUSHDB").query_async(&mut conn).await?;
    Ok(())
}
