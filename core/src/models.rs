use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryScope {
    CurrentUser,
    ClassesRoot,
    LocalMachine,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MenuEntry {
    pub id: String,
    pub label: String,
    pub scope: EntryScope,
    pub key_path: String,
    pub command: Option<String>,
    pub applies_to: Vec<String>,
    pub state: EntryState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Disable,
    Enable,
    Add,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposedChange {
    pub id: String,
    pub kind: ChangeKind,
    pub before: Option<MenuEntry>,
    pub after: Option<MenuEntry>,
    pub risk_level: RiskLevel,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyBackup {
    pub key_path: String,
    pub existed: bool,
    pub values: BTreeMap<String, String>,
    pub command_values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeFailure {
    pub change_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplyResult {
    pub backups: Vec<KeyBackup>,
    pub applied: Vec<String>,
    pub failed: Vec<ChangeFailure>,
    pub change_set_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeSetRecord {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub changes: Vec<ProposedChange>,
    pub backups: Vec<KeyBackup>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeSetSummary {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub change_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CustomEntryScope {
    CurrentUser,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomEntryPayload {
    pub label: String,
    pub executable_path: String,
    pub args: String,
    pub icon_path: Option<String>,
    pub extensions: Vec<String>,
    pub verb: Option<String>,
    pub scope: CustomEntryScope,
}
