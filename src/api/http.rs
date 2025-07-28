use std::time::Duration;

pub fn cria_cliente_http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(2))
        .tcp_nodelay(true)
        .pool_max_idle_per_host(500)
        .build()
        .unwrap()
}
