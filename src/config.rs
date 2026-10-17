//! `tko.toml` discovery/parsing and backend token resolution.
//!
//! `tko.toml` is discovered by walking up from the current directory (like `tk`
//! finds `.tickets/`). It carries no secrets; the backend access token is
//! resolved separately from the environment, a project `.env`, or a
//! user-global `~/.config/tko/secrets.env`.
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// The backend kind selected in `tko.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    Gitea,
    Github,
    Gitlab,
}

/// The `[backend]` table of `tko.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct BackendConfig {
    pub kind: BackendKind,
    pub url: String,
    pub owner: String,
    pub repo: String,
    pub token_env: Option<String>,
}

/// A parsed `tko.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub backend: BackendConfig,

    /// Directory containing `tko.toml`; used to locate a sibling `.env`.
    #[serde(skip)]
    pub root: PathBuf,
}

/// The backend-specific fallback environment variable, if any.
fn fallback_env_var(kind: BackendKind) -> Option<&'static str> {
    match kind {
        BackendKind::Gitea => Some("GITEA_TOKEN"),
        BackendKind::Github => Some("GITHUB_TOKEN"),
        BackendKind::Gitlab => Some("GITLAB_TOKEN"),
    }
}

impl Config {
    /// Walk up from `start` looking for `tko.toml`, parse it, and return it.
    pub fn discover(start: &Path) -> Result<Config> {
        let path =
            find_config_file(start).ok_or_else(|| Error::ConfigNotFound(start.to_path_buf()))?;
        Self::load(&path)
    }

    /// Parse the `tko.toml` at `path`.
    pub fn load(path: &Path) -> Result<Config> {
        let contents = fs::read_to_string(path).map_err(Error::Io)?;
        let mut config: Config =
            toml::from_str(&contents).map_err(|e| Error::ConfigParse(e.to_string()))?;
        config.root = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(config)
    }

    /// Resolve the backend access token.
    ///
    /// Resolution order: the environment variable named by `token_env` (or the
    /// default `TKO_TOKEN`, or a backend-specific fallback such as
    /// `GITEA_TOKEN`), then the project's `.env`, then
    /// `~/.config/tko/secrets.env`.
    pub fn resolve_token(&self) -> Result<String> {
        let env_names = self.token_env_names();

        for name in &env_names {
            if let Ok(value) = std::env::var(name)
                && !value.is_empty()
            {
                return Ok(value);
            }
        }

        let project_env = self.root.join(".env");
        if let Some(value) = read_env_file_var(&project_env, &env_names) {
            return Ok(value);
        }

        if let Some(config_dir) = dirs_config_dir() {
            let secrets_env = config_dir.join("tko").join("secrets.env");
            if let Some(value) = read_env_file_var(&secrets_env, &env_names) {
                return Ok(value);
            }
        }

        Err(Error::TokenNotFound {
            env: env_names[0].clone(),
        })
    }

    /// The ordered set of environment variable names to check for the token.
    fn token_env_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        if let Some(name) = &self.backend.token_env {
            names.push(name.clone());
        } else {
            names.push("TKO_TOKEN".to_string());
        }
        if let Some(fallback) = fallback_env_var(self.backend.kind)
            && !names.iter().any(|n| n == fallback)
        {
            names.push(fallback.to_string());
        }
        names
    }
}

/// Walk up from `start` looking for a file named `tko.toml`.
fn find_config_file(start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let candidate = dir.join("tko.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// Parse a simple `KEY=value` env file and look up the first matching key.
fn read_env_file_var(path: &Path, names: &[String]) -> Option<String> {
    let vars = parse_env_file(path)?;
    for name in names {
        if let Some(value) = vars.get(name)
            && !value.is_empty()
        {
            return Some(value.clone());
        }
    }
    None
}

/// Parse a `KEY=value` file, ignoring blank lines and `#` comments.
fn parse_env_file(path: &Path) -> Option<HashMap<String, String>> {
    let contents = fs::read_to_string(path).ok()?;
    let mut vars = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_string();
        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = value[1..value.len() - 1].to_string();
        }
        vars.insert(key, value);
    }
    Some(vars)
}

/// The user-global config directory, honoring `XDG_CONFIG_HOME`.
fn dirs_config_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg));
    }
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn write(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn discovers_config_walking_up() {
        let tmp = tempdir();
        let root = tmp.path();
        write(
            &root.join("tko.toml"),
            r#"
            [backend]
            kind = "gitea"
            url = "https://gitea.example.com"
            owner = "myorg"
            repo = "myproject"
            "#,
        );
        let nested = root.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();

        let config = Config::discover(&nested).unwrap();
        assert_eq!(config.backend.owner, "myorg");
        assert_eq!(config.root, root);
    }

    #[test]
    fn missing_config_errors() {
        let tmp = tempdir();
        let err = Config::discover(tmp.path()).unwrap_err();
        assert!(matches!(err, Error::ConfigNotFound(_)));
    }

    #[test]
    fn invalid_toml_errors() {
        let tmp = tempdir();
        write(&tmp.path().join("tko.toml"), "not valid toml {{{");
        let err = Config::discover(tmp.path()).unwrap_err();
        assert!(matches!(err, Error::ConfigParse(_)));
    }

    /// Guards a single test's env var mutations, always restoring the
    /// previous process-wide state on drop (even on panic) so a failure in
    /// one test can't poison the ones that run after it. Also points
    /// `XDG_CONFIG_HOME` at an empty temp dir, isolating tests from any real
    /// `~/.config/tko/secrets.env` the developer running the suite might have.
    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        saved: Vec<(&'static str, Option<String>)>,
        _config_home: TempDir,
    }

    impl EnvGuard {
        fn new() -> Self {
            let lock = ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let names = ["TKO_TOKEN", "GITEA_TOKEN", "MY_TOKEN", "XDG_CONFIG_HOME"];
            let saved = names.iter().map(|&n| (n, std::env::var(n).ok())).collect();
            let config_home = tempdir();
            unsafe { std::env::set_var("XDG_CONFIG_HOME", config_home.path()) };
            Self {
                _lock: lock,
                saved,
                _config_home: config_home,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                match value {
                    Some(v) => unsafe { std::env::set_var(name, v) },
                    None => unsafe { std::env::remove_var(name) },
                }
            }
        }
    }

    #[test]
    fn token_resolved_from_default_env_var() {
        let _guard = EnvGuard::new();
        let tmp = tempdir();
        write(
            &tmp.path().join("tko.toml"),
            r#"
            [backend]
            kind = "gitea"
            url = "https://gitea.example.com"
            owner = "myorg"
            repo = "myproject"
            "#,
        );
        let config = Config::discover(tmp.path()).unwrap();

        unsafe { std::env::set_var("TKO_TOKEN", "abc123") };
        let token = config.resolve_token().unwrap();
        assert_eq!(token, "abc123");
    }

    #[test]
    fn token_resolved_from_custom_env_var() {
        let _guard = EnvGuard::new();
        let tmp = tempdir();
        write(
            &tmp.path().join("tko.toml"),
            r#"
            [backend]
            kind = "gitea"
            url = "https://gitea.example.com"
            owner = "myorg"
            repo = "myproject"
            token_env = "MY_TOKEN"
            "#,
        );
        let config = Config::discover(tmp.path()).unwrap();

        unsafe { std::env::set_var("MY_TOKEN", "xyz789") };
        let token = config.resolve_token().unwrap();
        assert_eq!(token, "xyz789");
    }

    #[test]
    fn token_resolved_from_backend_fallback_env_var() {
        let _guard = EnvGuard::new();
        let tmp = tempdir();
        write(
            &tmp.path().join("tko.toml"),
            r#"
            [backend]
            kind = "gitea"
            url = "https://gitea.example.com"
            owner = "myorg"
            repo = "myproject"
            "#,
        );
        let config = Config::discover(tmp.path()).unwrap();

        unsafe { std::env::set_var("GITEA_TOKEN", "fallback-token") };
        let token = config.resolve_token().unwrap();
        assert_eq!(token, "fallback-token");
    }

    #[test]
    fn token_resolved_from_project_dotenv() {
        let _guard = EnvGuard::new();
        let tmp = tempdir();
        write(
            &tmp.path().join("tko.toml"),
            r#"
            [backend]
            kind = "gitea"
            url = "https://gitea.example.com"
            owner = "myorg"
            repo = "myproject"
            "#,
        );
        write(&tmp.path().join(".env"), "TKO_TOKEN=from-dotenv\n");
        let config = Config::discover(tmp.path()).unwrap();

        let token = config.resolve_token().unwrap();
        assert_eq!(token, "from-dotenv");
    }

    #[test]
    fn env_var_takes_precedence_over_dotenv() {
        let _guard = EnvGuard::new();
        let tmp = tempdir();
        write(
            &tmp.path().join("tko.toml"),
            r#"
            [backend]
            kind = "gitea"
            url = "https://gitea.example.com"
            owner = "myorg"
            repo = "myproject"
            "#,
        );
        write(&tmp.path().join(".env"), "TKO_TOKEN=from-dotenv\n");
        let config = Config::discover(tmp.path()).unwrap();

        unsafe { std::env::set_var("TKO_TOKEN", "from-env") };
        let token = config.resolve_token().unwrap();
        assert_eq!(token, "from-env");
    }

    #[test]
    fn token_not_found_errors() {
        let _guard = EnvGuard::new();
        let tmp = tempdir();
        write(
            &tmp.path().join("tko.toml"),
            r#"
            [backend]
            kind = "gitea"
            url = "https://gitea.example.com"
            owner = "myorg"
            repo = "myproject"
            "#,
        );
        let config = Config::discover(tmp.path()).unwrap();

        let err = config.resolve_token().unwrap_err();
        assert!(matches!(err, Error::TokenNotFound { .. }));
    }

    /// Minimal owned temp directory, avoiding a new dependency for tests.
    struct TempDir(PathBuf);

    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn tempdir() -> TempDir {
        let mut dir = std::env::temp_dir();
        let unique = format!(
            "tko-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        dir.push(unique);
        fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
}
