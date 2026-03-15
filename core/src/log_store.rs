use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::{Context, Result};

use crate::models::{ChangeSetRecord, ChangeSetSummary};

pub trait ChangeLogStore: Send + Sync {
    fn save(&self, record: &ChangeSetRecord) -> Result<()>;
    fn load(&self, id: &str) -> Result<Option<ChangeSetRecord>>;
    fn list(&self) -> Result<Vec<ChangeSetSummary>>;
}

pub struct JsonLogStore {
    base_path: PathBuf,
    lock: Mutex<()>,
}

impl JsonLogStore {
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
            lock: Mutex::new(()),
        }
    }

    fn ensure_dir(&self) -> Result<()> {
        if !self.base_path.exists() {
            fs::create_dir_all(&self.base_path).with_context(|| {
                format!(
                    "Failed to create log directory: {}",
                    self.base_path.display()
                )
            })?;
        }
        Ok(())
    }

    fn path_for(&self, id: &str) -> PathBuf {
        self.base_path.join(format!("{id}.json"))
    }
}

impl ChangeLogStore for JsonLogStore {
    fn save(&self, record: &ChangeSetRecord) -> Result<()> {
        let _guard = self.lock.lock().expect("log lock poisoned");
        self.ensure_dir()?;
        let path = self.path_for(&record.id);
        let contents = serde_json::to_string_pretty(record)?;
        fs::write(&path, contents)
            .with_context(|| format!("Failed to persist change set log: {}", path.display()))?;
        Ok(())
    }

    fn load(&self, id: &str) -> Result<Option<ChangeSetRecord>> {
        let _guard = self.lock.lock().expect("log lock poisoned");
        self.ensure_dir()?;
        let path = self.path_for(id);
        if !path.exists() {
            return Ok(None);
        }

        let body = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read change set log: {}", path.display()))?;
        let record: ChangeSetRecord = serde_json::from_str(&body)
            .with_context(|| format!("Failed to parse log file: {}", path.display()))?;
        Ok(Some(record))
    }

    fn list(&self) -> Result<Vec<ChangeSetSummary>> {
        let _guard = self.lock.lock().expect("log lock poisoned");
        self.ensure_dir()?;

        let mut out = Vec::new();
        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("json") {
                continue;
            }

            let body = fs::read_to_string(&path)?;
            let record: ChangeSetRecord = match serde_json::from_str(&body) {
                Ok(record) => record,
                Err(_) => continue,
            };

            out.push(ChangeSetSummary {
                id: record.id,
                created_at: record.created_at,
                change_count: record.changes.len(),
            });
        }

        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use chrono::Utc;

    use super::{ChangeLogStore, JsonLogStore};
    use crate::models::{ChangeSetRecord, KeyBackup};

    #[test]
    fn persists_and_loads_log_records() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = JsonLogStore::new(tmp.path());

        let record = ChangeSetRecord {
            id: "abc".to_string(),
            created_at: Utc::now(),
            changes: vec![],
            backups: vec![KeyBackup {
                key_path: "HKCU\\Software\\Classes\\Directory\\shell\\foo".to_string(),
                existed: true,
                values: BTreeMap::new(),
                command_values: BTreeMap::new(),
            }],
        };

        store.save(&record).expect("save");
        let loaded = store.load("abc").expect("load").expect("exists");
        assert_eq!(loaded.id, "abc");

        let list = store.list().expect("list");
        assert_eq!(list.len(), 1);
    }
}
