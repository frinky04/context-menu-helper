use anyhow::{Result, anyhow};
use uuid::Uuid;

use crate::{
    models::{
        ChangeKind, CustomEntryPayload, EntryScope, EntryState, MenuEntry, ProposedChange,
        RiskLevel,
    },
    validation::{normalize_extension, sanitize_verb, validate_custom_payload},
};

const GIT_BASH_LABELS: [&str; 2] = ["open git bash here", "git bash here"];

pub fn suggest_disable_git_bash(entries: &[MenuEntry]) -> Vec<ProposedChange> {
    entries
        .iter()
        .filter(|entry| {
            entry.state == EntryState::Enabled
                && GIT_BASH_LABELS.contains(&entry.label.to_ascii_lowercase().as_str())
        })
        .map(|entry| build_toggle_change(entry, false, "Suggested: disable Git Bash shell entry"))
        .collect()
}

pub fn build_toggle_change(entry: &MenuEntry, enable: bool, reason: &str) -> ProposedChange {
    let mut next = entry.clone();
    next.state = if enable {
        EntryState::Enabled
    } else {
        EntryState::Disabled
    };

    ProposedChange {
        id: Uuid::new_v4().to_string(),
        kind: if enable {
            ChangeKind::Enable
        } else {
            ChangeKind::Disable
        },
        before: Some(entry.clone()),
        after: Some(next),
        risk_level: RiskLevel::Low,
        reason: reason.to_string(),
    }
}

pub fn build_custom_video_changes(payload: &CustomEntryPayload) -> Result<Vec<ProposedChange>> {
    validate_custom_payload(payload)?;

    let verb = payload
        .verb
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| sanitize_verb(&payload.label));

    let normalized_extensions: Vec<String> = payload
        .extensions
        .iter()
        .map(|ext| normalize_extension(ext))
        .collect();

    let mut changes = Vec::with_capacity(normalized_extensions.len());
    for ext in normalized_extensions {
        let key_path = format!(
            "HKCU\\Software\\Classes\\SystemFileAssociations\\{}\\shell\\{}",
            ext, verb
        );

        let after = MenuEntry {
            id: format!("{}:{}", key_path, payload.label),
            label: payload.label.clone(),
            scope: EntryScope::CurrentUser,
            key_path,
            command: Some(format!("\"{}\" {}", payload.executable_path, payload.args)),
            applies_to: vec![ext],
            state: EntryState::Enabled,
        };

        changes.push(ProposedChange {
            id: Uuid::new_v4().to_string(),
            kind: ChangeKind::Add,
            before: None,
            after: Some(after),
            risk_level: RiskLevel::Medium,
            reason: "Add custom video action".to_string(),
        });
    }

    if changes.is_empty() {
        return Err(anyhow!("No changes generated"));
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::suggest_disable_git_bash;
    use crate::models::{EntryScope, EntryState, MenuEntry};

    #[test]
    fn suggests_disabling_git_bash() {
        let entries = vec![MenuEntry {
            id: "1".to_string(),
            label: "Open Git Bash Here".to_string(),
            scope: EntryScope::CurrentUser,
            key_path: "HKCU\\Software\\Classes\\Directory\\Background\\shell\\git_shell"
                .to_string(),
            command: Some("git-bash.exe".to_string()),
            applies_to: vec!["directory_background".to_string()],
            state: EntryState::Enabled,
        }];

        let suggestions = suggest_disable_git_bash(&entries);
        assert_eq!(suggestions.len(), 1);
    }
}
