//! The `tk` ticket document: YAML frontmatter plus a markdown body.
//!
//! This is the exact on-the-wire format `tk` uses, and it is the source of
//! truth for a ticket — backends mirror it onto native issue fields on a
//! best-effort basis, but [`Ticket::parse`] / [`Ticket::render`] are what
//! actually round-trip a ticket.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::error::{Error, Result};

/// The three-state ticket status. Always present in frontmatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    InProgress,
    Closed,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Status::Open => "open",
            Status::InProgress => "in_progress",
            Status::Closed => "closed",
        };
        f.write_str(s)
    }
}

impl std::str::FromStr for Status {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "open" => Ok(Status::Open),
            "in_progress" => Ok(Status::InProgress),
            "closed" => Ok(Status::Closed),
            other => Err(Error::TicketFormat(format!("invalid status: {other}"))),
        }
    }
}

/// The ticket type. Omitted from frontmatter when unset, defaulting to
/// [`Type::Task`] downstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    Task,
    Bug,
    Feature,
    Epic,
    Chore,
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Type::Task => "task",
            Type::Bug => "bug",
            Type::Feature => "feature",
            Type::Epic => "epic",
            Type::Chore => "chore",
        };
        f.write_str(s)
    }
}

impl std::str::FromStr for Type {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "task" => Ok(Type::Task),
            "bug" => Ok(Type::Bug),
            "feature" => Ok(Type::Feature),
            "epic" => Ok(Type::Epic),
            "chore" => Ok(Type::Chore),
            other => Err(Error::TicketFormat(format!("invalid type: {other}"))),
        }
    }
}

/// The valid priority range, 0 (highest) through 4 (lowest).
pub const MIN_PRIORITY: i64 = 0;
pub const MAX_PRIORITY: i64 = 4;
pub const DEFAULT_PRIORITY: i64 = 2;

/// Checks that `priority` falls within `[MIN_PRIORITY, MAX_PRIORITY]`.
pub fn validate_priority(priority: i64) -> Result<()> {
    if !(MIN_PRIORITY..=MAX_PRIORITY).contains(&priority) {
        return Err(Error::TicketFormat(format!(
            "invalid priority {priority}: must be between {MIN_PRIORITY} and {MAX_PRIORITY} ({MIN_PRIORITY}=highest)"
        )));
    }
    Ok(())
}

/// A timestamped note, rendered under `## Notes` as a `### <RFC3339>` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub timestamp: OffsetDateTime,
    pub content: String,
}

/// A ticket: YAML frontmatter fields plus markdown body sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub id: String,
    pub status: Status,

    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub ticket_type: Option<Type>,

    /// 0 = highest priority. Matches `tk`'s Go `omitempty` quirk: a priority
    /// of `0` is indistinguishable from "unset" and is omitted on render.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub priority: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,

    #[serde(
        rename = "external-ref",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub external_ref: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deps: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<String>,

    #[serde(with = "time::serde::rfc3339")]
    pub created: OffsetDateTime,

    #[serde(skip)]
    pub title: String,
    #[serde(skip)]
    pub description: String,
    #[serde(skip)]
    pub design: String,
    #[serde(skip)]
    pub acceptance: String,
    #[serde(skip)]
    pub notes: Vec<Note>,
}

fn is_zero(priority: &i64) -> bool {
    *priority == 0
}

impl Ticket {
    /// Renders the ticket as a JSON value, including the body fields that are
    /// excluded from the YAML frontmatter serialization (title, description,
    /// design, acceptance, notes).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "status": self.status.to_string(),
            "type": self.ticket_type.map(|t| t.to_string()),
            "priority": self.priority,
            "assignee": self.assignee,
            "parent": self.parent,
            "external_ref": self.external_ref,
            "pr": self.pr,
            "tags": self.tags,
            "deps": self.deps,
            "links": self.links,
            "created": self.created.format(&Rfc3339).unwrap_or_default(),
            "title": self.title,
            "description": self.description,
            "design": self.design,
            "acceptance": self.acceptance,
            "notes": self.notes.iter().map(|n| serde_json::json!({
                "timestamp": n.timestamp.format(&Rfc3339).unwrap_or_default(),
                "content": n.content,
            })).collect::<Vec<_>>(),
        })
    }
}

impl Ticket {
    /// Parses a ticket document (frontmatter + markdown body).
    pub fn parse(data: &[u8]) -> Result<Ticket> {
        let text = std::str::from_utf8(data).map_err(|e| Error::TicketFormat(e.to_string()))?;
        let (frontmatter, body) = split_frontmatter(text)?;

        let mut ticket: Ticket = serde_yaml::from_str(frontmatter)
            .map_err(|e| Error::TicketFormat(format!("failed to parse frontmatter: {e}")))?;

        let (title, description, design, acceptance, notes) = parse_body(body);
        ticket.title = title;
        ticket.description = description;
        ticket.design = design;
        ticket.acceptance = acceptance;
        ticket.notes = notes;

        Ok(ticket)
    }

    /// Renders the ticket back into a document: frontmatter + markdown body.
    pub fn render(&self) -> Result<Vec<u8>> {
        let mut frontmatter = serde_yaml::to_string(self)
            .map_err(|e| Error::TicketFormat(format!("failed to marshal frontmatter: {e}")))?;
        if let Some(rest) = frontmatter.strip_prefix("---\n") {
            frontmatter = rest.to_string();
        }

        let mut out = String::with_capacity(frontmatter.len() + 128);
        out.push_str("---\n");
        out.push_str(&frontmatter);
        out.push_str("---\n");
        out.push_str(&render_body(self));

        Ok(out.into_bytes())
    }
}

/// Splits `---\n<frontmatter>\n---\n<body>` into its frontmatter and body.
fn split_frontmatter(text: &str) -> Result<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| Error::TicketFormat("missing frontmatter delimiter".into()))?;
    rest.split_once("\n---\n")
        .ok_or_else(|| Error::TicketFormat("missing closing frontmatter delimiter".into()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    None,
    Title,
    Description,
    Design,
    Acceptance,
    Notes,
}

fn flush_section(
    section: Section,
    buf: &mut String,
    title: &mut String,
    description: &mut String,
    design: &mut String,
    acceptance: &mut String,
    notes_raw: &mut String,
) {
    let text = buf.trim();
    match section {
        Section::Title => {
            if !text.is_empty() {
                *title = text.to_string();
            }
        }
        Section::Description => *description = text.to_string(),
        Section::Design => *design = text.to_string(),
        Section::Acceptance => *acceptance = text.to_string(),
        Section::Notes => *notes_raw = text.to_string(),
        Section::None => {}
    }
    buf.clear();
}

/// Parses the markdown body into (title, description, design, acceptance, notes).
fn parse_body(content: &str) -> (String, String, String, String, Vec<Note>) {
    let mut title = String::new();
    let mut description = String::new();
    let mut design = String::new();
    let mut acceptance = String::new();
    let mut notes_raw = String::new();

    let mut section = Section::None;
    let mut buf = String::new();

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("# ") {
            flush_section(
                section,
                &mut buf,
                &mut title,
                &mut description,
                &mut design,
                &mut acceptance,
                &mut notes_raw,
            );
            title = rest.to_string();
            section = Section::Title;
            continue;
        }

        if let Some(rest) = line.strip_prefix("## ") {
            flush_section(
                section,
                &mut buf,
                &mut title,
                &mut description,
                &mut design,
                &mut acceptance,
                &mut notes_raw,
            );
            section = match rest.to_lowercase().as_str() {
                "design" => Section::Design,
                "acceptance criteria" => Section::Acceptance,
                "notes" => Section::Notes,
                _ => {
                    buf.push_str(line);
                    buf.push('\n');
                    Section::Description
                }
            };
            continue;
        }

        if section == Section::Title && !line.is_empty() {
            section = Section::Description;
        }

        buf.push_str(line);
        buf.push('\n');
    }

    flush_section(
        section,
        &mut buf,
        &mut title,
        &mut description,
        &mut design,
        &mut acceptance,
        &mut notes_raw,
    );

    (
        title,
        description,
        design,
        acceptance,
        parse_notes(&notes_raw),
    )
}

/// Parses `### <RFC3339>` note entries from the `## Notes` section content.
fn parse_notes(content: &str) -> Vec<Note> {
    let mut notes = Vec::new();
    let mut current: Option<(OffsetDateTime, String)> = None;

    for line in content.lines() {
        if let Some(timestamp) = line.strip_prefix("### ") {
            if let Some((timestamp, buf)) = current.take() {
                notes.push(Note {
                    timestamp,
                    content: buf.trim().to_string(),
                });
            }
            if let Ok(parsed) = OffsetDateTime::parse(timestamp, &Rfc3339) {
                current = Some((parsed, String::new()));
            }
            continue;
        }

        if let Some((_, buf)) = current.as_mut() {
            buf.push_str(line);
            buf.push('\n');
        }
    }

    if let Some((timestamp, buf)) = current {
        notes.push(Note {
            timestamp,
            content: buf.trim().to_string(),
        });
    }

    notes
}

/// Renders the body fields (title, description, design, acceptance, notes) as markdown.
fn render_body(ticket: &Ticket) -> String {
    let mut buf = String::new();

    if !ticket.title.is_empty() {
        buf.push_str("# ");
        buf.push_str(&ticket.title);
        buf.push_str("\n\n");
    }

    if !ticket.description.is_empty() {
        buf.push_str(&ticket.description);
        buf.push_str("\n\n");
    }

    if !ticket.design.is_empty() {
        buf.push_str("## Design\n\n");
        buf.push_str(&ticket.design);
        buf.push_str("\n\n");
    }

    if !ticket.acceptance.is_empty() {
        buf.push_str("## Acceptance Criteria\n\n");
        buf.push_str(&ticket.acceptance);
        buf.push_str("\n\n");
    }

    if !ticket.notes.is_empty() {
        buf.push_str("## Notes\n\n");
        for note in &ticket.notes {
            buf.push_str("### ");
            buf.push_str(&note.timestamp.format(&Rfc3339).unwrap_or_default());
            buf.push_str("\n\n");
            buf.push_str(&note.content);
            buf.push_str("\n\n");
        }
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn parse_status() {
        assert_eq!("open".parse::<Status>().unwrap(), Status::Open);
        assert_eq!("in_progress".parse::<Status>().unwrap(), Status::InProgress);
        assert_eq!("closed".parse::<Status>().unwrap(), Status::Closed);
        assert!("invalid".parse::<Status>().is_err());
    }

    #[test]
    fn status_display() {
        assert_eq!(Status::Open.to_string(), "open");
        assert_eq!(Status::InProgress.to_string(), "in_progress");
        assert_eq!(Status::Closed.to_string(), "closed");
    }

    #[test]
    fn parse_type() {
        assert_eq!("task".parse::<Type>().unwrap(), Type::Task);
        assert_eq!("bug".parse::<Type>().unwrap(), Type::Bug);
        assert_eq!("feature".parse::<Type>().unwrap(), Type::Feature);
        assert_eq!("epic".parse::<Type>().unwrap(), Type::Epic);
        assert_eq!("chore".parse::<Type>().unwrap(), Type::Chore);
        assert!("invalid".parse::<Type>().is_err());
    }

    #[test]
    fn validate_priority_range() {
        assert!(validate_priority(MIN_PRIORITY).is_ok());
        assert!(validate_priority(MAX_PRIORITY).is_ok());
        assert!(validate_priority(DEFAULT_PRIORITY).is_ok());
        assert!(validate_priority(MIN_PRIORITY - 1).is_err());
        assert!(validate_priority(MAX_PRIORITY + 1).is_err());
        assert!(validate_priority(-100).is_err());
        assert!(validate_priority(999).is_err());
    }

    #[test]
    fn parse_full_document() {
        let content = "---\n\
id: tic-abc1\n\
status: open\n\
type: task\n\
priority: 2\n\
assignee: John Doe\n\
deps: [tic-xyz1]\n\
links: []\n\
created: 2026-01-31T10:00:00Z\n\
---\n\
# Test Ticket\n\
\n\
This is the description.\n\
\n\
## Design\n\
\n\
Design notes here.\n\
\n\
## Acceptance Criteria\n\
\n\
- [ ] Criterion 1\n\
- [ ] Criterion 2\n\
\n\
## Notes\n\
\n\
### 2026-01-31T11:00:00Z\n\
\n\
First note content.\n\
\n\
### 2026-01-31T12:00:00Z\n\
\n\
Second note content.\n";

        let ticket = Ticket::parse(content.as_bytes()).unwrap();

        assert_eq!(ticket.id, "tic-abc1");
        assert_eq!(ticket.status, Status::Open);
        assert_eq!(ticket.ticket_type, Some(Type::Task));
        assert_eq!(ticket.priority, 2);
        assert_eq!(ticket.assignee.as_deref(), Some("John Doe"));
        assert_eq!(ticket.deps, vec!["tic-xyz1"]);
        assert_eq!(ticket.title, "Test Ticket");
        assert!(ticket.description.contains("This is the description"));
        assert!(ticket.design.contains("Design notes here"));
        assert!(ticket.acceptance.contains("Criterion 1"));
        assert_eq!(ticket.notes.len(), 2);
        assert!(ticket.notes[0].content.contains("First note content"));
        assert!(ticket.notes[1].content.contains("Second note content"));
    }

    #[test]
    fn parse_invalid_frontmatter() {
        assert!(Ticket::parse(b"# Title\n\nContent").is_err());
        assert!(Ticket::parse(b"---\nid: test\n# Title").is_err());
    }

    #[test]
    fn render_document() {
        let ticket = Ticket {
            id: "tic-test1".to_string(),
            status: Status::InProgress,
            ticket_type: Some(Type::Feature),
            priority: 1,
            assignee: Some("Jane Doe".to_string()),
            parent: None,
            external_ref: None,
            pr: None,
            tags: vec![],
            deps: vec!["tic-dep1".to_string()],
            links: vec![],
            created: datetime!(2026-01-31 10:00:00 UTC),
            title: "Feature Title".to_string(),
            description: "Feature description.".to_string(),
            design: "Design details.".to_string(),
            acceptance: "- [ ] Accept this".to_string(),
            notes: vec![Note {
                timestamp: datetime!(2026-01-31 11:00:00 UTC),
                content: "Note content".to_string(),
            }],
        };

        let rendered = ticket.render().unwrap();
        let content = String::from_utf8(rendered).unwrap();

        assert!(content.contains("id: tic-test1"));
        assert!(content.contains("status: in_progress"));
        assert!(content.contains("type: feature"));
        assert!(content.contains("priority: 1"));
        assert!(content.contains("# Feature Title"));
        assert!(content.contains("Feature description"));
        assert!(content.contains("## Design"));
        assert!(content.contains("## Acceptance Criteria"));
        assert!(content.contains("## Notes"));
    }

    #[test]
    fn round_trip() {
        let original = Ticket {
            id: "tic-round1".to_string(),
            status: Status::Open,
            ticket_type: Some(Type::Bug),
            priority: 3,
            assignee: Some("Developer".to_string()),
            parent: None,
            external_ref: None,
            pr: None,
            tags: vec!["urgent".to_string(), "backend".to_string()],
            deps: vec!["tic-dep1".to_string(), "tic-dep2".to_string()],
            links: vec!["tic-link1".to_string()],
            created: datetime!(2026-01-31 10:00:00 UTC),
            title: "Bug Title".to_string(),
            description: "Bug description with details.".to_string(),
            design: "Fix approach.".to_string(),
            acceptance: "- [ ] Bug is fixed\n- [ ] Tests pass".to_string(),
            notes: vec![],
        };

        let rendered = original.render().unwrap();
        let parsed = Ticket::parse(&rendered).unwrap();

        assert_eq!(original.id, parsed.id);
        assert_eq!(original.status, parsed.status);
        assert_eq!(original.ticket_type, parsed.ticket_type);
        assert_eq!(original.priority, parsed.priority);
        assert_eq!(original.assignee, parsed.assignee);
        assert_eq!(original.tags, parsed.tags);
        assert_eq!(original.deps, parsed.deps);
        assert_eq!(original.links, parsed.links);
        assert_eq!(original.title, parsed.title);
    }

    #[test]
    fn priority_zero_is_omitted_on_render() {
        let ticket = Ticket {
            id: "tic-p0".to_string(),
            status: Status::Open,
            ticket_type: None,
            priority: 0,
            assignee: None,
            parent: None,
            external_ref: None,
            pr: None,
            tags: vec![],
            deps: vec![],
            links: vec![],
            created: datetime!(2026-01-31 10:00:00 UTC),
            title: "No priority".to_string(),
            description: String::new(),
            design: String::new(),
            acceptance: String::new(),
            notes: vec![],
        };

        let rendered = ticket.render().unwrap();
        let content = String::from_utf8(rendered).unwrap();
        assert!(!content.contains("priority:"));

        let parsed = Ticket::parse(content.as_bytes()).unwrap();
        assert_eq!(parsed.priority, 0);
    }

    #[test]
    fn title_preserved_after_status_change() {
        let content = "---\n\
id: test-1234\n\
status: open\n\
type: task\n\
priority: 2\n\
created: 2026-01-31T17:10:46.21915Z\n\
---\n\
# My Test Title\n\
\n\
## Section\n\
- item\n";

        let mut ticket = Ticket::parse(content.as_bytes()).unwrap();
        assert_eq!(ticket.title, "My Test Title");

        ticket.status = Status::InProgress;

        let rendered = ticket.render().unwrap();
        let ticket2 = Ticket::parse(&rendered).unwrap();
        assert_eq!(ticket2.title, "My Test Title");
    }
}
