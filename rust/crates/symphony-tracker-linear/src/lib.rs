#![forbid(unsafe_code)]

mod graphql;

use std::collections::HashMap;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Serialize, de::DeserializeOwned};
use symphony_domain::IssueId;
use symphony_tracker::{TrackerClient, TrackerError, TrackerIssue, TrackerState};

const FETCH_CANDIDATES_QUERY: &str = r#"
query FetchCandidates {
  issues {
    nodes {
      id
      identifier
      state {
        name
      }
    }
  }
}
"#;

const FETCH_CANDIDATES_BY_STATES_QUERY: &str = r#"
query FetchCandidatesByStates($states: [String!]!) {
  issues(filter: { state: { name: { in: $states } } }) {
    nodes {
      id
      identifier
      state {
        name
      }
    }
  }
}
"#;

const FETCH_STATES_BY_IDS_QUERY: &str = r#"
query FetchStatesByIds($ids: [String!]!) {
  issues(filter: { id: { in: $ids } }) {
    nodes {
      id
      state {
        name
      }
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
        let data: graphql::IssuesData = self
            .post_graphql(FETCH_CANDIDATES_QUERY, graphql::EmptyVariables)
            .await?;
        data.issues
            .nodes
            .into_iter()
            .map(normalize_tracker_issue)
            .collect()
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
            let messages = response.errors.into_iter().map(|err| err.message).collect();
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
        let data: graphql::IssuesData = self
            .post_graphql(
                FETCH_CANDIDATES_BY_STATES_QUERY,
                graphql::FetchIssuesByStatesVariables {
                    states: states.iter().map(ToString::to_string).collect(),
                },
            )
            .await?;
        data.issues
            .nodes
            .into_iter()
            .map(normalize_tracker_issue)
            .collect()
    }

    async fn fetch_states_by_ids(
        &self,
        ids: &[IssueId],
    ) -> Result<HashMap<IssueId, TrackerState>, TrackerError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let data: graphql::IssueStatesData = self
            .post_graphql(
                FETCH_STATES_BY_IDS_QUERY,
                graphql::FetchIssueStatesByIdsVariables {
                    ids: ids.iter().map(|id| id.0.clone()).collect(),
                },
            )
            .await?;
        let mut states = HashMap::with_capacity(data.issues.nodes.len());
        for issue in data.issues.nodes {
            let (id, state) = normalize_tracker_state(issue)?;
            states.insert(id, state);
        }
        Ok(states)
    }
}
