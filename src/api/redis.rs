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
        .expire(&pagamento_chave, 360)
        .zadd(sorted_set_key, &pagamento_chave, pagamento_tempo)
        .query_async(&mut conn)
        .await
        .unwrap();
}

pub async fn coletar_entre_timestamp(
    state: &AppState,
    from: u64, // Alterado para u64 para consistência com timestamps
    to: u64,
) -> Result<Vec<models::payment::Payment>, RedisError> {
    // Pega uma conexão do pool de forma segura, propagando o erro se falhar.
    let mut conn = state.redis_pool.get().await.unwrap();

    let sorted_set_key = "payments_by_date";

    let keys: Vec<String> = conn.zrangebyscore(sorted_set_key, from, to).await?;

    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let pagamentos_valores: Vec<Option<String>> = conn.mget(keys).await?;

    let pagamentos = pagamentos_valores
        .into_iter()
        .filter_map(|opt_json| opt_json)
        .map(|json_str| serde_json::from_str(&json_str))
        .collect::<Result<Vec<models::payment::Payment>, serde_json::Error>>()?;

    Ok(pagamentos)
}

pub async fn expurgar_todos_pagamentos(state: &AppState) -> Result<(), RedisError> {
    let mut conn = state.redis_pool.get().await.unwrap();
    let () = redis::cmd("FLUSHDB").query_async(&mut conn).await?;
    Ok(())
}
