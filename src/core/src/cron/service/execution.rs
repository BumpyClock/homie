use serde_json::{json, Value};

use homie_protocol::{error_codes, Response};

use crate::cron::models::CronIdParams;

use super::common::parse_required_params;
use super::CronService;

impl CronService {
    pub(super) async fn run_now(&mut self, req_id: uuid::Uuid, params: Option<Value>) -> Response {
        let params: CronIdParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
        };

        match self.runner.run_cron_now(&params.cron_id).await {
            Ok(run) => Response::success(req_id, json!({ "run": run })),
            Err(err) => Response::error(req_id, error_codes::INVALID_PARAMS, err),
        }
    }

    pub(super) async fn run_force(
        &mut self,
        req_id: uuid::Uuid,
        params: Option<Value>,
    ) -> Response {
        let params: CronIdParams = match parse_required_params(req_id, params) {
            Ok(p) => p,
            Err(resp) => return resp,
        };

        match self.runner.run_cron_now_force(&params.cron_id, true).await {
            Ok(run) => Response::success(req_id, json!({ "run": run })),
            Err(err) => Response::error(req_id, error_codes::INVALID_PARAMS, err),
        }
    }
}
