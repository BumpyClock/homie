use serde::Deserialize;

use crate::storage::CronStatus;

#[derive(Debug, Deserialize)]
pub struct CronAddParams {
    pub name: String,
    pub schedule: String,
    pub command: String,
    pub status: Option<CronStatus>,
    pub skip_overlap: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CronUpdateParams {
    pub cron_id: String,
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub command: Option<String>,
    pub status: Option<CronStatus>,
    pub skip_overlap: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CronIdParams {
    pub cron_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CronRunsParams {
    pub cron_id: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CronListParams {
    pub status: Option<CronStatus>,
    pub limit: Option<usize>,
}

pub fn clamp_limit(limit: Option<usize>, fallback: usize, max: usize) -> usize {
    limit.unwrap_or(fallback).max(1).min(max)
}
