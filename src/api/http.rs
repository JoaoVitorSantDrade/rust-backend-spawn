use std::time::Duration;

pub fn cria_cliente_http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(1))
        .build()
        .unwrap()
}
