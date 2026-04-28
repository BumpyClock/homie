#[derive(Debug, PartialEq, Eq)]
pub(super) struct ReadRequest {
    pub(super) path: String,
    pub(super) offset: usize,
    pub(super) limit: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct LsRequest {
    pub(super) path: Option<String>,
    pub(super) depth: usize,
    pub(super) limit: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FindRequest {
    pub(super) pattern: String,
    pub(super) path: Option<String>,
    pub(super) limit: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct GrepRequest {
    pub(super) pattern: String,
    pub(super) path: Option<String>,
    pub(super) include: Option<String>,
    pub(super) limit: usize,
}
