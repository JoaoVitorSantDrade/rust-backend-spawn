use std::time::Duration;

pub fn cria_cliente_http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .connect_timeout(Duration::from_millis(500))
        .tcp_nodelay(true)
        .pool_max_idle_per_host(100)
        .build()
        .unwrap()
}
