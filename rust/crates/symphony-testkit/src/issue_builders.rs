use symphony_domain::IssueId;

pub fn issue_id(value: &str) -> IssueId {
    IssueId(value.to_owned())
}
