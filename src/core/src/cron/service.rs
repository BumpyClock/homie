use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};

use homie_protocol::{error_codes, BinaryFrame, Response};

use crate::cron::scheduler::schedule_next_after;
use crate::cron::CronRunner;
use crate::router::{ReapEvent, ServiceHandler};
use crate::storage::{CronRecord, CronStatus, Store};

use super::models::{
    clamp_limit, CronAddParams, CronIdParams, CronListParams, CronRunsParams, CronUpdateParams,
};

#[derive(Debug, Deserialize)]
struct CronStatusParams {
    cron_id: String,
}

/// Cron service backed by persistent store.
pub struct CronService {
    store: Arc<dyn Store>,
    runner: Arc<CronRunner>,
}

impl CronService {
    pub fn new(store: Arc<dyn Store>, runner: Arc<CronRunner>) -> Self {
        Self { store, runner }
    }

    fn add(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronAddParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        if params.name.trim().is_empty() {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "missing name");
        }
        if params.command.trim().is_empty() {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "missing command");
        }
        if params.schedule.trim().is_empty() {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "missing schedule");
        }
        let now = now_unix();
        let next_run_at = match schedule_next_after(&params.schedule, now) {
            Ok(next) => Some(next),
            Err(err) => return Response::error(req_id, error_codes::INVALID_PARAMS, err),
        };

        let cron = CronRecord {
            cron_id: uuid::Uuid::new_v4().to_string(),
            name: params.name,
            schedule: params.schedule,
            command: params.command,
            status: params.status.unwrap_or(CronStatus::Active),
            skip_overlap: params.skip_overlap.unwrap_or(true),
            created_at: now,
            updated_at: now,
            last_run_at: None,
            next_run_at,
        };

        if let Err(e) = self.store.upsert_cron(&cron) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }
        Response::success(req_id, json!({ "cron": cron }))
    }

    fn start(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        self.add(req_id, params)
    }

    fn list(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronListParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => CronListParams {
                status: None,
                limit: None,
            },
        };

        let mut crons = match self.store.list_crons() {
            Ok(crons) => crons,
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        if let Some(status) = params.status {
            crons.retain(|cron| cron.status == status);
        }

        let limit = clamp_limit(params.limit, 100, 1000);
        crons.truncate(limit);
        Response::success(req_id, json!({ "crons": crons }))
    }

    fn update(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronUpdateParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        let mut cron = match self.store.get_cron(&params.cron_id) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown cron")
            }
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        if let Some(name) = params.name {
            cron.name = name;
        }
        if let Some(schedule) = params.schedule {
            let now = now_unix();
            if let Err(err) = schedule_next_after(&schedule, now) {
                return Response::error(
                    req_id,
                    error_codes::INVALID_PARAMS,
                    format!("invalid schedule: {err}"),
                );
            }
            cron.schedule = schedule;
            cron.next_run_at = Some(schedule_next_after(&cron.schedule, now).unwrap_or(now));
        }
        if let Some(command) = params.command {
            cron.command = command;
        }
        if let Some(status) = params.status {
            cron.status = status;
        }
        if let Some(skip_overlap) = params.skip_overlap {
            cron.skip_overlap = skip_overlap;
        }
        cron.updated_at = now_unix();

        if let Err(e) = self.store.upsert_cron(&cron) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }
        Response::success(req_id, json!({ "cron": cron }))
    }

    fn remove(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronIdParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        match self.store.get_cron(&params.cron_id) {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown cron")
            }
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        if let Err(e) = self.store.delete_cron(&params.cron_id) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }
        Response::success(
            req_id,
            json!({ "cron_id": params.cron_id, "removed": true }),
        )
    }

    fn cancel(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronIdParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        let mut cron = match self.store.get_cron(&params.cron_id) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown cron")
            }
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        cron.status = CronStatus::Paused;
        cron.updated_at = now_unix();

        if let Err(e) = self.store.upsert_cron(&cron) {
            return Response::error(req_id, error_codes::INTERNAL_ERROR, e);
        }

        Response::success(req_id, json!({ "cron": cron }))
    }

    async fn run_now(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronIdParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        match self.runner.run_cron_now(&params.cron_id).await {
            Ok(run) => Response::success(req_id, json!({ "run": run })),
            Err(err) => Response::error(req_id, error_codes::INVALID_PARAMS, err),
        }
    }

    async fn run_force(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronIdParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        match self.runner.run_cron_now_force(&params.cron_id, true).await {
            Ok(run) => Response::success(req_id, json!({ "run": run })),
            Err(err) => Response::error(req_id, error_codes::INVALID_PARAMS, err),
        }
    }

    fn status(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronStatusParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        let cron = match self.store.get_cron(&params.cron_id) {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown cron")
            }
            Err(err) => return Response::error(req_id, error_codes::INTERNAL_ERROR, err),
        };

        let last_run = match self.store.get_cron_last_run(&params.cron_id) {
            Ok(last) => last,
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        Response::success(req_id, json!({ "cron": cron, "last_run": last_run }))
    }

    fn runs(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronRunsParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };

        if let Ok(None) = self.store.get_cron(&params.cron_id) {
            return Response::error(req_id, error_codes::INVALID_PARAMS, "unknown cron");
        }

        let limit = clamp_limit(params.limit, 100, 500);
        match self.store.list_cron_runs(&params.cron_id, limit) {
            Ok(runs) => Response::success(req_id, json!({ "runs": runs })),
            Err(e) => Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        }
    }

    fn logs_tail(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronRunsParams = match params {
            Some(v) => match serde_json::from_value(v) {
                Ok(p) => p,
                Err(e) => {
                    return Response::error(
                        req_id,
                        error_codes::INVALID_PARAMS,
                        format!("invalid params: {e}"),
                    );
                }
            },
            None => return Response::error(req_id, error_codes::INVALID_PARAMS, "missing params"),
        };
        let limit = clamp_limit(params.limit, 20, 100);
        let runs = match self.store.list_cron_runs(&params.cron_id, limit) {
            Ok(runs) => runs,
            Err(e) => return Response::error(req_id, error_codes::INTERNAL_ERROR, e),
        };

        let lines: Vec<String> = runs
            .iter()
            .map(|run| match &run.error {
                Some(err) => format!("{} {} {}", run.run_id, run.status.as_str(), err),
                None => format!("{} {}", run.run_id, run.status.as_str()),
            })
            .collect();
        Response::success(req_id, json!({ "runs": lines, "items": lines.len() }))
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

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{CronRunRecord, CronRunStatus};
    use crate::storage::SqliteStore;

    fn make_store() -> Arc<SqliteStore> {
        Arc::new(SqliteStore::open_memory().unwrap())
    }

    fn make_runner(store: Arc<SqliteStore>) -> Arc<CronRunner> {
        let (tx, _rx) = tokio::sync::broadcast::channel(16);
        Arc::new(CronRunner::new(store, 1, tx))
    }

    #[tokio::test]
    async fn cron_add_and_list_are_scoped_by_status() {
        let store = make_store();
        let runner = make_runner(store.clone());
        let mut svc = CronService::new(store, runner);

        let add_active = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.add",
                Some(json!({
                    "name": "active",
                    "schedule": "* * * * * *",
                    "command": "echo active",
                    "status": "active"
                })),
            )
            .await;
        assert!(add_active.result.is_some());

        let add_paused = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.start",
                Some(json!({
                    "name": "paused",
                    "schedule": "* * * * * *",
                    "command": "echo paused",
                    "status": "paused"
                })),
            )
            .await;
        assert!(add_paused.result.is_some());

        let active = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.list",
                Some(json!({ "status": "active" })),
            )
            .await;
        let active_result = active.result.unwrap();
        let active_crons = active_result["crons"].as_array().unwrap();
        assert_eq!(active_crons.len(), 1);
        assert_eq!(active_crons[0]["status"], "active");
    }

    #[tokio::test]
    async fn cron_update_remove_status_and_runs() {
        let store = make_store();
        let runner = make_runner(store.clone());
        let mut svc = CronService::new(store.clone(), runner);

        let created = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.add",
                Some(json!({
                    "name": "heartbeat",
                    "schedule": "* * * * * *",
                    "command": "echo once",
                })),
            )
            .await;
        let created_result = created.result.unwrap();
        let cron_id = created_result["cron"]["cron_id"].as_str().unwrap();

        let updated = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.update",
                Some(json!({
                    "cron_id": cron_id,
                    "name": "heartbeat-updated",
                    "skip_overlap": false
                })),
            )
            .await;
        assert_eq!(updated.result.unwrap()["cron"]["name"], "heartbeat-updated");

        let status = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.status",
                Some(json!({"cron_id": cron_id})),
            )
            .await;
        assert_eq!(status.result.unwrap()["cron"]["name"], "heartbeat-updated");

        let run = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.run",
                Some(json!({"cron_id": cron_id})),
            )
            .await;
        assert!(run.result.unwrap()["run"]["run_id"].is_string());

        let runs = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.runs",
                Some(json!({"cron_id": cron_id, "limit": 5})),
            )
            .await;
        assert!(runs.result.unwrap()["runs"].as_array().unwrap().len() >= 1);

        let removed = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.remove",
                Some(json!({"cron_id": cron_id})),
            )
            .await;
        assert_eq!(removed.result.unwrap()["removed"], true);
    }

    #[tokio::test]
    async fn cron_run_skips_when_overlap_is_set() {
        let store = make_store();
        let runner = make_runner(store.clone());
        let mut svc = CronService::new(store.clone(), runner);

        let created = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.add",
                Some(json!({
                    "name": "overlap",
                    "schedule": "* * * * * *",
                    "command": "echo overlap",
                    "skip_overlap": true
                })),
            )
            .await;
        let created_payload = created.result.unwrap();
        let cron_id = created_payload["cron"]["cron_id"]
            .as_str()
            .unwrap()
            .to_string();

        store
            .upsert_cron_run(&CronRunRecord {
                run_id: "run-inflight".into(),
                cron_id: cron_id.clone(),
                scheduled_at: 1,
                started_at: Some(1),
                finished_at: None,
                status: CronRunStatus::Running,
                exit_code: None,
                output: None,
                error: None,
            })
            .unwrap();

        let run = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.run",
                Some(json!({"cron_id": cron_id})),
            )
            .await;
        assert_eq!(run.result.unwrap()["run"]["status"], "skipped");
    }

    #[tokio::test]
    async fn cron_run_force_bypasses_overlap() {
        let store = make_store();
        let runner = make_runner(store.clone());
        let mut svc = CronService::new(store.clone(), runner);

        let created = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.add",
                Some(json!({
                    "name": "force",
                    "schedule": "* * * * * *",
                    "command": "echo force",
                    "skip_overlap": true
                })),
            )
            .await;
        let created_payload = created.result.unwrap();
        let cron_id = created_payload["cron"]["cron_id"]
            .as_str()
            .unwrap()
            .to_string();

        store
            .upsert_cron_run(&CronRunRecord {
                run_id: "run-inflight".into(),
                cron_id: cron_id.clone(),
                scheduled_at: 1,
                started_at: Some(1),
                finished_at: None,
                status: CronRunStatus::Running,
                exit_code: None,
                output: None,
                error: None,
            })
            .unwrap();

        let run = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.run.force",
                Some(json!({"cron_id": cron_id})),
            )
            .await;
        assert_ne!(run.result.unwrap()["run"]["status"], "skipped");
    }

    #[tokio::test]
    async fn cron_status_and_runs_returns_latest_and_limited() {
        let store = make_store();
        let runner = make_runner(store.clone());
        let mut svc = CronService::new(store.clone(), runner);

        let created = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.add",
                Some(json!({
                    "name": "history",
                    "schedule": "* * * * * *",
                    "command": "echo history"
                })),
            )
            .await;
        let created_payload = created.result.unwrap();
        let cron_id = created_payload["cron"]["cron_id"]
            .as_str()
            .unwrap()
            .to_string();

        for i in 0..3 {
            store
                .upsert_cron_run(&CronRunRecord {
                    run_id: format!("run-{i}"),
                    cron_id: cron_id.clone(),
                    scheduled_at: 1 + i as u64,
                    started_at: Some(1 + i as u64),
                    finished_at: Some(2 + i as u64),
                    status: CronRunStatus::Succeeded,
                    exit_code: Some(0),
                    output: None,
                    error: None,
                })
                .unwrap();
        }

        let status = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.status",
                Some(json!({"cron_id": cron_id})),
            )
            .await;
        let status_payload = status.result.unwrap();
        assert_eq!(status_payload["last_run"]["run_id"], "run-2");

        let limited = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.runs",
                Some(json!({
                    "cron_id": cron_id,
                    "limit": 2
                })),
            )
            .await;
        assert_eq!(limited.result.unwrap()["runs"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn cron_start_and_cancel_are_compatibility_aliases() {
        let store = make_store();
        let runner = make_runner(store.clone());
        let mut svc = CronService::new(store.clone(), runner);

        let created = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.start",
                Some(json!({
                    "name": "compat",
                    "schedule": "* * * * * *",
                    "command": "echo compat",
                })),
            )
            .await;
        assert!(created.result.is_some());
        let created_result = created.result.unwrap();
        let cron_id = created_result["cron"]["cron_id"].as_str().unwrap();

        let cancelled = svc
            .handle_request(
                uuid::Uuid::new_v4(),
                "cron.cancel",
                Some(json!({"cron_id": cron_id})),
            )
            .await;
        assert_eq!(cancelled.result.unwrap()["cron"]["status"], "paused");
    }
}
