#![forbid(unsafe_code)]

use symphony_domain::IssueId;

pub fn issue_id(value: &str) -> IssueId {
    IssueId(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_builds_issue_id() {
        let id = issue_id("SYM-7");
        assert_eq!(id.0, "SYM-7");
    }
}
