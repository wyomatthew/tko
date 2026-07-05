//! `tko agents-md` — bootstrap or append a `tko` agent workflow section to `AGENTS.md`.
//!
//! The command never touches an existing section it did not write itself; it
//! looks for its own marker heading and refuses to duplicate it unless
//! `--force` is passed.

use std::path::Path;

const SECTION_HEADING: &str = "## Agent workflow (tko)";

const SECTION_BODY: &str = r#"## Agent workflow (tko)

This project uses [`tko`](https://github.com/) for ticket management. Tickets
live on the configured git-service issue tracker, not as local files; `tko`
is the only supported way to read or change them. Run `tko --help` for the
full command reference.

Follow this workflow whenever you are given a coding task in this repo:

### 1. Decide whether to use `tko`

- If the user's request names a ticket directly (e.g. "fix ticket 42",
  "work on #42"), use `tko` for that ticket.
- Otherwise, briefly scan the open tickets to see if any already describe the
  request:

  ```bash
  tko ready          # open/in_progress tickets with no unresolved deps
  tko list           # all open tickets
  ```

  If one clearly matches, use it. If nothing applies, resolve the request
  normally without involving `tko` — don't force-fit unrelated work into a
  ticket.

### 2. Read and understand the ticket before writing any code

Load full context, not just the matched ticket:

```bash
tko show 42                # the ticket itself
tko dep tree 42             # its dependency tree (upstream/downstream)
tko show <parent-id>        # parent ticket, if any
tko show <dep-id>           # each blocking dependency
tko show <linked-id>        # each linked ticket
```

Read the description, design notes, and acceptance criteria. You must fully
understand the task and have no unresolved ambiguity before starting
implementation. If something is genuinely unclear — conflicting
requirements, missing acceptance criteria, an approach that isn't specified —
stop and ask the user rather than guessing.

### 3. Mark the ticket in progress before starting work

```bash
tko start 42
```

If you discover additional relevant metadata while getting oriented (a
missing assignee, a dependency that should be recorded, a tag), update it now:

```bash
tko dep add 42 17           # 42 is now blocked by 17
tko edit 42                 # open the ticket in $EDITOR for larger edits
```

### 4. Complete the task

Implement the work described by the ticket, using its description, design,
and acceptance criteria as your spec. If you need to change approach or
discover new information mid-implementation, don't silently diverge — record
it (see step 5).

### 5. Close out the ticket

Once the work is complete:

- If the final implementation differed from the ticket's original
  description or design, record why:

  ```bash
  tko add-note 42 "Implemented via X instead of Y because Z."
  tko edit 42                  # for larger rewrites of description/design
  ```

- Mark the ticket's status to reflect reality:

  ```bash
  tko close 42                 # work is complete
  ```

  If the task is only partially done, or is blocked, leave it `in_progress`
  or record the blocker as a note/dependency instead of closing it.

Never close a ticket whose acceptance criteria are not actually met.
"#;

/// Runs `tko agents-md` against the current directory.
pub fn run(force: bool) -> anyhow::Result<()> {
    let path = Path::new("AGENTS.md");

    let existing = if path.exists() { std::fs::read_to_string(path)? } else { String::new() };

    if existing.contains(SECTION_HEADING) && !force {
        anyhow::bail!(
            "{} already has a '{}' section; pass --force to append another copy",
            path.display(),
            SECTION_HEADING
        );
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    if !updated.is_empty() {
        updated.push('\n');
    }
    updated.push_str(SECTION_BODY);

    let created = !path.exists();
    std::fs::write(path, updated)?;

    if created {
        println!("Created {} with a '{}' section", path.display(), SECTION_HEADING);
    } else {
        println!("Appended '{}' section to {}", SECTION_HEADING, path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // `std::env::set_current_dir` is process-global, so tests that use it must
    // not run concurrently with each other.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn in_temp_dir<F: FnOnce()>(f: F) {
        let _guard = CWD_LOCK.lock().unwrap();
        let dir = tempfile_dir();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        f();
        std::env::set_current_dir(prev).unwrap();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("tko-agents-md-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn creates_agents_md_when_absent() {
        in_temp_dir(|| {
            assert!(!Path::new("AGENTS.md").exists());
            run(false).unwrap();
            let contents = std::fs::read_to_string("AGENTS.md").unwrap();
            assert!(contents.contains(SECTION_HEADING));
            std::fs::remove_file("AGENTS.md").unwrap();
        });
    }

    #[test]
    fn appends_to_existing_agents_md() {
        in_temp_dir(|| {
            std::fs::write("AGENTS.md", "# My project\n\nSome existing docs.\n").unwrap();
            run(false).unwrap();
            let contents = std::fs::read_to_string("AGENTS.md").unwrap();
            assert!(contents.contains("# My project"));
            assert!(contents.contains("Some existing docs."));
            assert!(contents.contains(SECTION_HEADING));
            std::fs::remove_file("AGENTS.md").unwrap();
        });
    }

    #[test]
    fn refuses_to_duplicate_section_without_force() {
        in_temp_dir(|| {
            run(false).unwrap();
            let err = run(false).unwrap_err();
            assert!(err.to_string().contains("--force"));
            std::fs::remove_file("AGENTS.md").unwrap();
        });
    }

    #[test]
    fn force_appends_a_second_copy() {
        in_temp_dir(|| {
            run(false).unwrap();
            run(true).unwrap();
            let contents = std::fs::read_to_string("AGENTS.md").unwrap();
            assert_eq!(contents.matches(SECTION_HEADING).count(), 2);
            std::fs::remove_file("AGENTS.md").unwrap();
        });
    }
}
