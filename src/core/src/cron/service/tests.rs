use super::*;
use crate::storage::SqliteStore;
use crate::storage::{CronRunRecord, CronRunStatus};
use serde_json::json;

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
    assert!(!runs.result.unwrap()["runs"].as_array().unwrap().is_empty());

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
