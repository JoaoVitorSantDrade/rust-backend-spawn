use std::time::Duration;

pub fn cria_cliente_http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(4))
        .build()
        .unwrap()
}
