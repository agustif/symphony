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

    if let Some(name) = value.strip_prefix('$') {
        if name.is_empty() {
            return None;
        }
        return env.get(name);
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
