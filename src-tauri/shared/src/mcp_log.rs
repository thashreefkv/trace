use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use crate::repo::now_utc;

const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024;

pub fn append_tool_log(
    app_support_dir: &Path,
    tool_name: &str,
    ok: bool,
    fields: &[(&str, String)],
) -> Result<(), String> {
    fs::create_dir_all(app_support_dir)
        .map_err(|error| format!("failed to create MCP log directory: {error}"))?;
    let path = log_path(app_support_dir);
    rotate_if_needed(&path)?;

    let outcome = if ok { "ok" } else { "err" };
    let mut line = format!("{}  {}  {}", now_utc(), tool_name, outcome);
    for (key, value) in fields {
        line.push_str("  ");
        line.push_str(key);
        line.push('=');
        line.push_str(&quote_value(value));
    }
    line.push('\n');

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("failed to open MCP log: {error}"))?;
    file.write_all(line.as_bytes())
        .map_err(|error| format!("failed to write MCP log: {error}"))
}

pub fn log_path(app_support_dir: &Path) -> PathBuf {
    app_support_dir.join("mcp.log")
}

fn rotate_if_needed(path: &Path) -> Result<(), String> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };

    if metadata.len() < MAX_LOG_BYTES {
        return Ok(());
    }

    let first_rotation = path.with_extension("log.1");
    let second_rotation = path.with_extension("log.2");

    if second_rotation.exists() {
        fs::remove_file(&second_rotation)
            .map_err(|error| format!("failed to remove old MCP log rotation: {error}"))?;
    }

    if first_rotation.exists() {
        fs::rename(&first_rotation, &second_rotation)
            .map_err(|error| format!("failed to rotate MCP log: {error}"))?;
    }

    fs::rename(path, first_rotation).map_err(|error| format!("failed to rotate MCP log: {error}"))
}

fn quote_value(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_writes_plain_text_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        append_tool_log(
            dir.path(),
            "search_deliverables",
            true,
            &[
                ("query", "quality".to_string()),
                ("results", "3".to_string()),
            ],
        )
        .expect("log write");

        let log = fs::read_to_string(log_path(dir.path())).expect("log read");
        assert!(log.contains("search_deliverables  ok"));
        assert!(log.contains("query=\"quality\""));
        assert!(log.contains("results=\"3\""));
    }

    #[test]
    fn log_rotates_at_size_limit() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = log_path(dir.path());
        fs::File::create(&path)
            .expect("create log")
            .set_len(MAX_LOG_BYTES)
            .expect("size log");

        append_tool_log(
            dir.path(),
            "create_deliverable",
            false,
            &[("error", "bad".to_string())],
        )
        .expect("log write");

        assert!(path.with_extension("log.1").exists());
        let log = fs::read_to_string(path).expect("log read");
        assert!(log.contains("create_deliverable  err"));
    }
}
