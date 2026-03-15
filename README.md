# Context Menu Helper

Desktop tool for safely managing common Windows context-menu entries.

## What is implemented

- Scan safe registry paths for shell menu entries.
- Suggest disabling known `Open Git Bash Here` variants.
- Toggle menu entries between enabled/disabled (`LegacyDisable` strategy).
- Generate generic custom actions for files/folders/background/drives.
- Apply change sets with automatic pre-change backups.
- Roll back by saved change-set ID.
- Tauri desktop UI for scan/suggest/custom/rollback workflows.

## Project layout

- `core`: reusable Rust engine (models, templates, registry provider, apply/rollback service, tests)
- `src-tauri`: Tauri desktop shell exposing commands
- `ui`: static frontend consumed by Tauri

## Run and test

### Core tests

```bash
cargo test -p context_menu_core
```

### Desktop app (Windows recommended)

```bash
cargo tauri dev
```

## Notes

- Registry write/read logic is Windows-only. On non-Windows platforms, registry commands return explicit errors.
- In this environment, desktop `cargo check` fails due to missing Linux GTK/WebKit dev libraries required by Tauri. This does not affect Windows builds.
