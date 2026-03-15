use std::path::Path;

use anyhow::{Result, bail};

use crate::models::{ActionTarget, CreateActionRequest};

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

    if let Some(icon) = &payload.icon_path {
        if !icon.trim().is_empty() && !Path::new(icon).exists() {
            bail!("Icon path does not exist: {icon}");
        }
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

#[cfg(test)]
mod tests {
    use super::{
        looks_like_file_path, normalize_extension, sanitize_verb, validate_create_action_request,
    };
    use crate::models::{ActionTarget, CreateActionRequest, CustomEntryScope};

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
}
