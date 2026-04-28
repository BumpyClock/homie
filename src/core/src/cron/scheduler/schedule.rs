use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use cron::Schedule;

use crate::storage::{CronRecord, CronRunRecord, CronRunStatus, Store};

use super::runner::CronRunner;

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

pub(super) async fn process_due_runs(
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

pub(super) fn run_and_touch(
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

pub(super) fn touch_cron_last_run(
    store: &Arc<dyn Store>,
    cron_id: &str,
    last_run_at: u64,
) -> Result<(), String> {
    let mut cron = store
        .get_cron(cron_id)?
        .ok_or_else(|| format!("unknown cron: {cron_id}"))?;
    cron.last_run_at = Some(last_run_at);
    cron.updated_at = super::common::now_unix();
    store.upsert_cron(&cron)
}

pub(super) fn build_run_record(
    cron: &CronRecord,
    scheduled_at: u64,
    started_at: Option<u64>,
    finished_at: Option<u64>,
    status: CronRunStatus,
) -> CronRunRecord {
    CronRunRecord {
        run_id: uuid::Uuid::new_v4().to_string(),
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
