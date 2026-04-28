use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::sync::{broadcast, Semaphore};
use tokio::task::JoinHandle;
use tracing::warn;

use crate::router::ReapEvent;
use crate::storage::{CronRecord, CronRunRecord, CronRunStatus, CronStatus, Store};

use super::command::execute_command;
use super::common::{now_unix, MAX_MISSED_RUNS, SCHEDULER_TICK_SECS};
use super::schedule::{
    build_run_record, due_runs, process_due_runs, run_and_touch, schedule_next_after,
    touch_cron_last_run,
};

#[derive(Clone)]
pub struct CronRunner {
    pub(super) store: Arc<dyn Store>,
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

    pub(super) fn record_skipped_run(
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

    pub(super) async fn start_run(
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

pub fn spawn_cron_scheduler(
    runner: Arc<CronRunner>,
    prune_retention_days: u64,
    prune_max_runs: usize,
) -> JoinHandle<()> {
    runner.spawn(prune_retention_days, prune_max_runs)
}
