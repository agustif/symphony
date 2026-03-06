#![forbid(unsafe_code)]

mod graphql;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use async_trait::async_trait;
use chrono::DateTime;
use reqwest::Client;
use serde::{Serialize, de::DeserializeOwned};
use symphony_domain::IssueId;
use symphony_tracker::{
    TrackerBlockerRef, TrackerClient, TrackerError, TrackerIssue, TrackerState,
};

const LINEAR_PAGE_SIZE: usize = 50;
const DEFAULT_LINEAR_REQUEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_LINEAR_RETRY_MAX_ATTEMPTS: usize = 3;
const DEFAULT_LINEAR_RETRY_BASE_DELAY_MILLIS: u64 = 100;

const FETCH_CANDIDATES_QUERY: &str = r#"
query FetchCandidates($projectSlug: String!, $first: Int!, $relationFirst: Int!, $after: String) {
  issues(first: $first, after: $after, filter: { project: { slugId: { eq: $projectSlug } } }) {
    nodes {
      id
      identifier
      title
      description
      priority
      state {
        name
      }
      branchName
      url
      labels {
        nodes {
          name
        }
      }
      inverseRelations(first: $relationFirst) {
        nodes {
          type
          issue {
            id
            identifier
            state {
              name
            }
          }
        }
      }
      createdAt
      updatedAt
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

const FETCH_CANDIDATES_BY_STATES_QUERY: &str = r#"
query FetchCandidatesByStates($projectSlug: String!, $states: [String!]!, $first: Int!, $relationFirst: Int!, $after: String) {
  issues(first: $first, after: $after, filter: { project: { slugId: { eq: $projectSlug } }, state: { name: { in: $states } } }) {
    nodes {
      id
      identifier
      title
      description
      priority
      state {
        name
      }
      branchName
      url
      labels {
        nodes {
          name
        }
      }
      inverseRelations(first: $relationFirst) {
        nodes {
          type
          issue {
            id
            identifier
            state {
              name
            }
          }
        }
      }
      createdAt
      updatedAt
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"#;

const FETCH_STATES_BY_IDS_QUERY: &str = r#"
query FetchStatesByIds($ids: [ID!]!, $first: Int!, $after: String) {
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
    project_slug: String,
    candidate_states: Vec<TrackerState>,
    http_client: Client,
}

impl LinearTracker {
    pub fn new(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        project_slug: impl Into<String>,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_LINEAR_REQUEST_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self::with_client(http_client, endpoint, api_key, project_slug)
    }

    pub fn with_client(
        http_client: Client,
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        project_slug: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            api_key: normalize_api_key(api_key.into()),
            project_slug: project_slug.into().trim().to_owned(),
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
                    graphql::FetchIssuesVariables {
                        project_slug: self.project_slug.clone(),
                        first: LINEAR_PAGE_SIZE,
                        relation_first: LINEAR_PAGE_SIZE,
                        after: after.clone(),
                    },
                )
                .await?;

            for node in data.issues.nodes.into_iter().flatten() {
                let issue = normalize_tracker_issue(node)?;
                insert_unique_issue_id(
                    &mut seen_issue_ids,
                    &issue.id.0,
                    "graphql payload has duplicate issue id",
                )?;
                issues.push(issue);
            }

            match next_page_cursor(data.issues.page_info, "issues.pageInfo.endCursor")? {
                Some(cursor) => {
                    insert_unique_issue_id(
                        &mut seen_cursors,
                        &cursor,
                        "graphql pagination cursor repeated",
                    )?;
                    after = Some(cursor);
                }
                None => break,
            }
        }

        Ok(issues)
    }

    async fn post_graphql<V, D>(&self, query: &'static str, variables: V) -> Result<D, TrackerError>
    where
        V: Serialize + Clone,
        D: DeserializeOwned,
    {
        for attempt in 1..=DEFAULT_LINEAR_RETRY_MAX_ATTEMPTS {
            let response = match self
                .http_client
                .post(self.endpoint.as_str())
                .header("Authorization", self.api_key.as_str())
                .json(&graphql::GraphQlRequest {
                    query,
                    variables: variables.clone(),
                })
                .send()
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    if attempt < DEFAULT_LINEAR_RETRY_MAX_ATTEMPTS {
                        tokio::time::sleep(retry_backoff(attempt)).await;
                        continue;
                    }
                    return Err(TrackerError::transport(err.to_string()));
                }
            };

            let status = response.status();
            let body = match response.bytes().await {
                Ok(body) => body,
                Err(err) => {
                    if attempt < DEFAULT_LINEAR_RETRY_MAX_ATTEMPTS {
                        tokio::time::sleep(retry_backoff(attempt)).await;
                        continue;
                    }
                    return Err(TrackerError::transport(err.to_string()));
                }
            };

            if !status.is_success() {
                if should_retry_status(status) && attempt < DEFAULT_LINEAR_RETRY_MAX_ATTEMPTS {
                    tokio::time::sleep(retry_backoff(attempt)).await;
                    continue;
                }

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
            return response
                .data
                .ok_or_else(|| TrackerError::payload("graphql payload missing data"));
        }

        unreachable!("bounded retry loop should always return or continue");
    }
}

fn normalize_api_key(api_key: String) -> String {
    let trimmed = api_key.trim();
    trimmed
        .strip_prefix("Bearer ")
        .or_else(|| trimmed.strip_prefix("bearer "))
        .unwrap_or(trimmed)
        .to_owned()
}

fn retry_backoff(attempt: usize) -> Duration {
    let exponent = attempt.saturating_sub(1).min(4) as u32;
    Duration::from_millis(DEFAULT_LINEAR_RETRY_BASE_DELAY_MILLIS.saturating_mul(1_u64 << exponent))
}

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::REQUEST_TIMEOUT
        || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error()
}

fn insert_unique_issue_id(
    seen: &mut HashSet<String>,
    value: &str,
    message: &str,
) -> Result<(), TrackerError> {
    if !seen.insert(value.to_owned()) {
        return Err(TrackerError::payload(format!("{message} `{value}`")));
    }
    Ok(())
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

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim().to_owned();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

fn normalize_tracker_issue(issue: graphql::IssueNode) -> Result<TrackerIssue, TrackerError> {
    Ok(TrackerIssue {
        id: IssueId(normalize_non_empty(issue.id, "id")?),
        identifier: normalize_non_empty(issue.identifier, "identifier")?,
        title: normalize_non_empty(issue.title, "title")?,
        description: normalize_optional_string(issue.description),
        priority: parse_priority(issue.priority),
        state: TrackerState::new(normalize_non_empty(issue.state.name, "state.name")?),
        branch_name: normalize_optional_string(issue.branch_name),
        url: normalize_optional_string(issue.url),
        labels: normalize_labels(issue.labels),
        blocked_by: normalize_blockers(issue.inverse_relations),
        created_at: parse_timestamp(issue.created_at),
        updated_at: parse_timestamp(issue.updated_at),
    })
}

fn normalize_tracker_state(
    issue: graphql::IssueStateNode,
) -> Result<Option<(IssueId, TrackerState)>, TrackerError> {
    let issue_id = IssueId(normalize_non_empty(issue.id, "id")?);
    let Some(state) = issue.state else {
        return Ok(None);
    };

    Ok(Some((
        issue_id,
        TrackerState::new(normalize_non_empty(state.name, "state.name")?),
    )))
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

fn normalize_labels(labels: graphql::LabelConnection) -> Vec<String> {
    let mut normalized_labels = Vec::new();
    let mut seen = HashSet::new();
    for label in labels.nodes.into_iter().flatten() {
        let Some(label) = label.name else {
            continue;
        };
        let normalized = label.trim().to_ascii_lowercase();
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        normalized_labels.push(normalized);
    }
    normalized_labels
}

fn normalize_blockers(relations: graphql::InverseRelationConnection) -> Vec<TrackerBlockerRef> {
    relations
        .nodes
        .into_iter()
        .flatten()
        .filter_map(|relation| {
            let relation_type = normalize_optional_string(relation.relation_type)?;
            if !relation_type.eq_ignore_ascii_case("blocks") {
                return None;
            }

            let blocker_issue = relation.issue?;
            Some(TrackerBlockerRef {
                id: normalize_optional_string(blocker_issue.id),
                identifier: normalize_optional_string(blocker_issue.identifier),
                state: blocker_issue
                    .state
                    .and_then(|state| normalize_optional_string(Some(state.name))),
            })
        })
        .collect()
}

fn parse_priority(priority: Option<serde_json::Value>) -> Option<i32> {
    priority
        .and_then(|priority| priority.as_i64())
        .and_then(|priority| i32::try_from(priority).ok())
}

fn parse_timestamp(raw: Option<String>) -> Option<u64> {
    raw.and_then(|raw| {
        DateTime::parse_from_rfc3339(raw.trim())
            .ok()
            .and_then(|timestamp| u64::try_from(timestamp.timestamp()).ok())
    })
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
                        project_slug: self.project_slug.clone(),
                        states: state_filters.clone(),
                        first: LINEAR_PAGE_SIZE,
                        relation_first: LINEAR_PAGE_SIZE,
                        after: after.clone(),
                    },
                )
                .await?;

            for node in data.issues.nodes.into_iter().flatten() {
                let issue = normalize_tracker_issue(node)?;
                insert_unique_issue_id(
                    &mut seen_issue_ids,
                    &issue.id.0,
                    "graphql payload has duplicate issue id",
                )?;
                issues.push(issue);
            }

            match next_page_cursor(data.issues.page_info, "issues.pageInfo.endCursor")? {
                Some(cursor) => {
                    insert_unique_issue_id(
                        &mut seen_cursors,
                        &cursor,
                        "graphql pagination cursor repeated",
                    )?;
                    after = Some(cursor);
                }
                None => break,
            }
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

            for issue in data.issues.nodes.into_iter().flatten() {
                let Some((id, state)) = normalize_tracker_state(issue)? else {
                    continue;
                };
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

            match next_page_cursor(data.issues.page_info, "issues.pageInfo.endCursor")? {
                Some(cursor) => {
                    insert_unique_issue_id(
                        &mut seen_cursors,
                        &cursor,
                        "graphql pagination cursor repeated",
                    )?;
                    after = Some(cursor);
                }
                None => break,
            }
        }

        Ok(states)
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_api_key;

    #[test]
    fn normalize_api_key_trims_optional_bearer_prefix() {
        assert_eq!(
            normalize_api_key("linear-api-key".to_owned()),
            "linear-api-key"
        );
        assert_eq!(
            normalize_api_key("Bearer linear-api-key".to_owned()),
            "linear-api-key"
        );
        assert_eq!(
            normalize_api_key(" bearer linear-api-key ".to_owned()),
            "linear-api-key"
        );
    }
}
