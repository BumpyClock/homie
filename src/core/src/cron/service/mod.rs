use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use homie_protocol::{error_codes, BinaryFrame, Response};

use crate::cron::CronRunner;
use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::Store;

mod common;
mod execution;
mod lifecycle;
mod query;
#[cfg(test)]
mod tests;

#[derive(Debug, Deserialize)]
pub(super) struct CronStatusParams {
    pub(super) cron_id: String,
}

/// Cron service backed by persistent store.
pub struct CronService {
    pub(super) store: Arc<dyn Store>,
    pub(super) runner: Arc<CronRunner>,
}

impl CronService {
    pub fn new(store: Arc<dyn Store>, runner: Arc<CronRunner>) -> Self {
        Self { store, runner }
    }
}

impl ServiceHandler for CronService {
    fn namespace(&self) -> &str {
        "cron"
    }

    fn handle_request(
        &mut self,
        id: uuid::Uuid,
        method: &str,
        params: Option<Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let method = method.to_string();
        Box::pin(async move {
            match method.as_str() {
                "cron.add" => self.add(id, params),
                "cron.start" => self.start(id, params),
                "cron.list" => self.list(id, params),
                "cron.update" => self.update(id, params),
                "cron.remove" => self.remove(id, params),
                "cron.cancel" => self.cancel(id, params),
                "cron.run" => self.run_now(id, params).await,
                "cron.run.force" => self.run_force(id, params).await,
                "cron.status" => self.status(id, params),
                "cron.runs" => self.runs(id, params),
                "cron.logs.tail" => self.logs_tail(id, params),
                _ => Response::error(
                    id,
                    error_codes::METHOD_NOT_FOUND,
                    format!("unknown method: {method}"),
                ),
            }
        })
    }

    fn handle_binary(&mut self, _frame: &BinaryFrame) {}

    fn reap(&mut self) -> Vec<ReapEvent> {
        Vec::new()
    }

    fn shutdown(&mut self) {}
}
