use std::path::Path;

use anyhow::{Result, bail};

use crate::models::{ActionTarget, ChangeKind, CreateActionRequest, ProposedChange};

pub fn validate_create_action_request(payload: &CreateActionRequest) -> Result<()> {
    if payload.label.trim().is_empty() {
        bail!("Label is required");
    }

    let executable = payload.executable_path.trim();
    if executable.is_empty() {
        bail!("Command is required");
    }

    if looks_like_file_path(executable) && !Path::new(executable).exists() {
        bail!("Command file does not exist: {executable}");
    }

    if payload.targets.is_empty() {
        bail!("Select at least one target context");
    }

    if payload
        .targets
        .iter()
        .any(|target| matches!(target, ActionTarget::Files))
        && !payload.apply_to_all_files
        && payload.extensions.is_empty()
    {
        bail!("For file target, choose at least one extension or enable all files");
    }

    for ext in &payload.extensions {
        if !ext.starts_with('.') || ext.len() < 2 {
            bail!("Invalid extension format: {ext}. Expected .ext");
        }
    }

    if let Some(icon) = &payload.icon_path
        && !icon.trim().is_empty() && !Path::new(icon).exists() {
            bail!("Icon path does not exist: {icon}");
        }

    Ok(())
}

pub fn normalize_extension(input: &str) -> String {
    let mut value = input.trim().to_ascii_lowercase();
    if !value.starts_with('.') {
        value.insert(0, '.');
    }
    value
}

pub fn looks_like_file_path(input: &str) -> bool {
    let value = input.trim();
    if value.is_empty() {
        return false;
    }

    // Absolute/relative path cues. Plain aliases like "mytool" should not be treated as paths.
    value.starts_with("\\\\")
        || value.contains('\\')
        || value.contains('/')
        || value.as_bytes().get(1).is_some_and(|byte| *byte == b':')
}

pub fn sanitize_verb(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    for ch in label.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_ascii_whitespace() || ch == '-' || ch == '_') && !out.ends_with('_') {
            out.push('_');
        }
    }

    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "custom_action".to_string()
    } else {
        out
    }
}

pub fn validate_change_batch(changes: &[ProposedChange]) -> Result<()> {
    for change in changes {
        validate_change(change)?;
    }
    Ok(())
}

fn validate_change(change: &ProposedChange) -> Result<()> {
    match change.kind {
        ChangeKind::Add => {
            let after = change
                .after
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("add change requires an 'after' entry"))?;
            if let Some(before) = &change.before
                && !before.key_path.eq_ignore_ascii_case(&after.key_path)
            {
                bail!("add change contains mismatched before/after key paths");
            }
            ensure_allowed_key_path(&after.key_path)?;
        }
        ChangeKind::Enable | ChangeKind::Disable => {
            let before = change
                .before
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("toggle change requires a 'before' entry"))?;
            let after = change
                .after
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("toggle change requires an 'after' entry"))?;
            if !before.key_path.eq_ignore_ascii_case(&after.key_path) {
                bail!("toggle change contains mismatched before/after key paths");
            }
            ensure_allowed_key_path(&after.key_path)?;
        }
        ChangeKind::Remove => {
            let key_path = change
                .before
                .as_ref()
                .or(change.after.as_ref())
                .map(|entry| entry.key_path.as_str())
                .ok_or_else(|| anyhow::anyhow!("remove change requires a target entry"))?;
            if let (Some(before), Some(after)) = (&change.before, &change.after)
                && !before.key_path.eq_ignore_ascii_case(&after.key_path)
            {
                bail!("remove change contains mismatched before/after key paths");
            }
            ensure_allowed_key_path(key_path)?;
            ensure_remove_allowed_path(key_path)?;
        }
    }

    Ok(())
}

fn ensure_allowed_key_path(path: &str) -> Result<()> {
    if !is_allowed_key_path(path) {
        bail!("write outside approved shell paths is not allowed: {path}");
    }
    Ok(())
}

fn ensure_remove_allowed_path(path: &str) -> Result<()> {
    if !is_remove_allowed_path(path) {
        bail!("remove is only allowed for HKCU shell verb paths: {path}");
    }
    Ok(())
}

fn is_allowed_key_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();

    if let Some(rel) = lower.strip_prefix(r"hkcu\software\classes\") {
        return is_allowed_classes_relative(rel);
    }
    if let Some(rel) = lower.strip_prefix(r"hkcr\") {
        return is_allowed_classes_relative(rel);
    }

    false
}

fn is_allowed_classes_relative(rel: &str) -> bool {
    is_direct_shell_path(rel) || is_handler_path(rel)
}

fn is_direct_shell_path(rel: &str) -> bool {
    rel.starts_with(r"directory\background\shell\")
        || rel.starts_with(r"directory\shell\")
        || rel.starts_with(r"drive\shell\")
        || rel.starts_with(r"*\shell\")
        || rel.starts_with(r"allfilesystemobjects\shell\")
        || (rel.starts_with(r"systemfileassociations\") && rel.contains(r"\shell\"))
}

fn is_handler_path(rel: &str) -> bool {
    rel.starts_with(r"directory\background\shellex\contextmenuhandlers\")
        || rel.starts_with(r"directory\shellex\contextmenuhandlers\")
        || rel.starts_with(r"drive\shellex\contextmenuhandlers\")
        || rel.starts_with(r"*\shellex\contextmenuhandlers\")
        || rel.starts_with(r"allfilesystemobjects\shellex\contextmenuhandlers\")
        || (rel.starts_with(r"systemfileassociations\")
            && rel.contains(r"\shellex\contextmenuhandlers\"))
}

fn is_remove_allowed_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with(r"hkcu\software\classes\")
        && lower.contains(r"\shell\")
        && !lower.contains(r"\shellex\contextmenuhandlers\")
}

#[cfg(test)]
mod tests {
    use super::{
        looks_like_file_path, normalize_extension, sanitize_verb, validate_change_batch,
        validate_create_action_request,
    };
    use crate::models::{
        ActionTarget, ChangeKind, CreateActionRequest, CustomEntryScope, EntryScope, EntryState,
        MenuEntry, ProposedChange, RiskLevel,
    };

    #[test]
    fn normalizes_extensions() {
        assert_eq!(normalize_extension("MP4"), ".mp4");
        assert_eq!(normalize_extension(".MKV"), ".mkv");
    }

    #[test]
    fn sanitizes_verb() {
        assert_eq!(sanitize_verb("My Tool"), "my_tool");
        assert_eq!(sanitize_verb("***"), "custom_action");
    }

    #[test]
    fn detects_path_like_inputs() {
        assert!(looks_like_file_path(r"C:\Program Files\App\app.exe"));
        assert!(looks_like_file_path(r"\\server\share\tool.exe"));
        assert!(looks_like_file_path("./tool.exe"));
        assert!(!looks_like_file_path("mytool"));
        assert!(!looks_like_file_path("notepad.exe"));
    }

    fn base_request(command: &str) -> CreateActionRequest {
        CreateActionRequest {
            label: "Open with Tool".to_string(),
            executable_path: command.to_string(),
            args: "\"%1\"".to_string(),
            icon_path: None,
            targets: vec![ActionTarget::Files],
            extensions: vec![".mp4".to_string()],
            apply_to_all_files: false,
            verb: None,
            scope: CustomEntryScope::CurrentUser,
        }
    }

    #[test]
    fn accepts_alias_command_without_path() {
        let request = base_request("mytool");
        assert!(validate_create_action_request(&request).is_ok());
    }

    #[test]
    fn rejects_missing_path_like_command() {
        let request = base_request("./definitely_missing_tool.exe");
        assert!(validate_create_action_request(&request).is_err());
    }

    fn shell_entry(path: &str) -> MenuEntry {
        MenuEntry {
            id: path.to_ascii_lowercase(),
            label: "Label".to_string(),
            scope: EntryScope::CurrentUser,
            key_path: path.to_string(),
            icon: None,
            command: Some("tool.exe \"%1\"".to_string()),
            applies_to: vec!["file".to_string()],
            state: EntryState::Enabled,
        }
    }

    #[test]
    fn accepts_valid_hkcu_shell_changes() {
        let before = shell_entry(r"HKCU\Software\Classes\Directory\shell\tool");
        let mut after = before.clone();
        after.state = EntryState::Disabled;
        let changes = vec![ProposedChange {
            id: "1".to_string(),
            kind: ChangeKind::Disable,
            before: Some(before),
            after: Some(after),
            risk_level: RiskLevel::Low,
            reason: "test".to_string(),
        }];

        assert!(validate_change_batch(&changes).is_ok());
    }

    #[test]
    fn rejects_hklm_changes() {
        let after = shell_entry(r"HKLM\Software\Classes\Directory\shell\tool");
        let changes = vec![ProposedChange {
            id: "1".to_string(),
            kind: ChangeKind::Add,
            before: None,
            after: Some(after),
            risk_level: RiskLevel::Medium,
            reason: "test".to_string(),
        }];

        assert!(validate_change_batch(&changes).is_err());
    }

    #[test]
    fn restricts_remove_to_hkcu_shell_paths() {
        let allowed = vec![ProposedChange {
            id: "1".to_string(),
            kind: ChangeKind::Remove,
            before: Some(shell_entry(r"HKCU\Software\Classes\Directory\shell\tool")),
            after: None,
            risk_level: RiskLevel::Medium,
            reason: "test".to_string(),
        }];
        assert!(validate_change_batch(&allowed).is_ok());

        let disallowed = vec![ProposedChange {
            id: "2".to_string(),
            kind: ChangeKind::Remove,
            before: Some(shell_entry(
                r"HKCU\Software\Classes\Directory\shellex\ContextMenuHandlers\tool",
            )),
            after: None,
            risk_level: RiskLevel::Medium,
            reason: "test".to_string(),
        }];
        assert!(validate_change_batch(&disallowed).is_err());
    }
}
