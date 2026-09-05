use std::path::{Path, PathBuf};

use rand::RngCore;

const KEYCHAIN_SERVICE: &str = "app.trace.credentials";
const GEMINI_ACCOUNT: &str = "gemini-api-key";
const GEMINI_LEGACY_FILE: &str = "gemini.key";
const SIRI_ACCOUNT: &str = "siri-bearer-token";
const SIRI_LEGACY_FILE: &str = "siri.token";

fn legacy_path(dir: &Path, legacy_file: &str) -> PathBuf {
    dir.join(legacy_file)
}

fn fallback_path(dir: &Path, account: &str) -> PathBuf {
    let safe_account: String = account
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    dir.join(format!(".{safe_account}.secret"))
}

/// Create a private data directory and restrict it to the current user on Unix.
pub fn harden_private_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("failed to create private data directory: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("failed to secure private data directory: {error}"))?;
    }

    Ok(())
}

fn read_legacy_secret(dir: &Path, legacy_file: &str) -> Result<Option<String>, String> {
    let path = legacy_path(dir, legacy_file);
    if !path.exists() {
        return Ok(None);
    }

    let value = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read legacy credential: {error}"))?;
    let value = value.trim().to_string();
    Ok((!value.is_empty()).then_some(value))
}

fn remove_legacy_secret(dir: &Path, legacy_file: &str) -> Result<(), String> {
    let path = legacy_path(dir, legacy_file);
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to remove legacy credential: {error}"))?;
    }
    Ok(())
}

#[cfg(all(target_os = "macos", not(test)))]
fn platform_save_secret(_dir: &Path, account: &str, value: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, account)
        .map_err(|error| format!("failed to open macOS Keychain: {error}"))?;
    entry
        .set_password(value)
        .map_err(|error| format!("failed to save credential in macOS Keychain: {error}"))
}

#[cfg(all(target_os = "macos", not(test)))]
fn platform_get_secret(_dir: &Path, account: &str) -> Result<Option<String>, String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, account)
        .map_err(|error| format!("failed to open macOS Keychain: {error}"))?;
    match entry.get_password() {
        Ok(value) => {
            let value = value.trim().to_string();
            Ok((!value.is_empty()).then_some(value))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!(
            "failed to read credential from macOS Keychain: {error}"
        )),
    }
}

#[cfg(all(target_os = "macos", not(test)))]
fn platform_clear_secret(_dir: &Path, account: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, account)
        .map_err(|error| format!("failed to open macOS Keychain: {error}"))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "failed to remove credential from macOS Keychain: {error}"
        )),
    }
}

// Tests and non-macOS development builds use an owner-only fallback. Trace is
// currently a macOS application, where production builds always use Keychain.
#[cfg(any(not(target_os = "macos"), test))]
fn platform_save_secret(dir: &Path, account: &str, value: &str) -> Result<(), String> {
    harden_private_dir(dir)?;
    let path = fallback_path(dir, account);
    std::fs::write(&path, value.as_bytes())
        .map_err(|error| format!("failed to save credential: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("failed to secure credential: {error}"))?;
    }
    Ok(())
}

#[cfg(any(not(target_os = "macos"), test))]
fn platform_get_secret(dir: &Path, account: &str) -> Result<Option<String>, String> {
    let path = fallback_path(dir, account);
    if !path.exists() {
        return Ok(None);
    }
    let value = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read credential: {error}"))?;
    let value = value.trim().to_string();
    Ok((!value.is_empty()).then_some(value))
}

#[cfg(any(not(target_os = "macos"), test))]
fn platform_clear_secret(dir: &Path, account: &str) -> Result<(), String> {
    let path = fallback_path(dir, account);
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to remove credential: {error}"))?;
    }
    Ok(())
}

pub fn save_secret(
    dir: &Path,
    account: &str,
    legacy_file: &str,
    value: &str,
) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("credential cannot be empty".to_string());
    }
    platform_save_secret(dir, account, value)?;
    remove_legacy_secret(dir, legacy_file)
}

/// Read a credential and migrate an older flat-file value on first access.
pub fn get_secret(dir: &Path, account: &str, legacy_file: &str) -> Result<Option<String>, String> {
    if let Some(value) = platform_get_secret(dir, account)? {
        return Ok(Some(value));
    }
    let Some(value) = read_legacy_secret(dir, legacy_file)? else {
        return Ok(None);
    };
    platform_save_secret(dir, account, &value)?;
    remove_legacy_secret(dir, legacy_file)?;
    Ok(Some(value))
}

pub fn clear_secret(dir: &Path, account: &str, legacy_file: &str) -> Result<(), String> {
    platform_clear_secret(dir, account)?;
    remove_legacy_secret(dir, legacy_file)
}

pub fn save_gemini_api_key(dir: &Path, key: &str) -> Result<(), String> {
    save_secret(dir, GEMINI_ACCOUNT, GEMINI_LEGACY_FILE, key)
}

pub fn get_gemini_api_key(dir: &Path) -> Result<Option<String>, String> {
    get_secret(dir, GEMINI_ACCOUNT, GEMINI_LEGACY_FILE)
}

pub fn clear_gemini_api_key(dir: &Path) -> Result<(), String> {
    clear_secret(dir, GEMINI_ACCOUNT, GEMINI_LEGACY_FILE)
}

pub fn gemini_api_key_configured(dir: &Path) -> Result<bool, String> {
    get_gemini_api_key(dir).map(|value| value.is_some())
}

pub fn get_or_create_siri_token(dir: &Path) -> Result<String, String> {
    if let Some(existing) = get_siri_token(dir)? {
        return Ok(existing);
    }
    regenerate_siri_token(dir)
}

pub fn get_siri_token(dir: &Path) -> Result<Option<String>, String> {
    get_secret(dir, SIRI_ACCOUNT, SIRI_LEGACY_FILE)
}

pub fn regenerate_siri_token(dir: &Path) -> Result<String, String> {
    let token = generate_token();
    save_secret(dir, SIRI_ACCOUNT, SIRI_LEGACY_FILE, &token)?;
    Ok(token)
}

fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn token_is_persistent_across_calls() {
        let dir = tempdir().expect("tempdir");
        let first = get_or_create_siri_token(dir.path()).expect("create");
        let second = get_or_create_siri_token(dir.path()).expect("read");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn regenerate_replaces_token() {
        let dir = tempdir().expect("tempdir");
        let first = get_or_create_siri_token(dir.path()).expect("create");
        let second = regenerate_siri_token(dir.path()).expect("regenerate");
        assert_ne!(first, second);
        assert_eq!(get_siri_token(dir.path()).expect("read"), Some(second));
    }

    #[test]
    fn migrates_and_removes_legacy_file() {
        let dir = tempdir().expect("tempdir");
        let legacy = dir.path().join("old-token");
        std::fs::write(&legacy, "legacy-value\n").expect("write legacy");

        let value = get_secret(dir.path(), "test-token", "old-token").expect("migrate");
        assert_eq!(value.as_deref(), Some("legacy-value"));
        assert!(!legacy.exists());
        assert_eq!(
            get_secret(dir.path(), "test-token", "old-token").expect("read migrated"),
            value
        );
    }

    #[cfg(unix)]
    #[test]
    fn fallback_credential_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().expect("tempdir");
        save_secret(dir.path(), "mode-test", "legacy", "secret").expect("save");
        let mode = std::fs::metadata(fallback_path(dir.path(), "mode-test"))
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
