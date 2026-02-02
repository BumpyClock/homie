use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use homie_protocol::{error_codes, BinaryFrame, Response};

use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::{JobRecord, JobStatus, Store};

#[derive(Debug, Deserialize)]
struct StartParams {
    name: String,
    spec: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct JobIdParams {
    job_id: String,
}

#[derive(Debug, Deserialize)]
struct TailParams {
    job_id: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

/// Jobs service backed by the persistent store.
pub struct JobsService {
    store: Arc<dyn Store>,
}

impl JobsService {
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    fn start(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: StartParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    )
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        let now = now_unix();
        let record = JobRecord {
            job_id: Uuid::new_v4().to_string(),
            name: params.name,
            status: JobStatus::Queued,
            created_at: now,
            updated_at: now,
            spec: params.spec.unwrap_or_else(|| json!({})),
            logs: Vec::new(),
        };

        if let Err(e) = self.store.upsert_job(&record) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        Response::success(req_id, json!({ "job": record }))
    }

    fn status(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: JobIdParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    )
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        match self.store.get_job(&params.job_id) {
            Ok(Some(job)) => Response::success(req_id, json!({ "job": job })),
            Ok(None) => Response::error(req_id, error_codes::INVALID_PARAMS, "unknown job"),
            Err(e) => Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        }
    }

    fn cancel(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: JobIdParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    )
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        let job = match self.store.get_job(&params.job_id) {
            Ok(Some(job)) => job,
            Ok(None) => return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown job"),
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        let updated = JobRecord {
            status: JobStatus::Cancelled,
            updated_at: now_unix(),
            ..job
        };

        if let Err(e) = self.store.upsert_job(&updated) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        Response::success(req_id, json!({ "job": updated }))
    }

    fn logs_tail(&mut self, req_id: Uuid, params: Option<Value>) -> Response {
        let params: TailParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    )
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        let job = match self.store.get_job(&params.job_id) {
            Ok(Some(job)) => job,
            Ok(None) => return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown job"),
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(200);
        let total = job.logs.len();
        let start = offset.min(total);
        let end = (start + limit).min(total);
        let lines = job.logs[start..end].to_vec();
        Response::success(req_id, json!({ "lines": lines, "next_offset": end }))
    }
}

impl ServiceHandler for JobsService {
    fn namespace(&self) -> &str {
        "jobs"
    }

    fn handle_request(
        &mut self,
        id: Uuid,
        method: &str,
        params: Option<Value>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send + '_>> {
        let resp = match method {
            "jobs.start" => self.start(id, params),
            "jobs.status" => self.status(id, params),
            "jobs.cancel" => self.cancel(id, params),
            "jobs.logs.tail" => self.logs_tail(id, params),
            _ => Response::error(
                id,
                error_codes::METHOD_NOT_FOUND,
                format!("unknown method: {method}"),
            ),
        };
        Box::pin(async move { resp })
    }

    fn handle_binary(&mut self, _frame: &BinaryFrame) {}

    fn reap(&mut self) -> Vec<ReapEvent> {
        Vec::new()
    }

    fn shutdown(&mut self) {}
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
