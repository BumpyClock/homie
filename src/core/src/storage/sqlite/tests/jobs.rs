use super::*;

#[test]
fn upsert_and_get_job() {
    let store = make_store();
    let now = now_unix();
    let job = JobRecord {
        job_id: "job-1".into(),
        name: "build".into(),
        status: JobStatus::Queued,
        created_at: now,
        updated_at: now,
        spec: serde_json::json!({"kind": "command", "command": "echo"}),
        logs: vec!["line1".into(), "line2".into()],
    };
    store.upsert_job(&job).unwrap();

    let loaded = store.get_job("job-1").unwrap().unwrap();
    assert_eq!(loaded.name, "build");
    assert_eq!(loaded.status, JobStatus::Queued);
    assert_eq!(loaded.logs.len(), 2);
}

#[test]
fn prune_jobs_removes_old_and_excess() {
    let store = make_store();
    let now = now_unix();
    let old = JobRecord {
        job_id: "old".into(),
        name: "old".into(),
        status: JobStatus::Succeeded,
        created_at: now.saturating_sub(900_000),
        updated_at: now,
        spec: serde_json::json!({}),
        logs: vec![],
    };
    let recent = JobRecord {
        job_id: "new".into(),
        name: "new".into(),
        status: JobStatus::Queued,
        created_at: now,
        updated_at: now,
        spec: serde_json::json!({}),
        logs: vec![],
    };
    store.upsert_job(&old).unwrap();
    store.upsert_job(&recent).unwrap();

    store.prune_jobs(1, 1).unwrap();
    let jobs = store.list_jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_id, "new");
}
