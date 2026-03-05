use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct GraphQlRequest<V> {
    pub(crate) query: &'static str,
    pub(crate) variables: V,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GraphQlResponse<D> {
    pub(crate) data: Option<D>,
    #[serde(default)]
    pub(crate) errors: Vec<GraphQlError>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GraphQlError {
    #[serde(default)]
    pub(crate) message: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PaginationVariables {
    pub(crate) first: usize,
    pub(crate) after: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FetchIssuesByStatesVariables {
    pub(crate) states: Vec<String>,
    pub(crate) first: usize,
    pub(crate) after: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FetchIssueStatesByIdsVariables {
    pub(crate) ids: Vec<String>,
    pub(crate) first: usize,
    pub(crate) after: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IssuesData {
    pub(crate) issues: IssueCollection<IssueNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IssueStatesData {
    pub(crate) issues: IssueCollection<IssueStateNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IssueCollection<T> {
    pub(crate) nodes: Vec<T>,
    #[serde(rename = "pageInfo")]
    pub(crate) page_info: PageInfo,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PageInfo {
    #[serde(rename = "hasNextPage")]
    pub(crate) has_next_page: bool,
    #[serde(rename = "endCursor")]
    pub(crate) end_cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IssueNode {
    pub(crate) id: String,
    pub(crate) identifier: String,
    pub(crate) state: IssueState,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IssueStateNode {
    pub(crate) id: String,
    pub(crate) state: IssueState,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IssueState {
    pub(crate) name: String,
}
