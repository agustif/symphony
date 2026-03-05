#![forbid(unsafe_code)]

mod graphql;

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, de::DeserializeOwned};
use symphony_domain::IssueId;
use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue, TrackerState};

const LINEAR_PAGE_SIZE: usize = 100;

const FETCH_CANDIDATES_QUERY: &str = r#"
query FetchCandidates($first: Int!, $after: String) {
  issues(first: $first, after: $after) {
    nodes {
      id
      identifier
      state {
        name
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

const FETCH_CANDIDATES_BY_STATES_QUERY: &str = r#"
query FetchCandidatesByStates($states: [String!]!, $first: Int!, $after: String) {
  issues(first: $first, after: $after, filter: { state: { name: { in: $states } } }) {
    nodes {
      id
      identifier
      state {
        name
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

const FETCH_STATES_BY_IDS_QUERY: &str = r#"
query FetchStatesByIds($ids: [String!]!, $first: Int!, $after: String) {
  issues(first: $first, after: $after, filter: { id: { in: $ids } }) {
    nodes {
      id
      state {
        name
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

#[derive(Clone, Debug)]
pub struct LinearTracker {
    endpoint: String,
    api_key: String,
    candidate_states: Vec<TrackerState>,
    http_client: Client,
}

impl LinearTracker {
    pub fn new(endpoint: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self::with_client(Client::new(), endpoint, api_key)
    }

    pub fn with_client(
        http_client: Client,
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: api_key.into(),
            candidate_states: Vec::new(),
            http_client,
        }
    }

    pub fn with_candidate_states(mut self, candidate_states: Vec<TrackerState>) -> Self {
        self.candidate_states = candidate_states;
        self
    }

    async fn fetch_all_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
        let mut issues = Vec::new();
        let mut seen_cursors = HashSet::new();
        let mut seen_issue_ids = HashSet::new();
        let mut after = None;

        loop {
            let data: graphql::IssuesData = self
                .post_graphql(
                    FETCH_CANDIDATES_QUERY,
                    graphql::PaginationVariables {
                        first: LINEAR_PAGE_SIZE,
                        after: after.clone(),
                    },
                )
                .await?;
            for node in data.issues.nodes {
                let issue = normalize_tracker_issue(node)?;
                if !seen_issue_ids.insert(issue.id.0.clone()) {
                    return Err(TrackerError::payload(format!(
                        "graphql payload has duplicate issue id `{}`",
                        issue.id.0
                    )));
                }
                issues.push(issue);
            }
            let next_cursor = next_page_cursor(data.issues.page_info, "issues.pageInfo.endCursor")?;
            if let Some(cursor) = next_cursor {
                if !seen_cursors.insert(cursor.clone()) {
                    return Err(TrackerError::payload(format!(
                        "graphql pagination cursor repeated: `{cursor}`"
                    )));
                }
                after = Some(cursor);
                continue;
            }
            break;
        }

        Ok(issues)
    }

    async fn post_graphql<V, D>(&self, query: &'static str, variables: V) -> Result<D, TrackerError>
    where
        V: Serialize,
        D: DeserializeOwned,
    {
        let response = self
            .http_client
            .post(self.endpoint.as_str())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&graphql::GraphQlRequest { query, variables })
            .send()
            .await
            .map_err(|err| TrackerError::transport(err.to_string()))?;
        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|err| TrackerError::transport(err.to_string()))?;
        if !status.is_success() {
            return Err(TrackerError::status(
                status.as_u16(),
                String::from_utf8_lossy(&body).to_string(),
            ));
        }

        let response = serde_json::from_slice::<graphql::GraphQlResponse<D>>(&body)
            .map_err(|err| TrackerError::payload(format!("invalid graphql payload: {err}")))?;
        if !response.errors.is_empty() {
            let messages = response
                .errors
                .into_iter()
                .map(|err| {
                    let message = err.message.trim().to_owned();
                    if message.is_empty() {
                        "unknown graphql error".to_owned()
                    } else {
                        message
                    }
                })
                .collect();
            return Err(TrackerError::graphql(messages));
        }
        response
            .data
            .ok_or_else(|| TrackerError::payload("graphql payload missing data"))
    }
}

fn normalize_non_empty(value: String, field: &'static str) -> Result<String, TrackerError> {
    let normalized = value.trim().to_owned();
    if normalized.is_empty() {
        return Err(TrackerError::payload(format!(
            "graphql payload field `{field}` cannot be empty"
        )));
    }
    Ok(normalized)
}

fn normalize_tracker_issue(issue: graphql::IssueNode) -> Result<TrackerIssue, TrackerError> {
    Ok(TrackerIssue::new(
        IssueId(normalize_non_empty(issue.id, "id")?),
        normalize_non_empty(issue.identifier, "identifier")?,
        TrackerState::new(normalize_non_empty(issue.state.name, "state.name")?),
    ))
}

fn normalize_tracker_state(
    issue: graphql::IssueStateNode,
) -> Result<(IssueId, TrackerState), TrackerError> {
    Ok((
        IssueId(normalize_non_empty(issue.id, "id")?),
        TrackerState::new(normalize_non_empty(issue.state.name, "state.name")?),
    ))
}

fn normalize_state_filters(states: &[TrackerState]) -> Vec<String> {
    let mut normalized_states = Vec::new();
    let mut seen = HashSet::new();
    for state in states {
        let normalized = state.normalized();
        if normalized.is_empty() {
            continue;
        }
        let state_key = normalized.to_ascii_lowercase();
        if seen.insert(state_key) {
            normalized_states.push(normalized);
        }
    }
    normalized_states
}

fn normalize_requested_ids(ids: &[IssueId]) -> Result<Vec<String>, TrackerError> {
    let mut normalized_ids = Vec::new();
    let mut seen = HashSet::new();
    for issue_id in ids {
        let normalized = normalize_non_empty(issue_id.0.clone(), "variables.ids[]")?;
        if seen.insert(normalized.clone()) {
            normalized_ids.push(normalized);
        }
    }
    Ok(normalized_ids)
}

fn next_page_cursor(
    page_info: graphql::PageInfo,
    field: &'static str,
) -> Result<Option<String>, TrackerError> {
    if !page_info.has_next_page {
        return Ok(None);
    }
    let cursor = page_info.end_cursor.ok_or_else(|| {
        TrackerError::payload(format!("graphql payload field `{field}` is missing"))
    })?;
    Ok(Some(normalize_non_empty(cursor, field)?))
}

#[async_trait]
impl TrackerClient for LinearTracker {
    async fn fetch_candidates(&self) -> Result<Vec<TrackerIssue>, TrackerError> {
        if self.candidate_states.is_empty() {
            return self.fetch_all_candidates().await;
        }
        self.fetch_candidates_by_states(self.candidate_states.as_slice())
            .await
    }

    async fn fetch_candidates_by_states(
        &self,
        states: &[TrackerState],
    ) -> Result<Vec<TrackerIssue>, TrackerError> {
        if states.is_empty() {
            return self.fetch_all_candidates().await;
        }
        let state_filters = normalize_state_filters(states);
        if state_filters.is_empty() {
            return Ok(Vec::new());
        }

        let mut issues = Vec::new();
        let mut seen_cursors = HashSet::new();
        let mut seen_issue_ids = HashSet::new();
        let mut after = None;

        loop {
            let data: graphql::IssuesData = self
                .post_graphql(
                    FETCH_CANDIDATES_BY_STATES_QUERY,
                    graphql::FetchIssuesByStatesVariables {
                        states: state_filters.clone(),
                        first: LINEAR_PAGE_SIZE,
                        after: after.clone(),
                    },
                )
                .await?;
            for node in data.issues.nodes {
                let issue = normalize_tracker_issue(node)?;
                if !seen_issue_ids.insert(issue.id.0.clone()) {
                    return Err(TrackerError::payload(format!(
                        "graphql payload has duplicate issue id `{}`",
                        issue.id.0
                    )));
                }
                issues.push(issue);
            }
            let next_cursor = next_page_cursor(data.issues.page_info, "issues.pageInfo.endCursor")?;
            if let Some(cursor) = next_cursor {
                if !seen_cursors.insert(cursor.clone()) {
                    return Err(TrackerError::payload(format!(
                        "graphql pagination cursor repeated: `{cursor}`"
                    )));
                }
                after = Some(cursor);
                continue;
            }
            break;
        }

        Ok(issues)
    }

    async fn fetch_states_by_ids(
        &self,
        ids: &[IssueId],
    ) -> Result<HashMap<IssueId, TrackerState>, TrackerError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let requested_ids = normalize_requested_ids(ids)?;
        let requested_id_set = requested_ids.iter().cloned().collect::<HashSet<_>>();

        let mut states = HashMap::new();
        let mut seen_cursors = HashSet::new();
        let mut after = None;

        loop {
            let data: graphql::IssueStatesData = self
                .post_graphql(
                    FETCH_STATES_BY_IDS_QUERY,
                    graphql::FetchIssueStatesByIdsVariables {
                        ids: requested_ids.clone(),
                        first: LINEAR_PAGE_SIZE,
                        after: after.clone(),
                    },
                )
                .await?;
            for issue in data.issues.nodes {
                let (id, state) = normalize_tracker_state(issue)?;
                if !requested_id_set.contains(&id.0) {
                    continue;
                }
                if states.contains_key(&id) {
                    return Err(TrackerError::payload(format!(
                        "graphql payload has duplicate issue id `{}` for state refresh",
                        id.0
                    )));
                }
                states.insert(id, state);
            }
            let next_cursor = next_page_cursor(data.issues.page_info, "issues.pageInfo.endCursor")?;
            if let Some(cursor) = next_cursor {
                if !seen_cursors.insert(cursor.clone()) {
                    return Err(TrackerError::payload(format!(
                        "graphql pagination cursor repeated: `{cursor}`"
                    )));
                }
                after = Some(cursor);
                continue;
            }
            break;
        }
        Ok(states)
    }
}
