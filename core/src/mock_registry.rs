use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use anyhow::{Result, anyhow};

use crate::models::{
    ChangeKind, EntryState, KeyBackup, MenuEntry, ProposedChange, RegistryKeySnapshot,
    RegistryValueSnapshot,
};
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
                let mut values = vec![string_value("", &entry.label)];
                if let Some(icon) = &entry.icon {
                    values.push(string_value("Icon", icon));
                }
                if entry.state == EntryState::Disabled {
                    values.push(string_value("LegacyDisable", ""));
                }
                values.sort_by(|a, b| a.name.cmp(&b.name));

                let mut subkeys = BTreeMap::new();
                if let Some(cmd) = &entry.command {
                    subkeys.insert(
                        "command".to_string(),
                        RegistryKeySnapshot {
                            values: vec![string_value("", cmd)],
                            subkeys: BTreeMap::new(),
                        },
                    );
                }

                out.push(KeyBackup {
                    key_path: key_path.clone(),
                    existed: true,
                    snapshot: Some(RegistryKeySnapshot { values, subkeys }),
                });
            } else {
                out.push(KeyBackup {
                    key_path: key_path.clone(),
                    existed: false,
                    snapshot: None,
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

        let snapshot = backup
            .snapshot
            .as_ref()
            .ok_or_else(|| anyhow!("missing snapshot for existing backup"))?;

        let state = if snapshot
            .values
            .iter()
            .any(|value| value.name.eq_ignore_ascii_case("LegacyDisable"))
        {
            EntryState::Disabled
        } else {
            EntryState::Enabled
        };

        let label = snapshot
            .values
            .iter()
            .find(|value| value.name.is_empty())
            .and_then(decode_string_value)
            .unwrap_or_else(|| "Restored Entry".to_string());

        let icon = snapshot
            .values
            .iter()
            .find(|value| value.name.eq_ignore_ascii_case("Icon"))
            .and_then(decode_string_value);

        let command = snapshot
            .subkeys
            .get("command")
            .and_then(|command_key| {
                command_key
                    .values
                    .iter()
                    .find(|value| value.name.is_empty())
            })
            .and_then(decode_string_value);

        let entry = MenuEntry {
            id: backup.key_path.to_ascii_lowercase(),
            label,
            scope: crate::models::EntryScope::CurrentUser,
            key_path: backup.key_path.clone(),
            icon,
            command,
            applies_to: vec!["unknown".to_string()],
            state,
        };

        map.insert(backup.key_path.clone(), entry);
        Ok(())
    }
}

fn string_value(name: &str, value: &str) -> RegistryValueSnapshot {
    RegistryValueSnapshot {
        name: name.to_string(),
        value_type: 1,
        data: value.as_bytes().to_vec(),
    }
}

fn decode_string_value(value: &RegistryValueSnapshot) -> Option<String> {
    String::from_utf8(value.data.clone())
        .ok()
        .map(|decoded| decoded.trim_end_matches('\0').to_string())
}
