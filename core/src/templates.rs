use std::collections::BTreeSet;

use anyhow::{Result, anyhow};
use uuid::Uuid;

use crate::{
    models::{
        ActionTarget, ChangeKind, CreateActionRequest, EntryScope, EntryState, MenuEntry,
        ProposedChange, RiskLevel,
    },
    validation::{
        looks_like_file_path, normalize_extension, sanitize_verb, validate_create_action_request,
    },
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

pub fn build_create_action_changes(request: &CreateActionRequest) -> Result<Vec<ProposedChange>> {
    validate_create_action_request(request)?;

    let verb = request
        .verb
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| sanitize_verb(&request.label));

    let command = build_command(&request.executable_path, &request.args);

    let mut entries = Vec::new();
    for target in &request.targets {
        match target {
            ActionTarget::FolderBackground => {
                entries.push(MenuEntry {
                    id: String::new(),
                    label: request.label.clone(),
                    scope: EntryScope::CurrentUser,
                    key_path: format!(r"HKCU\Software\Classes\Directory\Background\shell\{}", verb),
                    icon: request.icon_path.clone(),
                    command: Some(command.clone()),
                    applies_to: vec!["directory_background".to_string()],
                    state: EntryState::Enabled,
                });
            }
            ActionTarget::Folders => {
                entries.push(MenuEntry {
                    id: String::new(),
                    label: request.label.clone(),
                    scope: EntryScope::CurrentUser,
                    key_path: format!(r"HKCU\Software\Classes\Directory\shell\{}", verb),
                    icon: request.icon_path.clone(),
                    command: Some(command.clone()),
                    applies_to: vec!["directory".to_string()],
                    state: EntryState::Enabled,
                });
            }
            ActionTarget::Drives => {
                entries.push(MenuEntry {
                    id: String::new(),
                    label: request.label.clone(),
                    scope: EntryScope::CurrentUser,
                    key_path: format!(r"HKCU\Software\Classes\Drive\shell\{}", verb),
                    icon: request.icon_path.clone(),
                    command: Some(command.clone()),
                    applies_to: vec!["drive".to_string()],
                    state: EntryState::Enabled,
                });
            }
            ActionTarget::Files => {
                if request.apply_to_all_files {
                    entries.push(MenuEntry {
                        id: String::new(),
                        label: request.label.clone(),
                        scope: EntryScope::CurrentUser,
                        key_path: format!(r"HKCU\Software\Classes\*\shell\{}", verb),
                        icon: request.icon_path.clone(),
                        command: Some(command.clone()),
                        applies_to: vec!["file".to_string()],
                        state: EntryState::Enabled,
                    });
                } else {
                    let mut extensions = BTreeSet::new();
                    for extension in &request.extensions {
                        extensions.insert(normalize_extension(extension));
                    }

                    for extension in extensions {
                        entries.push(MenuEntry {
                            id: String::new(),
                            label: request.label.clone(),
                            scope: EntryScope::CurrentUser,
                            key_path: format!(
                                r"HKCU\Software\Classes\SystemFileAssociations\{}\shell\{}",
                                extension, verb
                            ),
                            icon: request.icon_path.clone(),
                            command: Some(command.clone()),
                            applies_to: vec![extension],
                            state: EntryState::Enabled,
                        });
                    }
                }
            }
        }
    }

    let mut changes = Vec::with_capacity(entries.len());
    for mut entry in entries {
        entry.id = format!("{}:{}", entry.key_path, request.label);
        changes.push(ProposedChange {
            id: Uuid::new_v4().to_string(),
            kind: ChangeKind::Add,
            before: None,
            after: Some(entry),
            risk_level: RiskLevel::Medium,
            reason: "Add custom action".to_string(),
        });
    }

    if changes.is_empty() {
        return Err(anyhow!("No changes generated"));
    }

    Ok(changes)
}

fn build_command(executable_or_alias: &str, args: &str) -> String {
    let command_head = executable_or_alias.trim();
    let unquoted_head = command_head.trim_matches('"');
    let is_quoted = command_head.starts_with('"') && command_head.ends_with('"');

    let normalized_head =
        if !is_quoted && (command_head.contains(' ') || looks_like_file_path(command_head)) {
            format!("\"{command_head}\"")
        } else {
            command_head.to_string()
        };

    let inner = if args.trim().is_empty() {
        normalized_head
    } else {
        format!("{normalized_head} {}", args.trim())
    };

    // Explorer command invocation is stricter than an interactive terminal.
    // Route alias-style commands and script entry points through cmd for predictable resolution.
    let lower = unquoted_head.to_ascii_lowercase();
    let needs_cmd_shell = !looks_like_file_path(command_head)
        || lower.ends_with(".cmd")
        || lower.ends_with(".bat")
        || lower.ends_with(".ps1");

    if needs_cmd_shell {
        format!("cmd.exe /d /s /c {inner}")
    } else {
        inner
    }
}

#[cfg(test)]
mod tests {
    use super::{build_command, suggest_disable_git_bash};
    use crate::models::{EntryScope, EntryState, MenuEntry};

    #[test]
    fn suggests_disabling_git_bash() {
        let entries = vec![MenuEntry {
            id: "1".to_string(),
            label: "Open Git Bash Here".to_string(),
            scope: EntryScope::CurrentUser,
            key_path: "HKCU\\Software\\Classes\\Directory\\Background\\shell\\git_shell"
                .to_string(),
            icon: None,
            command: Some("git-bash.exe".to_string()),
            applies_to: vec!["directory_background".to_string()],
            state: EntryState::Enabled,
        }];

        let suggestions = suggest_disable_git_bash(&entries);
        assert_eq!(suggestions.len(), 1);
    }

    #[test]
    fn wraps_alias_command_through_cmd() {
        assert_eq!(
            build_command("mytool", "\"%1\""),
            "cmd.exe /d /s /c mytool \"%1\""
        );
    }

    #[test]
    fn quotes_file_path_in_command() {
        assert_eq!(
            build_command(r"C:\Program Files\Tool\tool.exe", "\"%1\""),
            r#""C:\Program Files\Tool\tool.exe" "%1""#
        );
    }

    #[test]
    fn wraps_script_path_through_cmd() {
        assert_eq!(
            build_command(r"C:\Tools\runner.cmd", "\"%1\""),
            r#"cmd.exe /d /s /c "C:\Tools\runner.cmd" "%1""#
        );
    }
}
