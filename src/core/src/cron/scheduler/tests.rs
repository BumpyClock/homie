use std::sync::Arc;

use super::*;
use crate::storage::SqliteStore;
use crate::storage::{CronRecord, CronRunRecord, CronRunStatus, CronStatus};

fn now() -> u64 {
    1_700_000_000
}

#[test]
fn due_runs_returns_all_due_and_honors_max() {
    let runs = due_runs("* * * * * *", 1_700_000_000, 1_700_000_005, 3).unwrap();
    assert_eq!(runs, vec![1_700_000_000, 1_700_000_001, 1_700_000_002]);

    let skipped = due_runs("* * * * * *", 1_700_000_000, 1_699_999_999, 3).unwrap();
    assert!(skipped.is_empty());
}

#[tokio::test]
async fn manual_run_skips_when_running_with_overlap_protection() {
    let store = Arc::new(SqliteStore::open_memory().unwrap());
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let runner = Arc::new(CronRunner::new(store.clone(), 1, tx));
    runner
        .store
        .upsert_cron(&CronRecord {
            cron_id: "cron-overlap".into(),
            name: "overlap".into(),
            schedule: "* * * * * *".into(),
            command: "echo hi".into(),
            status: CronStatus::Active,
            skip_overlap: true,
            created_at: now(),
            updated_at: now(),
            last_run_at: None,
            next_run_at: Some(now()),
        })
        .unwrap();

    runner
        .store
        .upsert_cron_run(&CronRunRecord {
            run_id: "run-1".into(),
            cron_id: "cron-overlap".into(),
            scheduled_at: now(),
            started_at: Some(now()),
            finished_at: None,
            status: CronRunStatus::Running,
            exit_code: None,
            output: None,
            error: None,
        })
        .unwrap();

    let run = runner.run_cron_now("cron-overlap").await.unwrap();
    assert_eq!(run.status, CronRunStatus::Skipped);
}

#[tokio::test]
async fn run_now_skips_when_global_limit_is_zero() {
    let store = Arc::new(SqliteStore::open_memory().unwrap());
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let runner = Arc::new(CronRunner::new(store.clone(), 0, tx));
    let now = now();

    runner
        .store
        .upsert_cron(&CronRecord {
            cron_id: "cron-no-cap".into(),
            name: "no-cap".into(),
            schedule: "* * * * * *".into(),
            command: "echo hi".into(),
            status: CronStatus::Active,
            skip_overlap: false,
            created_at: now,
            updated_at: now,
            last_run_at: None,
            next_run_at: Some(now),
        })
        .unwrap();

    let run = runner.run_cron_now("cron-no-cap").await.unwrap();
    assert_eq!(run.status, CronRunStatus::Skipped);
}

fn make_runner() -> Arc<CronRunner> {
    let store = Arc::new(SqliteStore::open_memory().unwrap());
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    Arc::new(CronRunner::new(store, 3, tx))
}

#[tokio::test]
async fn process_due_runs_skips_missed_windows_when_overlap_is_set() {
    let runner = make_runner();
    let cron = CronRecord {
        cron_id: "cron-missed-overlap".into(),
        name: "missed-overlap".into(),
        schedule: "* * * * * *".into(),
        command: "echo overlap".into(),
        status: CronStatus::Active,
        skip_overlap: true,
        created_at: now(),
        updated_at: now(),
        next_run_at: Some(now() - 5),
        last_run_at: None,
    };
    runner.store.upsert_cron(&cron).unwrap();
    runner
        .store
        .upsert_cron_run(&CronRunRecord {
            run_id: "cron-running".into(),
            cron_id: cron.cron_id.clone(),
            scheduled_at: now() - 10,
            started_at: Some(now() - 10),
            finished_at: None,
            status: CronRunStatus::Running,
            exit_code: None,
            output: None,
            error: None,
        })
        .unwrap();

    let due = vec![now() - 3, now() - 2, now() - 1];
    super::schedule::process_due_runs(runner.clone(), cron.clone(), now() + 1, due.clone())
        .await
        .unwrap();

    let runs = runner.store.list_cron_runs(&cron.cron_id, 10).unwrap();
    let skipped = runs
        .into_iter()
        .filter(|run| run.status == CronRunStatus::Skipped)
        .count();
    assert_eq!(skipped, due.len());

    let latest = runner.store.get_cron(&cron.cron_id).unwrap().unwrap();
    assert_eq!(latest.last_run_at, Some(*due.last().unwrap()));
    assert_eq!(
        latest.next_run_at,
        Some(schedule_next_after(&cron.schedule, *due.last().unwrap()).unwrap())
    );
}

#[tokio::test]
async fn process_due_runs_runs_each_window_when_overlap_not_set() {
    let runner = make_runner();
    let cron = CronRecord {
        cron_id: "cron-missed-non-overlap".into(),
        name: "missed-non-overlap".into(),
        schedule: "* * * * * *".into(),
        command: "echo none".into(),
        status: CronStatus::Active,
        skip_overlap: false,
        created_at: now(),
        updated_at: now(),
        next_run_at: Some(now() - 5),
        last_run_at: None,
    };
    runner.store.upsert_cron(&cron).unwrap();

    let due = vec![now() - 3, now() - 2, now() - 1];
    super::schedule::process_due_runs(runner.clone(), cron.clone(), now() + 1, due.clone())
        .await
        .unwrap();

    let runs = runner.store.list_cron_runs(&cron.cron_id, 10).unwrap();
    assert_eq!(runs.len(), due.len());

    let latest = runner.store.get_cron(&cron.cron_id).unwrap().unwrap();
    assert_eq!(latest.last_run_at, Some(*due.last().unwrap()));
}
