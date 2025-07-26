use std::sync::{Arc, atomic::AtomicUsize};

use axum::body::Bytes;
use deadpool::managed::Pool;
use deadpool_redis::{Connection, Manager};
use reqwest::Client;
use tokio::sync::{RwLock, mpsc};

use crate::models::processor::Processor;

#[derive(Clone)]
pub struct AppState {
    pub processors: Vec<Arc<RwLock<Processor>>>,
    pub http_client: Client,
    pub redis_pool: Pool<Manager, Connection>,
    pub nats_client: async_nats::Client,
    pub sender_queue: Arc<Vec<mpsc::UnboundedSender<Bytes>>>,
    pub round_robin_counter: Arc<AtomicUsize>,
}
