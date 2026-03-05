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
pub(crate) struct FetchIssuesVariables {
    #[serde(rename = "projectSlug")]
    pub(crate) project_slug: String,
    pub(crate) first: usize,
    #[serde(rename = "relationFirst")]
    pub(crate) relation_first: usize,
    pub(crate) after: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FetchIssuesByStatesVariables {
    #[serde(rename = "projectSlug")]
    pub(crate) project_slug: String,
    pub(crate) states: Vec<String>,
    pub(crate) first: usize,
    #[serde(rename = "relationFirst")]
    pub(crate) relation_first: usize,
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

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct LabelConnection {
    #[serde(default)]
    pub(crate) nodes: Vec<LabelNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct LabelNode {
    #[serde(default)]
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct InverseRelationConnection {
    #[serde(default)]
    pub(crate) nodes: Vec<InverseRelationNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct InverseRelationNode {
    #[serde(rename = "type", default)]
    pub(crate) relation_type: Option<String>,
    #[serde(default)]
    pub(crate) issue: Option<BlockerIssueNode>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct BlockerIssueNode {
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) identifier: Option<String>,
    #[serde(default)]
    pub(crate) state: Option<IssueState>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IssueNode {
    pub(crate) id: String,
    pub(crate) identifier: String,
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) priority: Option<serde_json::Value>,
    pub(crate) state: IssueState,
    #[serde(rename = "branchName", default)]
    pub(crate) branch_name: Option<String>,
    #[serde(default)]
    pub(crate) url: Option<String>,
    #[serde(default)]
    pub(crate) labels: LabelConnection,
    #[serde(rename = "inverseRelations", default)]
    pub(crate) inverse_relations: InverseRelationConnection,
    #[serde(rename = "createdAt", default)]
    pub(crate) created_at: Option<String>,
    #[serde(rename = "updatedAt", default)]
    pub(crate) updated_at: Option<String>,
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
