use std::path::Path;

use anyhow::{Result, bail};

use crate::models::CustomEntryPayload;

pub fn validate_custom_payload(payload: &CustomEntryPayload) -> Result<()> {
    if payload.label.trim().is_empty() {
        bail!("Label is required");
    }

    if payload.executable_path.trim().is_empty() {
        bail!("Executable path is required");
    }

    if !Path::new(&payload.executable_path).exists() {
        bail!("Executable does not exist: {}", payload.executable_path);
    }

    if payload.extensions.is_empty() {
        bail!("At least one extension is required");
    }

    if !payload.args.contains("%1") {
        bail!("Arguments must include %1 placeholder for selected file path");
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
    use super::{normalize_extension, sanitize_verb};

    #[test]
    fn normalizes_extensions() {
        assert_eq!(normalize_extension("MP4"), ".mp4");
        assert_eq!(normalize_extension(".MKV"), ".mkv");
    }

    #[test]
    fn sanitizes_verb() {
        assert_eq!(sanitize_verb("Lossless Cut"), "lossless_cut");
        assert_eq!(sanitize_verb("***"), "custom_action");
    }
}
