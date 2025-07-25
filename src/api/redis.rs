use deadpool_redis::{
    Config, Manager, Runtime,
    redis::{RedisError, Script},
};
use redis::AsyncCommands;
use std::env;

use crate::{appstate::AppState, constantes, models};

pub async fn estabelecer_conexao() -> Result<redis::Client, RedisError> {
    let db_url = env::var("DB_URL").unwrap_or_else(|_| constantes::REDIS_URL.to_string());

    match redis::Client::open(db_url) {
        Ok(conn) => Ok(conn),
        Err(e) => {
            tracing::error!("Erro ao estabelecer conexão com o Redis");
            Err(e)
        }
    }
}

pub async fn estabelecer_pool_conexao()
-> deadpool::managed::Pool<Manager, deadpool_redis::Connection> {
    let db_url = env::var("DB_URL").unwrap_or_else(|_| constantes::REDIS_URL.to_string());
    let cfg = Config::from_url(db_url);
    cfg.create_pool(Some(Runtime::Tokio1)).unwrap()
}

pub async fn salvar_pagamento(pagamento: &models::payment::Payment, state: &AppState) {
    let mut conn = state.redis_pool.get().await.unwrap();
    let pagamento_json = serde_json::to_string(pagamento).unwrap();
    let pagamento_chave = format!("payment:{}", pagamento.correlation_id);
    let pagamento_tempo = pagamento.requested_at.unwrap().timestamp();
    let sorted_set_key = "payments_by_date";
    let () = redis::pipe()
        .atomic()
        .set(&pagamento_chave, &pagamento_json)
        .expire(&pagamento_chave, 65)
        .zadd(sorted_set_key, &pagamento_chave, pagamento_tempo)
        .query_async(&mut conn)
        .await
        .unwrap();
}

pub async fn coletar_entre_timestamp(
    state: &AppState,
    from: u64, // Alterado para u64 para consistência com timestamps
    to: u64,
) -> Result<Vec<f64>, RedisError> {
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
        
        -- Processa as chaves em lotes de 1000 para evitar limites do Lua
        local chunk_size = 1000
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

        return {default_reqs, default_amt, fallback_reqs, fallback_amt}
    "#,
    );

    // Invoca o script e espera um vetor de números como resposta.
    let summary_data: Vec<f64> = script
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
