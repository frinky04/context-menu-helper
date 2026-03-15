use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use anyhow::{Result, anyhow};

use crate::models::{ChangeKind, EntryState, KeyBackup, MenuEntry, ProposedChange};
use crate::registry::RegistryProvider;

#[derive(Default)]
pub struct MockRegistryProvider {
    inner: Mutex<HashMap<String, MenuEntry>>,
}

impl MockRegistryProvider {
    pub fn with_entries(entries: Vec<MenuEntry>) -> Self {
        let map = entries
            .into_iter()
            .map(|entry| (entry.key_path.clone(), entry))
            .collect();
        Self {
            inner: Mutex::new(map),
        }
    }
}

impl RegistryProvider for MockRegistryProvider {
    fn scan_entries(&self) -> Result<Vec<MenuEntry>> {
        let map = self.inner.lock().expect("mock registry lock poisoned");
        let mut out: Vec<MenuEntry> = map.values().cloned().collect();
        out.sort_by(|a, b| a.label.cmp(&b.label));
        Ok(out)
    }

    fn snapshot_keys(&self, key_paths: &[String]) -> Result<Vec<KeyBackup>> {
        let map = self.inner.lock().expect("mock registry lock poisoned");
        let mut out = Vec::with_capacity(key_paths.len());
        for key_path in key_paths {
            if let Some(entry) = map.get(key_path) {
                let mut values = BTreeMap::new();
                values.insert("".to_string(), entry.label.clone());
                if let Some(icon) = &entry.icon {
                    values.insert("Icon".to_string(), icon.clone());
                }
                if entry.state == EntryState::Disabled {
                    values.insert("LegacyDisable".to_string(), String::new());
                }

                let mut command_values = BTreeMap::new();
                if let Some(cmd) = &entry.command {
                    command_values.insert("".to_string(), cmd.clone());
                }

                out.push(KeyBackup {
                    key_path: key_path.clone(),
                    existed: true,
                    values,
                    command_values,
                });
            } else {
                out.push(KeyBackup {
                    key_path: key_path.clone(),
                    existed: false,
                    values: BTreeMap::new(),
                    command_values: BTreeMap::new(),
                });
            }
        }

        Ok(out)
    }

    fn apply_change(&self, change: &ProposedChange) -> Result<()> {
        let mut map = self.inner.lock().expect("mock registry lock poisoned");
        let key_path = change
            .after
            .as_ref()
            .or(change.before.as_ref())
            .map(|entry| entry.key_path.clone())
            .ok_or_else(|| anyhow!("change payload missing entry"))?;

        match change.kind {
            ChangeKind::Disable => {
                let entry = map
                    .get_mut(&key_path)
                    .ok_or_else(|| anyhow!("entry not found for disable"))?;
                entry.state = EntryState::Disabled;
            }
            ChangeKind::Enable => {
                let entry = map
                    .get_mut(&key_path)
                    .ok_or_else(|| anyhow!("entry not found for enable"))?;
                entry.state = EntryState::Enabled;
            }
            ChangeKind::Add => {
                let entry = change
                    .after
                    .as_ref()
                    .cloned()
                    .ok_or_else(|| anyhow!("missing after entry for add"))?;
                map.insert(key_path, entry);
            }
            ChangeKind::Remove => {
                map.remove(&key_path);
            }
        }

        Ok(())
    }

    fn restore_backup(&self, backup: &KeyBackup) -> Result<()> {
        let mut map = self.inner.lock().expect("mock registry lock poisoned");

        if !backup.existed {
            map.remove(&backup.key_path);
            return Ok(());
        }

        let state = if backup.values.contains_key("LegacyDisable") {
            EntryState::Disabled
        } else {
            EntryState::Enabled
        };

        let label = backup
            .values
            .get("")
            .cloned()
            .unwrap_or_else(|| "Restored Entry".to_string());

        let entry = MenuEntry {
            id: backup.key_path.to_ascii_lowercase(),
            label,
            scope: crate::models::EntryScope::CurrentUser,
            key_path: backup.key_path.clone(),
            icon: backup.values.get("Icon").cloned(),
            command: backup.command_values.get("").cloned(),
            applies_to: vec!["unknown".to_string()],
            state,
        };

        map.insert(backup.key_path.clone(), entry);
        Ok(())
    }
}
