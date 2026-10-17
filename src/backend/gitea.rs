//! [`GiteaBackend`]: the v1 [`Backend`] implementation against the Gitea REST
//! API.
//!
//! Every `tko` field is mirrored onto native Gitea issue fields on a
//! best-effort basis (title, body, state, labels, assignee, comments, native
//! issue dependencies); the caller is responsible for deciding which labels
//! (e.g. `tko:type:<t>`, `tko:prio:<n>`) represent which domain fields — this
//! module only knows how to talk to Gitea.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::{Backend, Comment, Issue, IssueState, IssueUpdate, NewIssue};
use crate::config::Config;
use crate::error::{Error, Result};

const PAGE_LIMIT: u32 = 50;

/// Talks to a single Gitea repository's issue tracker over its REST API.
pub struct GiteaBackend {
    client: reqwest::blocking::Client,
    api_base: String,
    token: String,
}

impl GiteaBackend {
    /// Build a backend from a parsed `tko.toml` and a resolved access token.
    pub fn new(config: &Config, token: String) -> Result<GiteaBackend> {
        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| Error::Backend(e.to_string()))?;
        let api_base = format!(
            "{}/api/v1/repos/{}/{}",
            config.backend.url.trim_end_matches('/'),
            config.backend.owner,
            config.backend.repo,
        );
        Ok(GiteaBackend {
            client,
            api_base,
            token,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.api_base, path)
    }

    fn auth(
        &self,
        builder: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        builder.header("Authorization", format!("token {}", self.token))
    }

    fn send<T: for<'de> Deserialize<'de>>(
        &self,
        builder: reqwest::blocking::RequestBuilder,
    ) -> Result<T> {
        let response = self
            .auth(builder)
            .send()
            .map_err(|e| Error::Backend(e.to_string()))?;
        let response = check_status(response)?;
        response
            .json::<T>()
            .map_err(|e| Error::Backend(e.to_string()))
    }

    fn send_no_body(&self, builder: reqwest::blocking::RequestBuilder) -> Result<()> {
        let response = self
            .auth(builder)
            .send()
            .map_err(|e| Error::Backend(e.to_string()))?;
        check_status(response)?;
        Ok(())
    }

    /// Resolve label names to their ids, creating any that don't yet exist.
    fn label_ids_for_names(&self, names: &[String]) -> Result<Vec<i64>> {
        if names.is_empty() {
            return Ok(Vec::new());
        }

        let existing = self.list_labels()?;
        let mut ids = Vec::with_capacity(names.len());
        for name in names {
            if let Some(label) = existing.iter().find(|l| &l.name == name) {
                ids.push(label.id);
            } else {
                let created = self.create_label(name)?;
                ids.push(created.id);
            }
        }
        Ok(ids)
    }

    fn list_labels(&self) -> Result<Vec<ApiLabel>> {
        let mut labels = Vec::new();
        let mut page = 1;
        loop {
            let url = format!("{}?page={page}&limit={PAGE_LIMIT}", self.url("/labels"));
            let batch: Vec<ApiLabel> = self.send(self.client.get(url))?;
            let got = batch.len();
            labels.extend(batch);
            if got < PAGE_LIMIT as usize {
                break;
            }
            page += 1;
        }
        Ok(labels)
    }

    fn create_label(&self, name: &str) -> Result<ApiLabel> {
        let body = CreateLabelBody {
            name,
            color: "#00aabb",
        };
        self.send(self.client.post(self.url("/labels")).json(&body))
    }
}

fn check_status(response: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
    if response.status().is_success() {
        Ok(response)
    } else {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        Err(Error::Backend(format!("gitea returned {status}: {body}")))
    }
}

impl Backend for GiteaBackend {
    fn create_issue(&self, new: NewIssue) -> Result<Issue> {
        let labels = self.label_ids_for_names(&new.labels)?;
        let body = CreateIssueBody {
            title: &new.title,
            body: &new.body,
            labels,
            assignees: new.assignee.iter().cloned().collect(),
        };
        let issue: ApiIssue = self.send(self.client.post(self.url("/issues")).json(&body))?;
        Ok(issue.into())
    }

    fn get_issue(&self, number: u64) -> Result<Issue> {
        let issue: ApiIssue = self.send(self.client.get(self.url(&format!("/issues/{number}"))))?;
        Ok(issue.into())
    }

    fn list_issues(&self) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let mut page = 1;
        loop {
            let url = format!(
                "{}?state=all&type=issues&page={page}&limit={PAGE_LIMIT}",
                self.url("/issues")
            );
            let batch: Vec<ApiIssue> = self.send(self.client.get(url))?;
            let got = batch.len();
            issues.extend(batch.into_iter().map(Issue::from));
            if got < PAGE_LIMIT as usize {
                break;
            }
            page += 1;
        }
        Ok(issues)
    }

    fn update_issue(&self, number: u64, update: IssueUpdate) -> Result<Issue> {
        let labels = update
            .labels
            .as_ref()
            .map(|names| self.label_ids_for_names(names))
            .transpose()?;

        let body = EditIssueBody {
            title: update.title,
            body: update.body,
            labels,
            assignees: update.assignee.map(|a| a.into_iter().collect()),
            state: None,
        };
        let issue: ApiIssue = self.send(
            self.client
                .patch(self.url(&format!("/issues/{number}")))
                .json(&body),
        )?;
        Ok(issue.into())
    }

    fn set_state(&self, number: u64, state: IssueState) -> Result<()> {
        let body = EditIssueBody {
            title: None,
            body: None,
            labels: None,
            assignees: None,
            state: None,
        }
        .with_state(state);
        let _: ApiIssue = self.send(
            self.client
                .patch(self.url(&format!("/issues/{number}")))
                .json(&body),
        )?;
        Ok(())
    }

    fn add_comment(&self, number: u64, body: &str) -> Result<Comment> {
        let payload = CreateCommentBody { body };
        let comment: ApiComment = self.send(
            self.client
                .post(self.url(&format!("/issues/{number}/comments")))
                .json(&payload),
        )?;
        comment.try_into()
    }

    fn list_comments(&self, number: u64) -> Result<Vec<Comment>> {
        let comments: Vec<ApiComment> = self.send(
            self.client
                .get(self.url(&format!("/issues/{number}/comments"))),
        )?;
        comments.into_iter().map(TryInto::try_into).collect()
    }

    fn add_dependency(&self, number: u64, depends_on: u64) -> Result<()> {
        let body = DependencyBody { index: depends_on };
        self.send_no_body(
            self.client
                .post(self.url(&format!("/issues/{number}/dependencies")))
                .json(&body),
        )
    }

    fn remove_dependency(&self, number: u64, depends_on: u64) -> Result<()> {
        let body = DependencyBody { index: depends_on };
        self.send_no_body(
            self.client
                .delete(self.url(&format!("/issues/{number}/dependencies")))
                .json(&body),
        )
    }

    fn list_dependencies(&self, number: u64) -> Result<Vec<u64>> {
        let issues: Vec<ApiIssue> = self.send(
            self.client
                .get(self.url(&format!("/issues/{number}/dependencies"))),
        )?;
        Ok(issues.into_iter().map(|i| i.number).collect())
    }
}

impl EditIssueBody {
    fn with_state(mut self, state: IssueState) -> Self {
        self.state = Some(match state {
            IssueState::Open => "open".to_string(),
            IssueState::Closed => "closed".to_string(),
        });
        self
    }
}

#[derive(Debug, Deserialize)]
struct ApiLabel {
    id: i64,
    name: String,
}

#[derive(Debug, Serialize)]
struct CreateLabelBody<'a> {
    name: &'a str,
    color: &'a str,
}

#[derive(Debug, Deserialize)]
struct ApiUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct ApiIssue {
    number: u64,
    title: String,
    #[serde(default)]
    body: String,
    state: String,
    #[serde(default)]
    labels: Vec<ApiLabel>,
    assignee: Option<ApiUser>,
}

impl From<ApiIssue> for Issue {
    fn from(issue: ApiIssue) -> Self {
        Issue {
            number: issue.number,
            title: issue.title,
            body: issue.body,
            state: if issue.state == "closed" {
                IssueState::Closed
            } else {
                IssueState::Open
            },
            labels: issue.labels.into_iter().map(|l| l.name).collect(),
            assignee: issue.assignee.map(|a| a.login),
        }
    }
}

#[derive(Debug, Serialize)]
struct CreateIssueBody<'a> {
    title: &'a str,
    body: &'a str,
    labels: Vec<i64>,
    assignees: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
struct EditIssueBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiComment {
    id: u64,
    body: String,
    created_at: String,
}

impl TryFrom<ApiComment> for Comment {
    type Error = Error;

    fn try_from(comment: ApiComment) -> Result<Self> {
        let created = OffsetDateTime::parse(&comment.created_at, &Rfc3339)
            .map_err(|e| Error::Backend(format!("invalid comment timestamp: {e}")))?;
        Ok(Comment {
            id: comment.id,
            body: comment.body,
            created,
        })
    }
}

#[derive(Debug, Serialize)]
struct CreateCommentBody<'a> {
    body: &'a str,
}

#[derive(Debug, Serialize)]
struct DependencyBody {
    index: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    fn test_backend(server_url: &str) -> GiteaBackend {
        let config = Config {
            backend: crate::config::BackendConfig {
                kind: crate::config::BackendKind::Gitea,
                url: server_url.to_string(),
                owner: "myorg".to_string(),
                repo: "myproject".to_string(),
                token_env: None,
            },
            root: std::path::PathBuf::from("."),
        };
        GiteaBackend::new(&config, "test-token".to_string()).unwrap()
    }

    #[test]
    fn get_issue_maps_fields() {
        let mut server = mockito::Server::new();
        let backend = test_backend(&server.url());

        let mock = server
            .mock("GET", "/api/v1/repos/myorg/myproject/issues/42")
            .match_header("authorization", "token test-token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"number":42,"title":"Fix bug","body":"---\nid: 42\n---\n","state":"open","labels":[{"id":1,"name":"tko:type:bug"}],"assignee":{"login":"alice"}}"#,
            )
            .create();

        let issue = backend.get_issue(42).unwrap();
        mock.assert();

        assert_eq!(issue.number, 42);
        assert_eq!(issue.title, "Fix bug");
        assert_eq!(issue.state, IssueState::Open);
        assert_eq!(issue.labels, vec!["tko:type:bug".to_string()]);
        assert_eq!(issue.assignee.as_deref(), Some("alice"));
    }

    #[test]
    fn create_issue_creates_missing_labels() {
        let mut server = mockito::Server::new();
        let backend = test_backend(&server.url());

        let list_labels = server
            .mock(
                "GET",
                Matcher::Regex(r"^/api/v1/repos/myorg/myproject/labels".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":1,"name":"tko:type:bug"}]"#)
            .create();

        let create_label = server
            .mock("POST", "/api/v1/repos/myorg/myproject/labels")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":2,"name":"tko:prio:1"}"#)
            .create();

        let create_issue = server
            .mock("POST", "/api/v1/repos/myorg/myproject/issues")
            .match_body(Matcher::PartialJson(serde_json::json!({
                "title": "New ticket",
                "labels": [1, 2],
            })))
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"number":1,"title":"New ticket","body":"","state":"open","labels":[],"assignee":null}"#,
            )
            .create();

        let issue = backend
            .create_issue(NewIssue {
                title: "New ticket".to_string(),
                body: String::new(),
                labels: vec!["tko:type:bug".to_string(), "tko:prio:1".to_string()],
                assignee: None,
            })
            .unwrap();

        list_labels.assert();
        create_label.assert();
        create_issue.assert();
        assert_eq!(issue.number, 1);
    }

    #[test]
    fn set_state_closed() {
        let mut server = mockito::Server::new();
        let backend = test_backend(&server.url());

        let mock = server
            .mock("PATCH", "/api/v1/repos/myorg/myproject/issues/7")
            .match_body(Matcher::Json(serde_json::json!({"state": "closed"})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"number":7,"title":"t","body":"","state":"closed","labels":[],"assignee":null}"#,
            )
            .create();

        backend.set_state(7, IssueState::Closed).unwrap();
        mock.assert();
    }

    #[test]
    fn add_and_list_comments() {
        let mut server = mockito::Server::new();
        let backend = test_backend(&server.url());

        let add = server
            .mock("POST", "/api/v1/repos/myorg/myproject/issues/3/comments")
            .match_body(Matcher::Json(serde_json::json!({"body": "hello"})))
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":10,"body":"hello","created_at":"2026-01-31T10:00:00Z"}"#)
            .create();

        let comment = backend.add_comment(3, "hello").unwrap();
        add.assert();
        assert_eq!(comment.id, 10);
        assert_eq!(comment.body, "hello");

        let list = server
            .mock("GET", "/api/v1/repos/myorg/myproject/issues/3/comments")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":10,"body":"hello","created_at":"2026-01-31T10:00:00Z"}]"#)
            .create();

        let comments = backend.list_comments(3).unwrap();
        list.assert();
        assert_eq!(comments.len(), 1);
    }

    #[test]
    fn dependency_wiring() {
        let mut server = mockito::Server::new();
        let backend = test_backend(&server.url());

        let add = server
            .mock(
                "POST",
                "/api/v1/repos/myorg/myproject/issues/5/dependencies",
            )
            .match_body(Matcher::Json(serde_json::json!({"index": 6})))
            .with_status(201)
            .create();
        backend.add_dependency(5, 6).unwrap();
        add.assert();

        let remove = server
            .mock(
                "DELETE",
                "/api/v1/repos/myorg/myproject/issues/5/dependencies",
            )
            .match_body(Matcher::Json(serde_json::json!({"index": 6})))
            .with_status(200)
            .create();
        backend.remove_dependency(5, 6).unwrap();
        remove.assert();

        let list = server
            .mock("GET", "/api/v1/repos/myorg/myproject/issues/5/dependencies")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"[{"number":6,"title":"dep","body":"","state":"open","labels":[],"assignee":null}]"#,
            )
            .create();
        let deps = backend.list_dependencies(5).unwrap();
        list.assert();
        assert_eq!(deps, vec![6]);
    }

    #[test]
    fn error_status_maps_to_backend_error() {
        let mut server = mockito::Server::new();
        let backend = test_backend(&server.url());

        let mock = server
            .mock("GET", "/api/v1/repos/myorg/myproject/issues/999")
            .with_status(404)
            .with_body("not found")
            .create();

        let err = backend.get_issue(999).unwrap_err();
        mock.assert();
        assert!(matches!(err, Error::Backend(_)));
    }
}
