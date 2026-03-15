use std::sync::Arc;

use anyhow::{Result, anyhow};

use crate::models::{KeyBackup, MenuEntry, ProposedChange};

pub trait RegistryProvider: Send + Sync {
    fn scan_entries(&self) -> Result<Vec<MenuEntry>>;
    fn snapshot_keys(&self, key_paths: &[String]) -> Result<Vec<KeyBackup>>;
    fn apply_change(&self, change: &ProposedChange) -> Result<()>;
    fn restore_backup(&self, backup: &KeyBackup) -> Result<()>;
}

pub type SharedRegistryProvider = Arc<dyn RegistryProvider>;

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        collections::{BTreeMap, BTreeSet},
        iter,
        path::Path,
    };

    use anyhow::Context;
    use winreg::{
        HKEY, RegKey,
        enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
    };

    use crate::models::{ChangeKind, EntryScope, EntryState};

    const DIRECT_SHELL_PATHS: &[(&str, &str)] = &[
        ("HKCU", r"Software\Classes\Directory\Background\shell"),
        ("HKCU", r"Software\Classes\Directory\shell"),
        ("HKCU", r"Software\Classes\Drive\shell"),
        ("HKCU", r"Software\Classes\*\shell"),
        ("HKCU", r"Software\Classes\AllFilesystemObjects\shell"),
        ("HKCU", r"Software\Classes\SystemFileAssociations"),
        ("HKCR", r"Directory\Background\shell"),
        ("HKCR", r"Directory\shell"),
        ("HKCR", r"Drive\shell"),
        ("HKCR", r"*\shell"),
        ("HKCR", r"AllFilesystemObjects\shell"),
        ("HKCR", r"SystemFileAssociations"),
    ];

    pub struct WindowsRegistryProvider;

    impl WindowsRegistryProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl RegistryProvider for WindowsRegistryProvider {
        fn scan_entries(&self) -> Result<Vec<MenuEntry>> {
            let mut entries = Vec::new();

            for (hive_name, base_path) in DIRECT_SHELL_PATHS {
                let hive = open_hive(hive_name)?;
                let base = match hive.open_subkey(base_path) {
                    Ok(key) => key,
                    Err(_) => continue,
                };

                if base_path.ends_with(r"SystemFileAssociations") {
                    scan_system_file_associations(hive_name, base_path, &base, &mut entries)?;
                    continue;
                }

                scan_shell_verbs(hive_name, base_path, &base, &mut entries)?;

                if let Some(prefix) = base_path.strip_suffix(r"\shell") {
                    let handlers_path = format!(r"{prefix}\shellex\ContextMenuHandlers");
                    if let Ok(handler_root) = hive.open_subkey(&handlers_path) {
                        scan_context_menu_handlers(
                            hive_name,
                            &handlers_path,
                            &handler_root,
                            &mut entries,
                        )?;
                    }
                }
            }

            entries.sort_by(|a, b| a.label.cmp(&b.label));
            Ok(entries)
        }

        fn snapshot_keys(&self, key_paths: &[String]) -> Result<Vec<KeyBackup>> {
            let mut unique = BTreeSet::new();
            unique.extend(key_paths.iter().cloned());

            unique
                .into_iter()
                .map(snapshot_single_key)
                .collect::<Result<Vec<_>>>()
        }

        fn apply_change(&self, change: &ProposedChange) -> Result<()> {
            let entry = change
                .after
                .as_ref()
                .or(change.before.as_ref())
                .ok_or_else(|| anyhow!("change missing entry payload"))?;
            let (hive, subpath) = parse_path(&entry.key_path)?;
            let hive_key = open_hive(hive)?;

            match change.kind {
                ChangeKind::Disable => {
                    let key = hive_key
                        .open_subkey_with_flags(subpath, winreg::enums::KEY_SET_VALUE)
                        .with_context(|| {
                            format!("failed to open key for disable: {}", entry.key_path)
                        })?;
                    key.set_value("LegacyDisable", &"")?;
                }
                ChangeKind::Enable => {
                    let key = hive_key
                        .open_subkey_with_flags(subpath, winreg::enums::KEY_SET_VALUE)
                        .with_context(|| {
                            format!("failed to open key for enable: {}", entry.key_path)
                        })?;
                    let _ = key.delete_value("LegacyDisable");
                }
                ChangeKind::Add => {
                    let (key, _) = hive_key.create_subkey(subpath)?;
                    key.set_value("", &entry.label)?;
                    key.set_value("MUIVerb", &entry.label)?;
                    if let Some(command) = &entry.command {
                        let (command_key, _) = key.create_subkey("command")?;
                        command_key.set_value("", command)?;
                    }
                }
                ChangeKind::Remove => {
                    let _ = hive_key.delete_subkey_all(subpath);
                }
            }

            Ok(())
        }

        fn restore_backup(&self, backup: &KeyBackup) -> Result<()> {
            let (hive, subpath) = parse_path(&backup.key_path)?;
            let hive_key = open_hive(hive)?;
            let _ = hive_key.delete_subkey_all(subpath);

            if !backup.existed {
                return Ok(());
            }

            let (key, _) = hive_key.create_subkey(subpath)?;
            write_values(&key, &backup.values)?;
            if !backup.command_values.is_empty() {
                let (command_key, _) = key.create_subkey("command")?;
                write_values(&command_key, &backup.command_values)?;
            }

            Ok(())
        }
    }

    fn scan_system_file_associations(
        hive_name: &str,
        base_path: &str,
        root: &RegKey,
        entries: &mut Vec<MenuEntry>,
    ) -> Result<()> {
        for ext in root.enum_keys().flatten() {
            if !ext.starts_with('.') {
                continue;
            }

            if let Ok(shell_key) = root.open_subkey(format!(r"{}\shell", ext)) {
                for verb in shell_key.enum_keys().flatten() {
                    let absolute = format!(r"{}\{}\{}\shell\{}", hive_name, base_path, ext, verb);
                    if let Ok(mut entry) = read_entry(&absolute) {
                        entry.applies_to = vec![ext.clone()];
                        entries.push(entry);
                    }
                }
            }

            let handlers_path = format!(r"{}\shellex\ContextMenuHandlers", ext);
            if let Ok(handlers_root) = root.open_subkey(&handlers_path) {
                scan_context_menu_handlers(
                    hive_name,
                    &format!(r"{}\{}", base_path, handlers_path),
                    &handlers_root,
                    entries,
                )?;
            }
        }

        Ok(())
    }

    fn scan_shell_verbs(
        hive_name: &str,
        base_path: &str,
        base: &RegKey,
        entries: &mut Vec<MenuEntry>,
    ) -> Result<()> {
        for subkey_name in base.enum_keys().flatten() {
            let absolute = format!(r"{}\{}\{}", hive_name, base_path, subkey_name);
            if let Ok(entry) = read_entry(&absolute) {
                entries.push(entry);
            }
        }
        Ok(())
    }

    fn scan_context_menu_handlers(
        hive_name: &str,
        handlers_path: &str,
        root: &RegKey,
        entries: &mut Vec<MenuEntry>,
    ) -> Result<()> {
        for handler in root.enum_keys().flatten() {
            let absolute = format!(r"{}\{}\{}", hive_name, handlers_path, handler);
            if let Ok(entry) = read_handler_entry(&absolute, &handler) {
                entries.push(entry);
            }
        }
        Ok(())
    }

    fn read_handler_entry(absolute_key_path: &str, fallback_label: &str) -> Result<MenuEntry> {
        let (hive, subpath) = parse_path(absolute_key_path)?;
        let hive_key = open_hive(hive)?;
        let key = hive_key.open_subkey(subpath)?;

        let raw_label = key
            .get_value::<String, _>("MUIVerb")
            .or_else(|_| key.get_value::<String, _>(""))
            .unwrap_or_else(|_| fallback_label.to_string());
        let handler_clsid = key
            .get_value::<String, _>("")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let mut label = normalize_label(&raw_label, subpath);
        if (looks_like_guid(&label) || label.eq_ignore_ascii_case(fallback_label))
            && let Some(resolved) = handler_clsid
                .as_deref()
                .and_then(resolve_clsid_label)
                .filter(|value| !looks_like_guid(value))
        {
            label = resolved;
        }

        let state = if key.get_raw_value("LegacyDisable").is_ok() {
            EntryState::Disabled
        } else {
            EntryState::Enabled
        };

        Ok(MenuEntry {
            id: absolute_key_path.to_ascii_lowercase(),
            label,
            scope: scope_from_hive(hive),
            key_path: absolute_key_path.to_string(),
            command: handler_clsid,
            applies_to: infer_applies_to(subpath),
            state,
        })
    }

    fn read_entry(absolute_key_path: &str) -> Result<MenuEntry> {
        let (hive, subpath) = parse_path(absolute_key_path)?;
        let hive_key = open_hive(hive)?;
        let key = hive_key.open_subkey(subpath)?;

        let raw_label = key
            .get_value::<String, _>("MUIVerb")
            .or_else(|_| key.get_value::<String, _>(""))
            .unwrap_or_else(|_| subpath.split('\\').last().unwrap_or("Unnamed").to_string());
        let label = normalize_label(&raw_label, subpath);

        let command = key
            .open_subkey("command")
            .ok()
            .and_then(|cmd| cmd.get_value::<String, _>("").ok());

        let state = if key.get_raw_value("LegacyDisable").is_ok() {
            EntryState::Disabled
        } else {
            EntryState::Enabled
        };

        let scope = scope_from_hive(hive);
        let applies_to = infer_applies_to(subpath);

        Ok(MenuEntry {
            id: absolute_key_path.to_ascii_lowercase(),
            label,
            scope,
            key_path: absolute_key_path.to_string(),
            command,
            applies_to,
            state,
        })
    }

    fn snapshot_single_key(path: String) -> Result<KeyBackup> {
        let (hive, subpath) = parse_path(&path)?;
        let hive_key = open_hive(hive)?;

        let key = match hive_key.open_subkey(subpath) {
            Ok(key) => key,
            Err(_) => {
                return Ok(KeyBackup {
                    key_path: path,
                    existed: false,
                    values: BTreeMap::new(),
                    command_values: BTreeMap::new(),
                });
            }
        };

        let values = read_all_string_values(&key)?;
        let command_values = key
            .open_subkey("command")
            .ok()
            .map(|k| read_all_string_values(&k))
            .transpose()?
            .unwrap_or_default();

        Ok(KeyBackup {
            key_path: path,
            existed: true,
            values,
            command_values,
        })
    }

    fn read_all_string_values(key: &RegKey) -> Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();

        for (name, _) in key.enum_values().flatten() {
            if let Ok(value) = key.get_value::<String, _>(&name) {
                out.insert(name, value);
            }
        }

        if !out.contains_key("") {
            if let Ok(default) = key.get_value::<String, _>("") {
                out.insert("".to_string(), default);
            }
        }

        Ok(out)
    }

    fn write_values(key: &RegKey, values: &BTreeMap<String, String>) -> Result<()> {
        for (name, value) in values {
            if name.is_empty() {
                key.set_value("", value)?;
            } else {
                key.set_value(name, value)?;
            }
        }
        Ok(())
    }

    fn infer_applies_to(path: &str) -> Vec<String> {
        if path.contains(r"SystemFileAssociations\") {
            if let Some(ext) = path.split('\\').find(|part| part.starts_with('.')) {
                return vec![ext.to_string()];
            }
        }
        if path.contains(r"Directory\Background") {
            return vec!["directory_background".to_string()];
        }
        if path.contains(r"Directory\shell") {
            return vec!["directory".to_string()];
        }
        if path.contains(r"Drive\shell") {
            return vec!["drive".to_string()];
        }
        if path.contains(r"AllFilesystemObjects\") {
            return vec!["all_filesystem_objects".to_string()];
        }
        if path.contains(r"*\shell") || path.contains(r"*\shellex") {
            return vec!["file".to_string()];
        }

        if let Some(ext) = path.split('\\').find(|part| part.starts_with('.')) {
            return vec![ext.to_string()];
        }

        vec!["unknown".to_string()]
    }

    fn normalize_label(raw: &str, subpath: &str) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return subpath.split('\\').last().unwrap_or("Unnamed").to_string();
        }

        if let Some(indirect) = trimmed.strip_prefix('@') {
            let mut parts = indirect.splitn(2, ',');
            let dll_path = parts.next().unwrap_or(indirect).trim_matches('"');
            let resource_id = parts.next().map(str::trim).unwrap_or("");
            let dll_name = Path::new(dll_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(dll_path);

            if resource_id.is_empty() {
                return dll_name.to_string();
            }

            return format!("{dll_name},{resource_id}");
        }

        trimmed.to_string()
    }

    fn resolve_clsid_label(clsid_value: &str) -> Option<String> {
        let normalized = normalize_clsid(clsid_value)?;
        let hkcr = open_hive("HKCR").ok()?;
        let clsid_key = hkcr.open_subkey(format!(r"CLSID\{}", normalized)).ok()?;

        if let Ok(name) = clsid_key.get_value::<String, _>("") {
            let trimmed = name.trim();
            if !trimmed.is_empty() && !looks_like_guid(trimmed) {
                return Some(normalize_label(trimmed, "CLSID"));
            }
        }

        if let Ok(localized) = clsid_key.get_value::<String, _>("LocalizedString") {
            let normalized = normalize_label(&localized, "CLSID");
            if !normalized.is_empty() && !looks_like_guid(&normalized) {
                return Some(normalized);
            }
        }

        if let Ok(inproc) = clsid_key.open_subkey("InprocServer32")
            && let Ok(path) = inproc.get_value::<String, _>("")
            && let Some(name) = binary_name_from_path(&path)
        {
            return Some(name);
        }

        if let Ok(local_server) = clsid_key.open_subkey("LocalServer32")
            && let Ok(path) = local_server.get_value::<String, _>("")
            && let Some(name) = binary_name_from_path(&path)
        {
            return Some(name);
        }

        None
    }

    fn binary_name_from_path(path: &str) -> Option<String> {
        let trimmed = path.trim().trim_matches('"');
        if trimmed.is_empty() {
            return None;
        }

        let file_stem = Path::new(trimmed).file_stem()?.to_str()?.trim();
        if file_stem.is_empty() {
            return None;
        }

        Some(file_stem.to_string())
    }

    fn normalize_clsid(value: &str) -> Option<String> {
        let trimmed = value.trim().trim_matches(|ch| ch == '{' || ch == '}');
        if !looks_like_guid(trimmed) {
            return None;
        }
        Some(format!("{{{}}}", trimmed))
    }

    fn looks_like_guid(value: &str) -> bool {
        let trimmed = value.trim().trim_matches(|ch| ch == '{' || ch == '}');
        let mut parts = trimmed.split('-');
        let expected = [8, 4, 4, 4, 12];
        for size in expected {
            let part = match parts.next() {
                Some(part) => part,
                None => return false,
            };
            if part.len() != size || !part.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return false;
            }
        }
        parts.next().is_none()
    }

    fn scope_from_hive(hive: &str) -> EntryScope {
        match hive {
            "HKCU" => EntryScope::CurrentUser,
            "HKLM" => EntryScope::LocalMachine,
            _ => EntryScope::ClassesRoot,
        }
    }

    fn parse_path(path: &str) -> Result<(&str, &str)> {
        let mut parts = path.splitn(2, '\\');
        let hive = parts
            .next()
            .ok_or_else(|| anyhow!("missing hive in key path: {path}"))?;
        let subpath = parts
            .next()
            .ok_or_else(|| anyhow!("missing subpath in key path: {path}"))?;
        Ok((hive, subpath))
    }

    fn open_hive(hive_name: &str) -> Result<RegKey> {
        let hive: HKEY = match hive_name {
            "HKCU" => HKEY_CURRENT_USER,
            "HKCR" => HKEY_CLASSES_ROOT,
            "HKLM" => HKEY_LOCAL_MACHINE,
            _ => return Err(anyhow!("unsupported hive: {hive_name}")),
        };

        Ok(RegKey::predef(hive))
    }

    #[allow(dead_code)]
    fn _iter_one<T>(item: T) -> impl Iterator<Item = T> {
        iter::once(item)
    }
}

#[cfg(not(windows))]
mod windows {
    use super::*;

    pub struct WindowsRegistryProvider;

    impl WindowsRegistryProvider {
        pub fn new() -> Self {
            Self
        }
    }

    impl RegistryProvider for WindowsRegistryProvider {
        fn scan_entries(&self) -> Result<Vec<MenuEntry>> {
            Err(anyhow!("Registry scanning is only supported on Windows"))
        }

        fn snapshot_keys(&self, _key_paths: &[String]) -> Result<Vec<KeyBackup>> {
            Err(anyhow!("Registry backups are only supported on Windows"))
        }

        fn apply_change(&self, _change: &ProposedChange) -> Result<()> {
            Err(anyhow!("Registry writes are only supported on Windows"))
        }

        fn restore_backup(&self, _backup: &KeyBackup) -> Result<()> {
            Err(anyhow!("Registry restore is only supported on Windows"))
        }
    }
}

pub use windows::WindowsRegistryProvider;
