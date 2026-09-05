/// Apple Notes integration via `osascript`.
///
/// Two folders are managed in the default Notes account:
///   "Trace"            — the user drops notes here to be captured
///   "Trace — Captured" — notes are moved here after import
///
/// All functions are synchronous (osascript itself is blocking) and are
/// meant to be called from inside `tauri::async_runtime::spawn` tasks via
/// `tokio::task::spawn_blocking`.

pub const INBOX_FOLDER: &str = "Trace";
pub const CAPTURED_FOLDER: &str = "Trace \u{2014} Captured"; // em-dash

#[derive(Debug, Clone)]
pub struct AppleNote {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// Run an AppleScript and return trimmed stdout, or an error string.
fn run_osascript(script: &str) -> Result<String, String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| format!("osascript launch failed: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Create "Trace" and "Trace — Captured" folders in Apple Notes if they
/// don't already exist. Safe to call repeatedly.
pub fn ensure_folders() -> Result<(), String> {
    let script = format!(
        r#"tell application "Notes"
  if not (exists folder "{inbox}") then
    make new folder with properties {{name:"{inbox}"}}
  end if
  if not (exists folder "{captured}") then
    make new folder with properties {{name:"{captured}"}}
  end if
end tell"#,
        inbox = INBOX_FOLDER,
        captured = CAPTURED_FOLDER,
    );
    run_osascript(&script)?;
    Ok(())
}

/// Return all notes currently sitting in the "Trace" inbox folder.
/// Returns an empty list if the folder doesn't exist yet.
pub fn list_inbox_notes() -> Result<Vec<AppleNote>, String> {
    // We use a multi-character delimiter that is very unlikely to appear in
    // note content so we can safely split the osascript output.
    let sep = "\u{241E}"; // ␞  (ASCII record separator rendered as Unicode)
    let end = "\u{241F}"; // ␟  (ASCII unit separator)

    let script = format!(
        r#"tell application "Notes"
  set output to ""
  if not (exists folder "{inbox}") then
    return output
  end if
  repeat with n in (notes of folder "{inbox}")
    set output to output & "{sep}" & (id of n) & "{sep}" & (name of n) & "{sep}" & (plaintext of n) & "{end}"
  end repeat
  return output
end tell"#,
        inbox = INBOX_FOLDER,
        sep = sep,
        end = end,
    );

    let raw = run_osascript(&script)?;
    if raw.is_empty() {
        return Ok(vec![]);
    }

    let mut notes = Vec::new();
    for chunk in raw.split(end) {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        // Each chunk looks like: <sep>id<sep>title<sep>body
        let parts: Vec<&str> = chunk.splitn(4, sep).collect();
        // parts[0] is empty (before first sep), [1]=id, [2]=title, [3]=body
        if parts.len() < 4 {
            continue;
        }
        let id = parts[1].trim().to_string();
        let title = parts[2].trim().to_string();
        let body = parts[3].trim().to_string();
        if id.is_empty() {
            continue;
        }
        notes.push(AppleNote { id, title, body });
    }
    Ok(notes)
}

/// Move a note (identified by its Core Data URI id) to the "Trace — Captured"
/// folder. Logs but does not propagate errors so one bad move doesn't abort
/// the whole sync.
pub fn move_to_captured(note_id: &str) -> Result<(), String> {
    // Sanitise: note ids from Apple Notes look like
    // "x-coredata://UUID/ICNote/p123" — safe characters, but be defensive.
    let safe_id = note_id.replace('"', "");
    let script = format!(
        r#"tell application "Notes"
  set targetNote to first note whose id is "{id}"
  move targetNote to folder "{captured}"
end tell"#,
        id = safe_id,
        captured = CAPTURED_FOLDER,
    );
    run_osascript(&script).map(|_| ())
}
