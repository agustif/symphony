use std::path::PathBuf;

pub trait EnvProvider {
    fn get(&self, key: &str) -> Option<String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ProcessEnv;

impl EnvProvider for ProcessEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }
}

pub fn resolve_env_reference(value: &str, env: &dyn EnvProvider) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if value.starts_with('$') {
        return parse_env_reference(value).and_then(|name| env.get(name));
    }

    Some(value.to_owned())
}

pub fn expand_workspace_root(value: &str, env: &dyn EnvProvider) -> Option<PathBuf> {
    let value = resolve_env_reference(value, env)?;

    if value == "~" {
        return home_dir(env);
    }

    if let Some(suffix) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return home_dir(env).map(|home| home.join(suffix));
    }

    Some(PathBuf::from(value))
}

fn home_dir(env: &dyn EnvProvider) -> Option<PathBuf> {
    env.get("HOME")
        .or_else(|| env.get("USERPROFILE"))
        .map(PathBuf::from)
}

fn parse_env_reference(value: &str) -> Option<&str> {
    if let Some(name) = value
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
    {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        return Some(name);
    }

    if let Some(name) = value.strip_prefix('$') {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        return Some(name);
    }

    None
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::Path};

    use super::{EnvProvider, expand_workspace_root, resolve_env_reference};

    struct TestEnv {
        values: HashMap<String, String>,
    }

    impl TestEnv {
        fn from_entries(entries: &[(&str, &str)]) -> Self {
            let values = entries
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect();
            Self { values }
        }
    }

    impl EnvProvider for TestEnv {
        fn get(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }
    }

    #[test]
    fn resolves_dollar_and_brace_env_references() {
        let env = TestEnv::from_entries(&[("TOKEN", "abc"), ("ROOT", "/tmp/workspaces")]);

        assert_eq!(
            resolve_env_reference("$TOKEN", &env),
            Some("abc".to_owned())
        );
        assert_eq!(
            resolve_env_reference("${TOKEN}", &env),
            Some("abc".to_owned())
        );
        assert_eq!(
            expand_workspace_root("${ROOT}", &env),
            Some(Path::new("/tmp/workspaces").to_path_buf())
        );
    }

    #[test]
    fn leaves_plain_values_unchanged_and_rejects_invalid_env_references() {
        let env = TestEnv::from_entries(&[]);

        assert_eq!(
            resolve_env_reference("literal", &env),
            Some("literal".to_owned())
        );
        assert_eq!(resolve_env_reference("${}", &env), None);
        assert_eq!(resolve_env_reference("$", &env), None);
    }

    #[test]
    fn expands_home_directory_prefixes() {
        let env = TestEnv::from_entries(&[("HOME", "/home/tester")]);

        assert_eq!(
            expand_workspace_root("~/work", &env),
            Some(Path::new("/home/tester/work").to_path_buf())
        );
        assert_eq!(
            expand_workspace_root("~", &env),
            Some(Path::new("/home/tester").to_path_buf())
        );
    }
}
