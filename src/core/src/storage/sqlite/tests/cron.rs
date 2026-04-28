use super::*;
use crate::storage::types::{CronRecord, CronRunRecord, CronRunStatus, CronStatus};

#[test]
fn cron_upsert_and_get() {
    let store = make_store();
    let now = now_unix();
    let cron = CronRecord {
        cron_id: "cron-1".into(),
        name: "heartbeat".into(),
        schedule: "* * * * * *".into(),
        command: "echo hi".into(),
        status: CronStatus::Active,
        skip_overlap: true,
        created_at: now,
        updated_at: now,
        next_run_at: Some(now + 60),
        last_run_at: None,
    };
    store.upsert_cron(&cron).unwrap();

    let loaded = store.get_cron("cron-1").unwrap().unwrap();
    assert_eq!(loaded.name, "heartbeat");
    assert_eq!(loaded.status, CronStatus::Active);
    assert!(loaded.skip_overlap);
    assert_eq!(loaded.next_run_at, Some(now + 60));
}

#[test]
fn cron_list_and_update_ordered_and_remove() {
    let store = make_store();
    let now = now_unix();
    let a = CronRecord {
        cron_id: "a".into(),
        name: "every-min".into(),
        schedule: "* * * * * *".into(),
        command: "echo a".into(),
        status: CronStatus::Active,
        skip_overlap: true,
        created_at: now,
        updated_at: now,
        next_run_at: Some(now + 60),
        last_run_at: None,
    };
    let b = CronRecord {
        cron_id: "b".into(),
        name: "every-sec".into(),
        schedule: "* * * * * *".into(),
        command: "echo b".into(),
        status: CronStatus::Paused,
        skip_overlap: true,
        created_at: now + 1,
        updated_at: now + 1,
        next_run_at: Some(now + 30),
        last_run_at: None,
    };
    store.upsert_cron(&a).unwrap();
    store.upsert_cron(&b).unwrap();

    let items = store.list_crons().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].cron_id, "b");

    let mut updated = a;
    updated.name = "updated".into();
    updated.status = CronStatus::Paused;
    store.upsert_cron(&updated).unwrap();
    let reloaded = store.get_cron("a").unwrap().unwrap();
    assert_eq!(reloaded.name, "updated");
    assert_eq!(reloaded.status, CronStatus::Paused);

    store.delete_cron("a").unwrap();
    assert!(store.get_cron("a").unwrap().is_none());
}

#[test]
fn cron_runs_store_and_retrieve() {
    let store = make_store();
    let now = now_unix();
    let run = CronRunRecord {
        run_id: "run-1".into(),
        cron_id: "cron-1".into(),
        scheduled_at: now,
        started_at: Some(now),
        finished_at: Some(now + 1),
        status: CronRunStatus::Succeeded,
        exit_code: Some(0),
        output: Some("ok".into()),
        error: None,
    };
    store.upsert_cron_run(&run).unwrap();
    store
        .upsert_cron_run(&CronRunRecord {
            run_id: "run-2".into(),
            cron_id: "cron-1".into(),
            scheduled_at: now + 10,
            started_at: Some(now + 10),
            finished_at: Some(now + 11),
            status: CronRunStatus::Failed,
            exit_code: Some(1),
            output: Some("oops".into()),
            error: Some("boom".into()),
        })
        .unwrap();

    let latest = store.get_cron_last_run("cron-1").unwrap().unwrap();
    assert_eq!(latest.run_id, "run-2");

    let items = store.list_cron_runs("cron-1", 10).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].status, CronRunStatus::Failed);
    assert!(!store.cron_has_running("cron-1").unwrap());
    assert_eq!(items[0].run_id, "run-2");
}

#[test]
fn cron_runs_pruning_keeps_recent_per_cron() {
    let store = make_store();
    let now = now_unix();
    for i in 0..6 {
        store
            .upsert_cron_run(&CronRunRecord {
                run_id: format!("run-{i}"),
                cron_id: "cron-1".into(),
                scheduled_at: now + (i as u64),
                started_at: Some(now + (i as u64)),
                finished_at: Some(now + (i as u64) + 1),
                status: CronRunStatus::Succeeded,
                exit_code: Some(0),
                output: None,
                error: None,
            })
            .unwrap();
    }

    store.prune_cron_runs(0, 3).unwrap();
    let runs = store.list_cron_runs("cron-1", 10).unwrap();
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].run_id, "run-5");
}

#[test]
fn cron_has_running_detects_running_runs() {
    let store = make_store();
    let cron_id = "cron-running-check";
    let now = now_unix();
    store
        .upsert_cron_run(&CronRunRecord {
            run_id: "running-1".into(),
            cron_id: cron_id.into(),
            scheduled_at: now,
            started_at: Some(now),
            finished_at: None,
            status: CronRunStatus::Running,
            exit_code: None,
            output: None,
            error: None,
        })
        .unwrap();
    assert!(store.cron_has_running(cron_id).unwrap());

    store
        .upsert_cron_run(&CronRunRecord {
            run_id: "running-1".into(),
            cron_id: cron_id.into(),
            scheduled_at: now,
            started_at: Some(now),
            finished_at: Some(now + 1),
            status: CronRunStatus::Succeeded,
            exit_code: Some(0),
            output: None,
            error: None,
        })
        .unwrap();
    assert!(!store.cron_has_running(cron_id).unwrap());
}

#[test]
fn cron_runs_prune_respects_retention_and_per_cron_cap() {
    let store = make_store();
    let now = now_unix();
    store
        .upsert_cron(&CronRecord {
            cron_id: "cron-prune".into(),
            name: "keep".into(),
            schedule: "* * * * * *".into(),
            command: "echo hi".into(),
            status: CronStatus::Active,
            skip_overlap: true,
            created_at: now,
            updated_at: now,
            next_run_at: None,
            last_run_at: None,
        })
        .unwrap();

    store
        .upsert_cron_run(&CronRunRecord {
            run_id: "old".into(),
            cron_id: "cron-prune".into(),
            scheduled_at: now - (3 * 86_400),
            started_at: Some(now - (3 * 86_400)),
            finished_at: Some(now - (3 * 86_400)),
            status: CronRunStatus::Succeeded,
            exit_code: Some(0),
            output: None,
            error: None,
        })
        .unwrap();
    for i in 0..5 {
        store
            .upsert_cron_run(&CronRunRecord {
                run_id: format!("new-{i}"),
                cron_id: "cron-prune".into(),
                scheduled_at: now + i as u64,
                started_at: Some(now + i as u64),
                finished_at: Some(now + i as u64 + 1),
                status: CronRunStatus::Succeeded,
                exit_code: Some(0),
                output: None,
                error: None,
            })
            .unwrap();
    }

    store.prune_cron_runs(1, 3).unwrap();

    let runs = store.list_cron_runs("cron-prune", 10).unwrap();
    assert_eq!(runs.len(), 3);
    assert!(!runs.iter().any(|run| run.run_id == "old"));
}
