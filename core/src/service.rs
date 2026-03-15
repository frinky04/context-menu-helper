use std::sync::Arc;

use anyhow::{Result, anyhow};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    log_store::ChangeLogStore,
    models::{
        ApplyResult, ChangeFailure, ChangeSetRecord, ChangeSetSummary, CreateActionRequest,
        ProposedChange,
    },
    registry::{RegistryProvider, SharedRegistryProvider},
    templates::{build_create_action_changes, build_toggle_change, suggest_disable_git_bash},
};

pub struct ContextMenuService {
    provider: SharedRegistryProvider,
    log_store: Arc<dyn ChangeLogStore>,
}

impl ContextMenuService {
    pub fn new(provider: Arc<dyn RegistryProvider>, log_store: Arc<dyn ChangeLogStore>) -> Self {
        Self {
            provider,
            log_store,
        }
    }

    pub fn scan_entries(&self) -> Result<Vec<crate::models::MenuEntry>> {
        self.provider.scan_entries()
    }

    pub fn suggest_actions(&self) -> Result<Vec<ProposedChange>> {
        let entries = self.provider.scan_entries()?;
        Ok(suggest_disable_git_bash(&entries))
    }

    pub fn create_action(&self, request: CreateActionRequest) -> Result<Vec<ProposedChange>> {
        build_create_action_changes(&request)
    }

    pub fn toggle_entry(&self, id: &str, enabled: bool) -> Result<ApplyResult> {
        let entry = self
            .provider
            .scan_entries()?
            .into_iter()
            .find(|entry| entry.id == id || entry.key_path.eq_ignore_ascii_case(id))
            .ok_or_else(|| anyhow!("Menu entry not found: {id}"))?;

        let change = build_toggle_change(
            &entry,
            enabled,
            if enabled {
                "User requested enable"
            } else {
                "User requested disable"
            },
        );
        self.apply_changes(vec![change])
    }

    pub fn apply_changes(&self, changes: Vec<ProposedChange>) -> Result<ApplyResult> {
        if changes.is_empty() {
            return Ok(ApplyResult {
                backups: vec![],
                applied: vec![],
                failed: vec![],
                change_set_id: None,
            });
        }

        let key_paths = collect_key_paths(&changes);
        let backups = self.provider.snapshot_keys(&key_paths)?;

        let mut applied = Vec::new();
        let mut failed = Vec::new();
        for change in &changes {
            match self.provider.apply_change(change) {
                Ok(_) => applied.push(change.id.clone()),
                Err(err) => failed.push(ChangeFailure {
                    change_id: change.id.clone(),
                    message: err.to_string(),
                }),
            }
        }

        let mut change_set_id = None;
        if !applied.is_empty() {
            let id = Uuid::new_v4().to_string();
            let record = ChangeSetRecord {
                id: id.clone(),
                created_at: Utc::now(),
                changes,
                backups: backups.clone(),
            };
            self.log_store.save(&record)?;
            change_set_id = Some(id);
        }

        Ok(ApplyResult {
            backups,
            applied,
            failed,
            change_set_id,
        })
    }

    pub fn rollback(&self, change_set_id: &str) -> Result<ApplyResult> {
        let record = self
            .log_store
            .load(change_set_id)?
            .ok_or_else(|| anyhow!("Change set not found: {change_set_id}"))?;

        let mut applied = Vec::new();
        let mut failed = Vec::new();
        for backup in &record.backups {
            match self.provider.restore_backup(backup) {
                Ok(_) => applied.push(backup.key_path.clone()),
                Err(err) => failed.push(ChangeFailure {
                    change_id: backup.key_path.clone(),
                    message: err.to_string(),
                }),
            }
        }

        Ok(ApplyResult {
            backups: record.backups,
            applied,
            failed,
            change_set_id: Some(change_set_id.to_string()),
        })
    }

    pub fn list_change_sets(&self) -> Result<Vec<ChangeSetSummary>> {
        self.log_store.list()
    }

    pub fn get_change_set(&self, change_set_id: &str) -> Result<ChangeSetRecord> {
        self.log_store
            .load(change_set_id)?
            .ok_or_else(|| anyhow!("Change set not found: {change_set_id}"))
    }
}

fn collect_key_paths(changes: &[ProposedChange]) -> Vec<String> {
    let mut out = Vec::new();
    for change in changes {
        if let Some(after) = &change.after {
            out.push(after.key_path.clone());
        } else if let Some(before) = &change.before {
            out.push(before.key_path.clone());
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::tempdir;

    use crate::{
        log_store::JsonLogStore,
        mock_registry::MockRegistryProvider,
        models::{ChangeKind, EntryScope, EntryState, MenuEntry, ProposedChange, RiskLevel},
        registry::RegistryProvider,
    };

    use super::ContextMenuService;

    #[test]
    fn apply_and_rollback_round_trip() {
        let entry = MenuEntry {
            id: "git_shell".to_string(),
            label: "Open Git Bash Here".to_string(),
            scope: EntryScope::CurrentUser,
            key_path: "HKCU\\Software\\Classes\\Directory\\Background\\shell\\git_shell"
                .to_string(),
            icon: None,
            command: Some("git-bash.exe".to_string()),
            applies_to: vec!["directory_background".to_string()],
            state: EntryState::Enabled,
        };

        let provider = Arc::new(MockRegistryProvider::with_entries(vec![entry.clone()]));
        let logs = Arc::new(JsonLogStore::new(tempdir().expect("tempdir").path()));
        let service = ContextMenuService::new(provider.clone(), logs);

        let change = ProposedChange {
            id: "change-1".to_string(),
            kind: ChangeKind::Disable,
            before: Some(entry.clone()),
            after: Some(MenuEntry {
                state: EntryState::Disabled,
                ..entry
            }),
            risk_level: RiskLevel::Low,
            reason: "test".to_string(),
        };

        let result = service.apply_changes(vec![change]).expect("apply");
        assert_eq!(result.failed.len(), 0);
        let change_set_id = result.change_set_id.expect("change set");

        let scanned = provider.scan_entries().expect("scan");
        assert_eq!(scanned[0].state, EntryState::Disabled);

        let rollback = service.rollback(&change_set_id).expect("rollback");
        assert_eq!(rollback.failed.len(), 0);

        let rescanned = provider.scan_entries().expect("scan");
        assert_eq!(rescanned[0].state, EntryState::Enabled);
    }

    #[test]
    fn can_load_change_set_details() {
        let entry = MenuEntry {
            id: "entry-1".to_string(),
            label: "Open with Tool".to_string(),
            scope: EntryScope::CurrentUser,
            key_path: "HKCU\\Software\\Classes\\*\\shell\\tool".to_string(),
            icon: None,
            command: Some("tool.exe \"%1\"".to_string()),
            applies_to: vec!["file".to_string()],
            state: EntryState::Enabled,
        };

        let provider = Arc::new(MockRegistryProvider::default());
        let logs = Arc::new(JsonLogStore::new(tempdir().expect("tempdir").path()));
        let service = ContextMenuService::new(provider, logs);

        let change = ProposedChange {
            id: "change-1".to_string(),
            kind: ChangeKind::Add,
            before: None,
            after: Some(entry),
            risk_level: RiskLevel::Medium,
            reason: "test add".to_string(),
        };

        let result = service.apply_changes(vec![change]).expect("apply");
        let change_set_id = result.change_set_id.expect("change set id");

        let details = service.get_change_set(&change_set_id).expect("details");
        assert_eq!(details.id, change_set_id);
        assert_eq!(details.changes.len(), 1);
    }
}
