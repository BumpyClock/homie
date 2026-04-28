use serde_json::{json, Value};

use homie_protocol::{error_codes, Response};

use crate::cron::models::{
    clamp_limit, CronAddParams, CronIdParams, CronListParams, CronUpdateParams,
};
use crate::cron::scheduler::schedule_next_after;
use crate::storage::{CronRecord, CronStatus};

use super::common::{now_unix, parse_required_params};
use super::CronService;

impl CronService {
    pub(super) fn add(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronAddParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
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

    pub(super) fn start(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        self.add(req_id, params)
    }

    pub(super) fn list(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
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

    pub(super) fn update(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronUpdateParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
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

    pub(super) fn remove(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronIdParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
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

    pub(super) fn cancel(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronIdParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
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
}
