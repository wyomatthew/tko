//! v1 command implementations, wired against the [`Backend`] trait.
//!
//! Every command reads/writes the [`Ticket`] document as the issue body and
//! best-effort mirrors it onto native issue fields (title, labels, assignee,
//! state); the document remains the source of truth.

use std::io::Read;

use time::OffsetDateTime;

use crate::backend::{self, Backend, IssueState, IssueUpdate, NewIssue};
use crate::cli::{Command, DepCommand, ListFilters};
use crate::ticket::{Note, Status, Ticket, Type};

/// Dispatches a parsed [`Command`] against `backend`.
pub fn dispatch(backend: &dyn Backend, json: bool, command: Command) -> anyhow::Result<()> {
    match command {
        Command::Create {
            title,
            description,
            design,
            acceptance,
            ticket_type,
            priority,
            assignee,
            external_ref,
            pr,
            parent,
            tags,
        } => create(
            backend,
            json,
            title,
            description,
            design,
            acceptance,
            ticket_type,
            priority,
            assignee,
            external_ref,
            pr,
            parent,
            tags,
        ),
        Command::Show { id } => show(backend, json, &id),
        Command::List { status, filters } => list(backend, json, status, filters),
        Command::Ready { filters } => ready(backend, json, filters),
        Command::Blocked { filters } => blocked(backend, json, filters),
        Command::Closed { limit, filters } => closed(backend, json, limit, filters),
        Command::Start { id } => set_status(backend, json, &id, Status::InProgress),
        Command::Close { id } => set_status(backend, json, &id, Status::Closed),
        Command::Reopen { id } => set_status(backend, json, &id, Status::Open),
        Command::Status { id, status } => {
            let status: Status = status.parse()?;
            set_status(backend, json, &id, status)
        }
        Command::Edit { id } => edit(backend, &id),
        Command::AddNote { id, text } => add_note(backend, json, &id, text),
        Command::Dep { command } => match command {
            DepCommand::Add { id, dep_id } => dep_add(backend, json, &id, &dep_id),
            DepCommand::Remove { id, dep_id } => dep_remove(backend, json, &id, &dep_id),
            DepCommand::Tree { id, full } => dep_tree(backend, id, full),
            DepCommand::Check => dep_check(backend, json),
        },
        Command::Undep { id, dep_id } => dep_remove(backend, json, &id, &dep_id),
        Command::Link { ids } => link(backend, json, ids),
        Command::Unlink { id, target_id } => unlink(backend, json, &id, &target_id),
        Command::Init { .. } => unreachable!("Init is handled in main before backend setup"),
    }
}

fn resolve(backend: &dyn Backend, input: &str) -> anyhow::Result<u64> {
    let issues = backend.list_issues()?;
    Ok(backend::resolve_id(input, issues.iter().map(|i| i.number))?)
}

fn read_ticket(backend: &dyn Backend, number: u64) -> anyhow::Result<Ticket> {
    let issue = backend.get_issue(number)?;
    Ok(Ticket::parse(issue.body.as_bytes())?)
}

fn all_tickets(backend: &dyn Backend) -> anyhow::Result<Vec<Ticket>> {
    backend
        .list_issues()?
        .iter()
        .map(|issue| Ticket::parse(issue.body.as_bytes()).map_err(Into::into))
        .collect()
}

/// The best-effort label mirror for a ticket: tags plus `tko:type:<t>`,
/// `tko:prio:<n>` (when non-default), and `tko:in_progress`.
fn labels_for(ticket: &Ticket) -> Vec<String> {
    let mut labels = ticket.tags.clone();
    if let Some(t) = ticket.ticket_type {
        labels.push(format!("tko:type:{t}"));
    }
    if ticket.priority != 0 {
        labels.push(format!("tko:prio:{}", ticket.priority));
    }
    if ticket.status == Status::InProgress {
        labels.push("tko:in_progress".to_string());
    }
    labels
}

/// Persists a ticket's document and mirrored fields back to its issue.
fn persist(backend: &dyn Backend, ticket: &Ticket) -> anyhow::Result<()> {
    let number: u64 = ticket
        .id
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid ticket id: {}", ticket.id))?;
    let body = String::from_utf8(ticket.render()?)?;
    backend.update_issue(
        number,
        IssueUpdate {
            title: Some(ticket.title.clone()),
            body: Some(body),
            labels: Some(labels_for(ticket)),
            assignee: Some(ticket.assignee.clone()),
        },
    )?;
    let state = if ticket.status == Status::Closed {
        IssueState::Closed
    } else {
        IssueState::Open
    };
    backend.set_state(number, state)?;
    Ok(())
}

fn print_ticket(json: bool, ticket: &Ticket) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&ticket.to_json())?);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create(
    backend: &dyn Backend,
    json: bool,
    title: Option<String>,
    description: Option<String>,
    design: Option<String>,
    acceptance: Option<String>,
    ticket_type: Option<String>,
    priority: Option<u8>,
    assignee: Option<String>,
    external_ref: Option<String>,
    pr: Option<String>,
    parent: Option<String>,
    tags: Vec<String>,
) -> anyhow::Result<()> {
    let priority = priority.map(i64::from).unwrap_or(crate::ticket::DEFAULT_PRIORITY);
    crate::ticket::validate_priority(priority)?;

    let parent = parent.map(|p| resolve(backend, &p).map(|n| n.to_string())).transpose()?;

    let ticket_type = match ticket_type {
        Some(t) => Some(t.parse::<Type>()?),
        None => Some(Type::Task),
    };

    let mut ticket = Ticket {
        id: String::new(),
        status: Status::Open,
        ticket_type,
        priority,
        assignee,
        parent,
        external_ref,
        pr,
        tags,
        deps: Vec::new(),
        links: Vec::new(),
        created: OffsetDateTime::now_utc(),
        title: title.unwrap_or_default(),
        description: description.unwrap_or_default(),
        design: design.unwrap_or_default(),
        acceptance: acceptance.unwrap_or_default(),
        notes: Vec::new(),
    };

    let body = String::from_utf8(ticket.render()?)?;
    let issue = backend.create_issue(NewIssue {
        title: ticket.title.clone(),
        body,
        labels: labels_for(&ticket),
        assignee: ticket.assignee.clone(),
    })?;

    ticket.id = issue.number.to_string();
    persist(backend, &ticket)?;

    if json {
        print_ticket(json, &ticket)?;
    } else {
        println!("{}", ticket.id);
    }
    Ok(())
}

fn show(backend: &dyn Backend, json: bool, id: &str) -> anyhow::Result<()> {
    let number = resolve(backend, id)?;
    let ticket = read_ticket(backend, number)?;

    if json {
        return print_ticket(json, &ticket);
    }

    let tickets = all_tickets(backend)?;

    let mut output = String::from_utf8(ticket.render()?)?;
    if let Some(parent_id) = &ticket.parent {
        let parent_title = tickets
            .iter()
            .find(|t| &t.id == parent_id)
            .map(|t| t.title.clone());
        let line = match parent_title {
            Some(title) if !title.is_empty() => format!("parent: {parent_id}  # {title}"),
            _ => format!("parent: {parent_id}"),
        };
        output = output.replacen("links:", &format!("{line}\nlinks:"), 1);
    }

    let rel = relationships(&ticket.id, &ticket, &tickets);
    print!("{output}");
    if let Some(text) = format_relationships(&rel) {
        println!("---");
        print!("{text}");
    }
    Ok(())
}

struct Relationships {
    blockers: Vec<String>,
    blocking: Vec<String>,
    children: Vec<String>,
    links: Vec<String>,
}

fn relationships(id: &str, ticket: &Ticket, all: &[Ticket]) -> Relationships {
    let mut rel = Relationships {
        blockers: ticket.deps.clone(),
        blocking: Vec::new(),
        children: Vec::new(),
        links: ticket.links.clone(),
    };
    for t in all {
        if t.id == id {
            continue;
        }
        if t.deps.iter().any(|d| d == id) {
            rel.blocking.push(t.id.clone());
        }
        if t.parent.as_deref() == Some(id) {
            rel.children.push(t.id.clone());
        }
    }
    rel
}

fn format_relationships(rel: &Relationships) -> Option<String> {
    let mut lines = Vec::new();
    if !rel.blockers.is_empty() {
        lines.push(format!("Blockers: {}", rel.blockers.join(", ")));
    }
    if !rel.blocking.is_empty() {
        lines.push(format!("Blocking: {}", rel.blocking.join(", ")));
    }
    if !rel.children.is_empty() {
        lines.push(format!("Children: {}", rel.children.join(", ")));
    }
    if !rel.links.is_empty() {
        lines.push(format!("Links: {}", rel.links.join(", ")));
    }
    (!lines.is_empty()).then(|| lines.join("\n") + "\n")
}

fn matches_filters(t: &Ticket, status: Option<&str>, filters: &ListFilters) -> bool {
    if let Some(status) = status
        && t.status.to_string() != status
    {
        return false;
    }
    if let Some(assignee) = &filters.assignee
        && t.assignee.as_deref() != Some(assignee.as_str())
    {
        return false;
    }
    if let Some(tag) = &filters.tag
        && !t.tags.iter().any(|tg| tg.eq_ignore_ascii_case(tag))
    {
        return false;
    }
    if let Some(ticket_type) = &filters.ticket_type
        && t.ticket_type.map(|tt| tt.to_string()) != Some(ticket_type.clone())
    {
        return false;
    }
    true
}

fn sort_tickets(tickets: &mut [Ticket], sort: Option<&str>, reverse: bool) {
    let sort_by = sort.unwrap_or("priority");
    tickets.sort_by(|a, b| {
        let ordering = match sort_by {
            "created" => a.created.cmp(&b.created),
            "status" => a.status.to_string().cmp(&b.status.to_string()),
            "title" => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            _ => a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)),
        };
        if reverse { ordering.reverse() } else { ordering }
    });
}

fn format_ticket_line(t: &Ticket) -> String {
    format!("{} [P{}][{}] - {}", t.id, t.priority, t.status, t.title)
}

fn print_tickets(json: bool, tickets: &[Ticket]) -> anyhow::Result<()> {
    if json {
        let values: Vec<_> = tickets.iter().map(Ticket::to_json).collect();
        println!("{}", serde_json::to_string_pretty(&values)?);
        return Ok(());
    }
    for t in tickets {
        println!("{}", format_ticket_line(t));
    }
    Ok(())
}

fn list(
    backend: &dyn Backend,
    json: bool,
    status: Option<String>,
    filters: ListFilters,
) -> anyhow::Result<()> {
    let mut tickets: Vec<Ticket> = all_tickets(backend)?
        .into_iter()
        .filter(|t| matches_filters(t, status.as_deref(), &filters))
        .collect();
    sort_tickets(&mut tickets, filters.sort.as_deref(), filters.reverse);
    print_tickets(json, &tickets)
}

fn open_ids(tickets: &[Ticket]) -> std::collections::HashSet<String> {
    tickets
        .iter()
        .filter(|t| t.status != Status::Closed)
        .map(|t| t.id.clone())
        .collect()
}

fn filter_by_dependency_status(
    tickets: Vec<Ticket>,
    want_blocked: bool,
    filters: &ListFilters,
) -> Vec<Ticket> {
    let open = open_ids(&tickets);
    tickets
        .into_iter()
        .filter(|t| {
            if t.status == Status::Closed {
                return false;
            }
            let has_blocking = t.deps.iter().any(|d| open.contains(d));
            has_blocking == want_blocked && matches_filters(t, None, filters)
        })
        .collect()
}

fn ready(backend: &dyn Backend, json: bool, filters: ListFilters) -> anyhow::Result<()> {
    let tickets = all_tickets(backend)?;
    let mut result = filter_by_dependency_status(tickets, false, &filters);
    sort_tickets(&mut result, filters.sort.as_deref(), filters.reverse);
    print_tickets(json, &result)
}

fn blocked(backend: &dyn Backend, json: bool, filters: ListFilters) -> anyhow::Result<()> {
    let tickets = all_tickets(backend)?;
    let mut result = filter_by_dependency_status(tickets, true, &filters);
    sort_tickets(&mut result, filters.sort.as_deref(), filters.reverse);
    print_tickets(json, &result)
}

fn closed(
    backend: &dyn Backend,
    json: bool,
    limit: usize,
    filters: ListFilters,
) -> anyhow::Result<()> {
    let mut tickets: Vec<Ticket> = all_tickets(backend)?
        .into_iter()
        .filter(|t| t.status == Status::Closed && matches_filters(t, None, &filters))
        .collect();

    if filters.sort.is_some() {
        sort_tickets(&mut tickets, filters.sort.as_deref(), filters.reverse);
    } else {
        sort_tickets(&mut tickets, Some("created"), true);
    }

    if limit > 0 && tickets.len() > limit {
        tickets.truncate(limit);
    }

    print_tickets(json, &tickets)
}

fn set_status(backend: &dyn Backend, json: bool, id: &str, status: Status) -> anyhow::Result<()> {
    let number = resolve(backend, id)?;
    let mut ticket = read_ticket(backend, number)?;
    ticket.status = status;
    persist(backend, &ticket)?;

    if json {
        print_ticket(json, &ticket)
    } else {
        println!("Updated {} -> {}", ticket.id, ticket.status);
        Ok(())
    }
}

fn edit(backend: &dyn Backend, id: &str) -> anyhow::Result<()> {
    let number = resolve(backend, id)?;
    let ticket = read_ticket(backend, number)?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let path = std::env::temp_dir().join(format!("tko-{number}.md"));
    std::fs::write(&path, ticket.render()?)?;

    let status = std::process::Command::new(&editor).arg(&path).status()?;
    if !status.success() {
        anyhow::bail!("editor failed");
    }

    let mut contents = Vec::new();
    std::fs::File::open(&path)?.read_to_end(&mut contents)?;
    let _ = std::fs::remove_file(&path);

    let edited = Ticket::parse(&contents)?;
    persist(backend, &edited)?;
    Ok(())
}

fn add_note(backend: &dyn Backend, json: bool, id: &str, text: Option<String>) -> anyhow::Result<()> {
    let number = resolve(backend, id)?;
    let mut ticket = read_ticket(backend, number)?;

    let text = match text {
        Some(text) => text,
        None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf.trim_end().to_string()
        }
    };
    if text.is_empty() {
        anyhow::bail!("no note text provided");
    }

    ticket.notes.push(Note {
        timestamp: OffsetDateTime::now_utc(),
        content: text,
    });
    persist(backend, &ticket)?;

    if json {
        print_ticket(json, &ticket)
    } else {
        println!("Added note to {}", ticket.id);
        Ok(())
    }
}

fn check_cycle(tickets: &[Ticket], ticket_id: &str, dep_id: &str) -> anyhow::Result<()> {
    use std::collections::HashMap;
    let mut deps: HashMap<&str, Vec<&str>> = HashMap::new();
    for t in tickets {
        deps.insert(&t.id, t.deps.iter().map(String::as_str).collect());
    }
    deps.entry(ticket_id).or_default().push(dep_id);

    fn has_cycle<'a>(
        deps: &HashMap<&'a str, Vec<&'a str>>,
        visited: &mut std::collections::HashSet<&'a str>,
        current: &'a str,
        target: &'a str,
    ) -> bool {
        if current == target {
            return true;
        }
        if !visited.insert(current) {
            return false;
        }
        deps.get(current)
            .into_iter()
            .flatten()
            .any(|dep| has_cycle(deps, visited, dep, target))
    }

    let mut visited = std::collections::HashSet::new();
    if has_cycle(&deps, &mut visited, dep_id, ticket_id) {
        anyhow::bail!("adding dependency would create a cycle: {ticket_id} -> {dep_id}");
    }
    Ok(())
}

fn dep_add(backend: &dyn Backend, json: bool, id: &str, dep_id: &str) -> anyhow::Result<()> {
    let number = resolve(backend, id)?;
    let dep_number = resolve(backend, dep_id)?;
    let mut ticket = read_ticket(backend, number)?;
    let dep_id_str = dep_number.to_string();

    if ticket.id == dep_id_str {
        anyhow::bail!("ticket cannot depend on itself");
    }
    if ticket.deps.contains(&dep_id_str) {
        anyhow::bail!("dependency {dep_id_str} already exists");
    }

    let all = all_tickets(backend)?;
    check_cycle(&all, &ticket.id, &dep_id_str)?;

    ticket.deps.push(dep_id_str.clone());
    persist(backend, &ticket)?;
    let _ = backend.add_dependency(number, dep_number);

    if json {
        print_ticket(json, &ticket)
    } else {
        println!("Added dependency: {} -> {}", ticket.id, dep_id_str);
        Ok(())
    }
}

fn dep_remove(backend: &dyn Backend, json: bool, id: &str, dep_id: &str) -> anyhow::Result<()> {
    let number = resolve(backend, id)?;
    let dep_number = resolve(backend, dep_id)?;
    let mut ticket = read_ticket(backend, number)?;
    let dep_id_str = dep_number.to_string();

    let before = ticket.deps.len();
    ticket.deps.retain(|d| d != &dep_id_str);
    if ticket.deps.len() == before {
        anyhow::bail!("dependency {dep_id_str} not found on {}", ticket.id);
    }
    persist(backend, &ticket)?;
    let _ = backend.remove_dependency(number, dep_number);

    if json {
        print_ticket(json, &ticket)
    } else {
        println!("Removed dependency: {} -> {}", ticket.id, dep_id_str);
        Ok(())
    }
}

fn status_symbol(status: Status) -> &'static str {
    match status {
        Status::Open => "[ ]",
        Status::InProgress => "[~]",
        Status::Closed => "[x]",
    }
}

fn build_dep_tree_string(
    ticket: &Ticket,
    by_id: &std::collections::HashMap<&str, &Ticket>,
    prefix: &str,
    is_last: bool,
) -> String {
    let mut out = String::new();
    let connector = if is_last { "└── " } else { "├── " };
    let node = format!(
        "{} {} - {}",
        status_symbol(ticket.status),
        ticket.id,
        ticket.title
    );
    if prefix.is_empty() {
        out.push_str(&node);
        out.push('\n');
    } else {
        out.push_str(prefix);
        out.push_str(connector);
        out.push_str(&node);
        out.push('\n');
    }

    let child_prefix = if prefix.is_empty() {
        String::new()
    } else if is_last {
        format!("{prefix}    ")
    } else {
        format!("{prefix}│   ")
    };

    for (i, dep_id) in ticket.deps.iter().enumerate() {
        let last = i == ticket.deps.len() - 1;
        match by_id.get(dep_id.as_str()) {
            Some(dep) => out.push_str(&build_dep_tree_string(dep, by_id, &child_prefix, last)),
            None => {
                let connector = if last { "└── " } else { "├── " };
                out.push_str(&child_prefix);
                out.push_str(connector);
                out.push_str(&format!("[?] {dep_id} - (not found)\n"));
            }
        }
    }

    out
}

fn find_root_tickets(tickets: &[Ticket]) -> Vec<&Ticket> {
    let mut is_dep = std::collections::HashSet::new();
    for t in tickets {
        for d in &t.deps {
            is_dep.insert(d.as_str());
        }
    }
    tickets
        .iter()
        .filter(|t| !is_dep.contains(t.id.as_str()) && t.status != Status::Closed)
        .collect()
}

fn dep_tree(backend: &dyn Backend, id: Option<String>, full: bool) -> anyhow::Result<()> {
    let tickets = all_tickets(backend)?;
    let by_id: std::collections::HashMap<&str, &Ticket> =
        tickets.iter().map(|t| (t.id.as_str(), t)).collect();

    if full || id.is_none() {
        for root in find_root_tickets(&tickets) {
            print!("{}", build_dep_tree_string(root, &by_id, "", true));
        }
        return Ok(());
    }

    let number = resolve(backend, &id.unwrap())?;
    let number_str = number.to_string();
    let ticket = by_id
        .get(number_str.as_str())
        .ok_or_else(|| anyhow::anyhow!("ticket not found: {number_str}"))?;
    print!("{}", build_dep_tree_string(ticket, &by_id, "", true));
    Ok(())
}

fn detect_cycles(tickets: &[Ticket]) -> Vec<Vec<String>> {
    use std::collections::HashMap;
    let by_id: HashMap<&str, &Ticket> = tickets.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut cycles = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut rec_stack = std::collections::HashSet::new();
    let mut path: Vec<String> = Vec::new();

    fn dfs<'a>(
        id: &'a str,
        by_id: &HashMap<&'a str, &'a Ticket>,
        visited: &mut std::collections::HashSet<&'a str>,
        rec_stack: &mut std::collections::HashSet<&'a str>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(id);
        rec_stack.insert(id);
        path.push(id.to_string());

        if let Some(t) = by_id.get(id) {
            for dep in &t.deps {
                if !visited.contains(dep.as_str()) {
                    dfs(dep, by_id, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(dep.as_str())
                    && let Some(start) = path.iter().position(|p| p == dep)
                {
                    cycles.push(path[start..].to_vec());
                }
            }
        }

        path.pop();
        rec_stack.remove(id);
    }

    for t in tickets {
        if !visited.contains(t.id.as_str()) {
            dfs(&t.id, &by_id, &mut visited, &mut rec_stack, &mut path, &mut cycles);
        }
    }

    cycles
}

fn dep_check(backend: &dyn Backend, json: bool) -> anyhow::Result<()> {
    let tickets = all_tickets(backend)?;
    let cycles = detect_cycles(&tickets);

    if json {
        let value = serde_json::json!({ "cycles": cycles, "count": cycles.len() });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    if cycles.is_empty() {
        println!("No cycles detected");
        return Ok(());
    }

    println!("Found {} cycle(s):", cycles.len());
    for (i, cycle) in cycles.iter().enumerate() {
        println!("  {}: {}", i + 1, cycle.join(" -> "));
    }
    anyhow::bail!("dependency cycles detected")
}

fn link(backend: &dyn Backend, json: bool, ids: Vec<String>) -> anyhow::Result<()> {
    let numbers = ids
        .iter()
        .map(|id| resolve(backend, id))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let id_strs: Vec<String> = numbers.iter().map(u64::to_string).collect();

    let mut seen = std::collections::HashSet::new();
    for id in &id_strs {
        if !seen.insert(id) {
            anyhow::bail!("duplicate ticket ID: {id}");
        }
    }

    let mut linked = Vec::new();
    for (number, id) in numbers.iter().zip(&id_strs) {
        let mut ticket = read_ticket(backend, *number)?;
        for other in &id_strs {
            if other != id && !ticket.links.contains(other) {
                ticket.links.push(other.clone());
            }
        }
        persist(backend, &ticket)?;
        linked.push(ticket);
    }

    if json {
        let values: Vec<_> = linked.iter().map(Ticket::to_json).collect();
        println!("{}", serde_json::to_string_pretty(&values)?);
    } else {
        println!("Linked: {id_strs:?}");
    }
    Ok(())
}

fn unlink(backend: &dyn Backend, json: bool, id: &str, target_id: &str) -> anyhow::Result<()> {
    let number1 = resolve(backend, id)?;
    let number2 = resolve(backend, target_id)?;
    let id1 = number1.to_string();
    let id2 = number2.to_string();

    let mut ticket1 = read_ticket(backend, number1)?;
    let before1 = ticket1.links.len();
    ticket1.links.retain(|l| l != &id2);
    let found1 = ticket1.links.len() != before1;

    let mut ticket2 = read_ticket(backend, number2)?;
    let before2 = ticket2.links.len();
    ticket2.links.retain(|l| l != &id1);
    let found2 = ticket2.links.len() != before2;

    if !found1 && !found2 {
        anyhow::bail!("no link found between {id1} and {id2}");
    }

    persist(backend, &ticket1)?;
    persist(backend, &ticket2)?;

    if json {
        let values = vec![ticket1.to_json(), ticket2.to_json()];
        println!("{}", serde_json::to_string_pretty(&values)?);
    } else {
        println!("Unlinked: {id1} and {id2}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Comment, Issue};
    use crate::error::{Error, Result as TkResult};
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// An in-memory [`Backend`] for exercising command logic without a network.
    #[derive(Default)]
    struct MockBackend {
        issues: RefCell<HashMap<u64, Issue>>,
        next: RefCell<u64>,
    }

    impl MockBackend {
        fn seed(&self, title: &str, body: &str) -> u64 {
            let mut next = self.next.borrow_mut();
            *next += 1;
            let number = *next;
            self.issues.borrow_mut().insert(
                number,
                Issue {
                    number,
                    title: title.to_string(),
                    body: body.to_string(),
                    state: IssueState::Open,
                    labels: Vec::new(),
                    assignee: None,
                },
            );
            number
        }
    }

    impl Backend for MockBackend {
        fn create_issue(&self, new: NewIssue) -> TkResult<Issue> {
            let mut next = self.next.borrow_mut();
            *next += 1;
            let number = *next;
            let issue = Issue {
                number,
                title: new.title,
                body: new.body,
                state: IssueState::Open,
                labels: new.labels,
                assignee: new.assignee,
            };
            self.issues.borrow_mut().insert(number, issue.clone());
            Ok(issue)
        }

        fn get_issue(&self, number: u64) -> TkResult<Issue> {
            self.issues
                .borrow()
                .get(&number)
                .cloned()
                .ok_or_else(|| Error::TicketNotFound(number.to_string()))
        }

        fn list_issues(&self) -> TkResult<Vec<Issue>> {
            let mut issues: Vec<_> = self.issues.borrow().values().cloned().collect();
            issues.sort_by_key(|i| i.number);
            Ok(issues)
        }

        fn update_issue(&self, number: u64, update: IssueUpdate) -> TkResult<Issue> {
            let mut issues = self.issues.borrow_mut();
            let issue = issues
                .get_mut(&number)
                .ok_or_else(|| Error::TicketNotFound(number.to_string()))?;
            if let Some(title) = update.title {
                issue.title = title;
            }
            if let Some(body) = update.body {
                issue.body = body;
            }
            if let Some(labels) = update.labels {
                issue.labels = labels;
            }
            if let Some(assignee) = update.assignee {
                issue.assignee = assignee;
            }
            Ok(issue.clone())
        }

        fn set_state(&self, number: u64, state: IssueState) -> TkResult<()> {
            let mut issues = self.issues.borrow_mut();
            let issue = issues
                .get_mut(&number)
                .ok_or_else(|| Error::TicketNotFound(number.to_string()))?;
            issue.state = state;
            Ok(())
        }

        fn add_comment(&self, _number: u64, _body: &str) -> TkResult<Comment> {
            unimplemented!("not exercised by these tests")
        }

        fn list_comments(&self, _number: u64) -> TkResult<Vec<Comment>> {
            Ok(Vec::new())
        }

        fn add_dependency(&self, _number: u64, _depends_on: u64) -> TkResult<()> {
            Ok(())
        }

        fn remove_dependency(&self, _number: u64, _depends_on: u64) -> TkResult<()> {
            Ok(())
        }

        fn list_dependencies(&self, _number: u64) -> TkResult<Vec<u64>> {
            Ok(Vec::new())
        }
    }

    fn blank_ticket_body(status: &str) -> String {
        format!(
            "---\nid: 0\nstatus: {status}\ntype: task\npriority: 2\ncreated: 2026-01-01T00:00:00Z\n---\n# Title\n\nDescription.\n"
        )
    }

    #[test]
    fn create_assigns_backend_issue_number() {
        let backend = MockBackend::default();
        create(
            &backend,
            false,
            Some("My ticket".to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Vec::new(),
        )
        .unwrap();

        let issues = backend.list_issues().unwrap();
        assert_eq!(issues.len(), 1);
        let ticket = Ticket::parse(issues[0].body.as_bytes()).unwrap();
        assert_eq!(ticket.id, issues[0].number.to_string());
        assert_eq!(ticket.title, "My ticket");
    }

    #[test]
    fn set_status_persists_and_mirrors_state() {
        let backend = MockBackend::default();
        let number = backend.seed("T", &blank_ticket_body("open"));
        // fix up id to match issue number
        let mut t = Ticket::parse(backend.get_issue(number).unwrap().body.as_bytes()).unwrap();
        t.id = number.to_string();
        persist(&backend, &t).unwrap();

        set_status(&backend, false, &number.to_string(), Status::Closed).unwrap();

        let issue = backend.get_issue(number).unwrap();
        assert_eq!(issue.state, IssueState::Closed);
        let ticket = Ticket::parse(issue.body.as_bytes()).unwrap();
        assert_eq!(ticket.status, Status::Closed);
    }

    #[test]
    fn dep_add_rejects_self_and_cycles() {
        let backend = MockBackend::default();
        let n1 = backend.seed("A", &blank_ticket_body("open"));
        let mut t1 = Ticket::parse(backend.get_issue(n1).unwrap().body.as_bytes()).unwrap();
        t1.id = n1.to_string();
        persist(&backend, &t1).unwrap();

        let n2 = backend.seed("B", &blank_ticket_body("open"));
        let mut t2 = Ticket::parse(backend.get_issue(n2).unwrap().body.as_bytes()).unwrap();
        t2.id = n2.to_string();
        persist(&backend, &t2).unwrap();

        dep_add(&backend, false, &n1.to_string(), &n2.to_string()).unwrap();
        let err = dep_add(&backend, false, &n2.to_string(), &n1.to_string()).unwrap_err();
        assert!(err.to_string().contains("cycle"));
    }

    #[test]
    fn resolve_finds_unique_prefix() {
        let backend = MockBackend::default();
        backend.seed("A", &blank_ticket_body("open"));
        let n = resolve(&backend, "1").unwrap();
        assert_eq!(n, 1);
    }
}
