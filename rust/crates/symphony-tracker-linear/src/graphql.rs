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
    pub(crate) message: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct EmptyVariables;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FetchIssuesByStatesVariables {
    pub(crate) states: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FetchIssueStatesByIdsVariables {
    pub(crate) ids: Vec<String>,
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
