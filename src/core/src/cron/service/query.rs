use serde_json::{json, Value};

use homie_protocol::{error_codes, Response};

use crate::cron::models::{clamp_limit, CronRunsParams};

use super::common::parse_required_params;
use super::{CronService, CronStatusParams};

impl CronService {
    pub(super) fn status(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronStatusParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
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

    pub(super) fn runs(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronRunsParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
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

    pub(super) fn logs_tail(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronRunsParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
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
