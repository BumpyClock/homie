use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use cron::Schedule;
use serde_json::json;
use tokio::process::Command;
use tokio::sync::{broadcast, Semaphore};
use tokio::task::JoinHandle;
use tracing::warn;
use uuid::Uuid;

use crate::router::ReapEvent;
use crate::storage::{CronRecord, CronRunRecord, CronRunStatus, CronStatus, Store};

const SCHEDULER_TICK_SECS: u64 = 1;
const MAX_MISSED_RUNS: usize = 32;
const MAX_OUTPUT_BYTES: usize = 16_384;

#[derive(Clone)]
pub struct CronRunner {
    store: Arc<dyn Store>,
    event_tx: broadcast::Sender<ReapEvent>,
    max_concurrency: usize,
    run_slots: Arc<Semaphore>,
}

impl CronRunner {
    pub fn new(
        store: Arc<dyn Store>,
        max_concurrent_runs: usize,
        event_tx: broadcast::Sender<ReapEvent>,
    ) -> Self {
        Self {
            store,
            event_tx,
            max_concurrency: max_concurrent_runs,
            run_slots: Arc::new(Semaphore::new(max_concurrent_runs)),
        }
    }

    pub fn spawn(
        self: Arc<Self>,
        prune_retention_days: u64,
        prune_max_runs: usize,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(SCHEDULER_TICK_SECS));
            let mut prune_tick = tokio::time::interval(Duration::from_secs(60 * 60));
            tick.tick().await;
            prune_tick.tick().await;
            loop {
                tokio::select! {
                    _ = tick.tick() => {
                        if let Err(err) = self.clone().tick_once().await {
                            warn!(error = %err, "cron scheduler tick failed");
                        }
                    }
                    _ = prune_tick.tick() => {
                        if let Err(err) = self.store.prune_cron_runs(prune_retention_days, prune_max_runs) {
                            warn!(error = %err, "cron run prune failed");
                        }
                    }
                }
            }
        })
    }

    async fn tick_once(self: &Arc<Self>) -> Result<(), String> {
        let now = now_unix();
        let mut crons = self.store.list_crons()?;
        for mut cron in crons.drain(..) {
            if cron.status != CronStatus::Active {
                continue;
            }

            let Some(next_run_at) = cron.next_run_at else {
                if let Ok(next_run_at) = schedule_next_after(&cron.schedule, now) {
                    cron.next_run_at = Some(next_run_at);
                    cron.updated_at = now;
                    self.store.upsert_cron(&cron)?;
                }
                continue;
            };

            let due = due_runs(&cron.schedule, next_run_at, now, MAX_MISSED_RUNS)?;
            if due.is_empty() {
                continue;
            }

            process_due_runs(self.clone(), cron, now, due).await?;
        }
        Ok(())
    }

    pub async fn run_cron_now(self: &Arc<Self>, cron_id: &str) -> Result<CronRunRecord, String> {
        self.run_cron_now_force(cron_id, false).await
    }

    pub async fn run_cron_now_force(
        self: &Arc<Self>,
        cron_id: &str,
        force: bool,
    ) -> Result<CronRunRecord, String> {
        let mut cron = self
            .store
            .get_cron(cron_id)?
            .ok_or_else(|| format!("unknown cron: {cron_id}"))?;

        if cron.status == CronStatus::Paused {
            return Err("cron is paused".into());
        }

        let now = now_unix();
        if !force && cron.skip_overlap && self.store.cron_has_running(&cron.cron_id)? {
            let run =
                self.record_skipped_run(&cron, now, now, "overlapped with running cron run")?;
            run_and_touch(&self.store, &mut cron, now, &run)?;
            return Ok(run);
        }

        let next_run_at = schedule_next_after(&cron.schedule, now)?;
        cron.next_run_at = Some(next_run_at);
        cron.last_run_at = Some(now);
        cron.updated_at = now;
        self.store.upsert_cron(&cron)?;

        self.start_run(&cron, now).await
    }

    fn emit(&self, topic: &str, params: serde_json::Value) {
        let _ = self.event_tx.send(ReapEvent::new(topic, Some(params)));
    }

    fn emit_run_started(&self, run: &CronRunRecord) {
        self.emit(
            "cron.run.started",
            json!({
                "cron_id": run.cron_id,
                "run_id": run.run_id,
                "scheduled_at": run.scheduled_at,
                "status": run.status.as_str(),
            }),
        );
    }

    fn emit_run_skipped(&self, run: &CronRunRecord, reason: &str) {
        self.emit(
            "cron.run.skipped",
            json!({
                "cron_id": run.cron_id,
                "run_id": run.run_id,
                "scheduled_at": run.scheduled_at,
                "status": run.status.as_str(),
                "reason": reason,
                "error": run.error,
            }),
        );
    }

    fn emit_run_completed(&self, run: &CronRunRecord) {
        self.emit(
            "cron.run.completed",
            json!({
                "cron_id": run.cron_id,
                "run_id": run.run_id,
                "scheduled_at": run.scheduled_at,
                "started_at": run.started_at,
                "finished_at": run.finished_at,
                "status": run.status.as_str(),
                "exit_code": run.exit_code,
                "error": run.error,
            }),
        );
    }

    fn record_skipped_run(
        &self,
        cron: &CronRecord,
        scheduled_at: u64,
        timestamp: u64,
        reason: &str,
    ) -> Result<CronRunRecord, String> {
        let mut run = build_run_record(
            cron,
            scheduled_at,
            Some(timestamp),
            Some(timestamp),
            CronRunStatus::Skipped,
        );
        run.error = Some(reason.to_string());
        self.store.upsert_cron_run(&run)?;
        self.emit_run_skipped(&run, reason);
        Ok(run)
    }

    async fn start_run(
        self: &Arc<Self>,
        cron: &CronRecord,
        scheduled_at: u64,
    ) -> Result<CronRunRecord, String> {
        let run = build_run_record(
            cron,
            scheduled_at,
            Some(now_unix()),
            None,
            CronRunStatus::Running,
        );

        let Some(permit) = self.run_slots.clone().try_acquire_owned().ok() else {
            let run = self.record_skipped_run(
                cron,
                scheduled_at,
                scheduled_at,
                &format!(
                    "global cron concurrency limit reached: {}",
                    self.max_concurrency
                ),
            )?;
            return Ok(run);
        };

        self.store.upsert_cron_run(&run)?;
        self.emit_run_started(&run);

        let mut run_for_task = run.clone();
        let store = self.store.clone();
        let command = cron.command.clone();
        let cron_id = run.cron_id.clone();
        let run_id = run.run_id.clone();
        let runner_for_task = self.clone();

        tokio::spawn(async move {
            let output = execute_command(&command).await;
            let ended_at = now_unix();
            match output {
                Ok((status, code, result, error)) => {
                    run_for_task.status = status;
                    run_for_task.exit_code = code;
                    run_for_task.output = result;
                    run_for_task.error = error;
                    run_for_task.finished_at = Some(ended_at);
                }
                Err(err) => {
                    run_for_task.status = CronRunStatus::Failed;
                    run_for_task.exit_code = Some(1);
                    run_for_task.error = Some(err);
                    run_for_task.finished_at = Some(ended_at);
                }
            }

            if let Err(err) = store.upsert_cron_run(&run_for_task) {
                warn!(error = %err, run_id = %run_id, cron_id = %cron_id, "failed to persist cron run result");
            }

            if let Err(err) = touch_cron_last_run(&store, &cron_id, run_for_task.scheduled_at) {
                warn!(error = %err, cron_id = %cron_id, "failed to persist cron last_run_at");
            }

            runner_for_task.emit_run_completed(&run_for_task);

            drop(permit);
        });

        Ok(run)
    }
}

/// Parse a cron schedule and get the next timestamp (in seconds since epoch) after `after`.
pub(crate) fn schedule_next_after(expression: &str, after: u64) -> Result<u64, String> {
    let schedule = parse_schedule(expression)?;
    let anchor = unix_to_datetime(after)?;
    let next = schedule
        .after(&anchor)
        .next()
        .ok_or_else(|| "unable to compute next schedule time".to_string())?;
    Ok(next.timestamp() as u64)
}

/// Return all missed run times from `next_run_at` up to `now`, capped to `max`.
pub(crate) fn due_runs(
    expression: &str,
    next_run_at: u64,
    now: u64,
    max: usize,
) -> Result<Vec<u64>, String> {
    if now < next_run_at {
        return Ok(Vec::new());
    }

    let schedule = parse_schedule(expression)?;
    let mut cursor = unix_to_datetime(next_run_at)?
        .checked_sub_signed(chrono::Duration::seconds(1))
        .unwrap_or_else(|| unix_to_datetime(next_run_at).expect("next_run_at anchor"));
    let mut runs = Vec::new();
    for _ in 0..max {
        let next = schedule
            .after(&cursor)
            .next()
            .ok_or_else(|| "unable to compute scheduled run".to_string())?;
        let next_unix = next.timestamp() as u64;
        if next_unix > now {
            break;
        }
        runs.push(next_unix);
        cursor = next;
    }
    Ok(runs)
}

pub fn spawn_cron_scheduler(
    runner: Arc<CronRunner>,
    prune_retention_days: u64,
    prune_max_runs: usize,
) -> JoinHandle<()> {
    runner.spawn(prune_retention_days, prune_max_runs)
}

async fn process_due_runs(
    runner: Arc<CronRunner>,
    mut cron: CronRecord,
    now: u64,
    due_runs: Vec<u64>,
) -> Result<(), String> {
    let last_due = *due_runs
        .last()
        .ok_or_else(|| "no due runs to process".to_string())?;
    let next_run_at = schedule_next_after(&cron.schedule, last_due)?;

    if cron.skip_overlap {
        if runner.store.cron_has_running(&cron.cron_id)? {
            for due in due_runs {
                let _ = runner.record_skipped_run(
                    &cron,
                    due,
                    now,
                    "overlapped with running cron run",
                )?;
            }

            cron.last_run_at = Some(last_due);
            cron.next_run_at = Some(next_run_at);
            cron.updated_at = now;
            return runner.store.upsert_cron(&cron);
        }

        let first_due = due_runs[0];
        let _ = runner.start_run(&cron, first_due).await?;

        for due in due_runs.iter().skip(1) {
            let _ = runner.record_skipped_run(
                &cron,
                *due,
                now,
                "overlapped due to missed execution",
            )?;
        }

        cron.last_run_at = Some(last_due);
    } else {
        for scheduled_at in due_runs {
            let _ = runner.start_run(&cron, scheduled_at).await?;
        }
        cron.last_run_at = Some(last_due);
    }

    cron.next_run_at = Some(next_run_at);
    cron.updated_at = now;
    runner.store.upsert_cron(&cron)?;
    Ok(())
}

fn run_and_touch(
    store: &Arc<dyn Store>,
    cron: &mut CronRecord,
    now: u64,
    run: &CronRunRecord,
) -> Result<(), String> {
    store.upsert_cron_run(run)?;
    cron.last_run_at = Some(now);
    cron.updated_at = now;
    store.upsert_cron(cron)
}

fn touch_cron_last_run(
    store: &Arc<dyn Store>,
    cron_id: &str,
    last_run_at: u64,
) -> Result<(), String> {
    let mut cron = store
        .get_cron(cron_id)?
        .ok_or_else(|| format!("unknown cron: {cron_id}"))?;
    cron.last_run_at = Some(last_run_at);
    cron.updated_at = now_unix();
    store.upsert_cron(&cron)
}

fn build_run_record(
    cron: &CronRecord,
    scheduled_at: u64,
    started_at: Option<u64>,
    finished_at: Option<u64>,
    status: CronRunStatus,
) -> CronRunRecord {
    CronRunRecord {
        run_id: Uuid::new_v4().to_string(),
        cron_id: cron.cron_id.clone(),
        scheduled_at,
        started_at,
        finished_at,
        status,
        exit_code: None,
        output: None,
        error: None,
    }
}

fn parse_schedule(expression: &str) -> Result<Schedule, String> {
    Schedule::from_str(expression).map_err(|err| format!("invalid schedule: {err}"))
}

fn unix_to_datetime(unix_secs: u64) -> Result<DateTime<Utc>, String> {
    Utc.timestamp_opt(unix_secs as i64, 0)
        .single()
        .ok_or_else(|| "invalid schedule anchor timestamp".to_string())
}

async fn execute_command(
    command: &str,
) -> Result<(CronRunStatus, Option<i64>, Option<String>, Option<String>), String> {
    #[cfg(unix)]
    let mut cmd = {
        let mut c = Command::new("sh");
        c.arg("-lc").arg(command);
        c
    };

    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    };

    let command_output = cmd
        .output()
        .await
        .map_err(|err| format!("failed to run cron command: {err}"))?;

    let mut combined = String::new();
    if !command_output.stdout.is_empty() {
        combined.push_str(&String::from_utf8_lossy(&command_output.stdout));
    }
    if !command_output.stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&String::from_utf8_lossy(&command_output.stderr));
    }
    if combined.len() > MAX_OUTPUT_BYTES {
        combined.truncate(MAX_OUTPUT_BYTES);
    }

    if command_output.status.success() {
        let output = if combined.is_empty() {
            None
        } else {
            Some(combined)
        };
        Ok((
            CronRunStatus::Succeeded,
            output_status_code(&command_output.status),
            output,
            None,
        ))
    } else {
        let err = if combined.is_empty() {
            Some("command exited with failure".to_string())
        } else {
            Some(combined.clone())
        };
        let output = if combined.is_empty() {
            None
        } else {
            Some(combined)
        };
        Ok((
            CronRunStatus::Failed,
            output_status_code(&command_output.status),
            output,
            err,
        ))
    }
}

fn output_status_code(status: &std::process::ExitStatus) -> Option<i64> {
    status.code().map(|v| v as i64)
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
    use crate::storage::SqliteStore;

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
        process_due_runs(runner.clone(), cron.clone(), now() + 1, due.clone())
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
        process_due_runs(runner.clone(), cron.clone(), now() + 1, due.clone())
            .await
            .unwrap();

        let runs = runner.store.list_cron_runs(&cron.cron_id, 10).unwrap();
        assert_eq!(runs.len(), due.len());

        let latest = runner.store.get_cron(&cron.cron_id).unwrap().unwrap();
        assert_eq!(latest.last_run_at, Some(*due.last().unwrap()));
    }
}
