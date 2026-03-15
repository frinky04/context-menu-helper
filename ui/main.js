const state = {
  entries: [],
  suggestions: [],
  customChanges: [],
  changeSets: [],
  changeSetDetails: {},
  expandedChangeSets: {},
  showAdvanced: false,
  searchTerm: "",
  selectedCategory: "all",
  activePanel: "entries"
};

const ui = {
  status: document.getElementById("status-text"),
  entriesList: document.getElementById("entries-list"),
  entriesSummary: document.getElementById("entries-summary"),
  suggestionsList: document.getElementById("suggestions-list"),
  customChangesList: document.getElementById("custom-changes-list"),
  changeSetsList: document.getElementById("change-sets-list"),
  applySuggestionsBtn: document.getElementById("apply-suggestions-btn"),
  applyCustomBtn: document.getElementById("apply-custom-btn"),
  refreshBtn: document.getElementById("refresh-btn"),
  customForm: document.getElementById("custom-form"),
  showAdvancedToggle: document.getElementById("show-advanced-toggle"),
  searchInput: document.getElementById("search-input"),
  categoryFilters: document.getElementById("category-filters"),
  targetFiles: document.getElementById("target-files"),
  targetFolders: document.getElementById("target-folders"),
  targetFolderBackground: document.getElementById("target-folder-background"),
  targetDrives: document.getElementById("target-drives"),
  fileTypeToggle: document.getElementById("file-type-toggle"),
  fileTypeRow: document.getElementById("file-type-row"),
  extensionsRow: document.getElementById("extensions-row"),
  chipContainer: document.getElementById("chip-container"),
  chipInput: document.getElementById("chip-input"),
  executableInput: document.getElementById("executable"),
  iconInput: document.getElementById("icon"),
  browseExecutableBtn: document.getElementById("browse-executable-btn"),
  browseIconBtn: document.getElementById("browse-icon-btn"),
  argsInput: document.getElementById("args"),
  commandPreview: document.getElementById("command-preview"),
  quickFillButtons: [...document.querySelectorAll(".quick-fill-btn")],
  resetFormBtn: document.getElementById("reset-form-btn"),
  pendingCard: document.getElementById("pending-card"),
  pendingCount: document.getElementById("pending-count"),
  discardChangesBtn: document.getElementById("discard-changes-btn")
};

ui.showAdvancedToggle.checked = state.showAdvanced;

const CATEGORY_META = {
  all: "All",
  developer: "Developer",
  media: "Media",
  archive: "Archives",
  file_ops: "File Actions",
  shell_ext: "Extensions",
  system: "System",
  other: "Other"
};

// ── Chip input for extensions ──────────────────────────

class ChipInput {
  constructor(container, input, onChange) {
    this.container = container;
    this.input = input;
    this.onChange = onChange;
    this.values = new Set();

    this.input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === ",") {
        event.preventDefault();
        this._addFromInput();
      }
      if (event.key === "Backspace" && this.input.value === "" && this.values.size > 0) {
        const last = [...this.values].pop();
        this.remove(last);
      }
    });

    this.input.addEventListener("paste", (event) => {
      event.preventDefault();
      const text = (event.clipboardData || window.clipboardData).getData("text");
      for (const part of text.split(",")) {
        const normalized = this._normalize(part);
        if (normalized) {
          this.add(normalized);
        }
      }
    });

    this.container.addEventListener("click", () => this.input.focus());
  }

  _normalize(value) {
    let v = value.trim().toLowerCase();
    if (!v) {
      return null;
    }
    if (!v.startsWith(".")) {
      v = `.${v}`;
    }
    if (v.length < 2 || !/^\.[a-z0-9]+$/i.test(v)) {
      return null;
    }
    return v;
  }

  _addFromInput() {
    const normalized = this._normalize(this.input.value);
    if (normalized) {
      this.add(normalized);
    }
    this.input.value = "";
  }

  add(value) {
    if (this.values.has(value)) {
      return;
    }
    this.values.add(value);

    const chip = document.createElement("span");
    chip.className = "chip";
    chip.dataset.value = value;
    chip.innerHTML = `${value}<button type="button" class="chip-remove">&times;</button>`;
    chip.querySelector(".chip-remove").addEventListener("click", () => this.remove(value));
    this.container.insertBefore(chip, this.input);
    this._updatePlaceholder();
    this.onChange();
  }

  remove(value) {
    this.values.delete(value);
    const chip = this.container.querySelector(`.chip[data-value="${value}"]`);
    if (chip) {
      chip.remove();
    }
    this._updatePlaceholder();
    this.onChange();
  }

  getValues() {
    return [...this.values];
  }

  clear() {
    this.values.clear();
    this.container.querySelectorAll(".chip").forEach((chip) => chip.remove());
    this.input.value = "";
    this._updatePlaceholder();
    this.onChange();
  }

  _updatePlaceholder() {
    this.input.placeholder = this.values.size === 0 ? "Type extension, press Enter" : "";
  }
}

// ── Form state helpers ────────────────────────────────

function getFileTypeMode() {
  const active = ui.fileTypeToggle.querySelector(".toggle-option.active");
  return active ? active.dataset.value : "all";
}

function updateAddActionFormState() {
  const filesChecked = ui.targetFiles.checked;

  if (!filesChecked) {
    ui.fileTypeRow.style.display = "none";
    ui.extensionsRow.style.display = "none";
    return;
  }

  ui.fileTypeRow.style.display = "";
  ui.extensionsRow.style.display = getFileTypeMode() === "specific" ? "" : "none";
}

function updateCommandPreview() {
  const executable = ui.executableInput.value.trim();
  const args = ui.argsInput.value.trim();
  if (!executable) {
    ui.commandPreview.textContent = "Set a command above to see the preview.";
    return;
  }
  ui.commandPreview.textContent = `${executable}${args ? ` ${args}` : ""}`;
}

function updateQuickFillHighlight() {
  const currentArgs = ui.argsInput.value;
  for (const btn of ui.quickFillButtons) {
    btn.classList.toggle("active", btn.dataset.args === currentArgs);
  }
}

function resetCreateForm() {
  document.getElementById("label").value = "";
  ui.executableInput.value = "";
  ui.argsInput.value = "";
  ui.iconInput.value = "";
  ui.targetFiles.checked = true;
  ui.targetFolders.checked = false;
  ui.targetFolderBackground.checked = false;
  ui.targetDrives.checked = false;

  // Reset file type toggle to "All file types"
  for (const btn of ui.fileTypeToggle.querySelectorAll(".toggle-option")) {
    btn.classList.toggle("active", btn.dataset.value === "all");
  }

  extensionChips.clear();
  state.customChanges = [];
  renderCustomChanges();
  updateAddActionFormState();
  updateCommandPreview();
  updateQuickFillHighlight();
}

let _statusTimer = null;
const _statusBar = document.querySelector(".status-bar");
const _toastContainer = document.getElementById("toast-container");

const MAX_TOASTS = 5;
const TOAST_ICONS = { success: "\u2713", error: "\u2717", info: "\u2139" };

function showToast(message, kind = "info", durationMs = 3500) {
  // Cap visible toasts — remove oldest if at limit
  while (_toastContainer.children.length >= MAX_TOASTS) {
    _toastContainer.firstElementChild.remove();
  }

  const toast = document.createElement("div");
  toast.className = `toast toast-${kind}`;

  const iconSpan = document.createElement("span");
  iconSpan.className = "toast-icon";
  iconSpan.textContent = TOAST_ICONS[kind] || TOAST_ICONS.info;

  const textSpan = document.createElement("span");
  textSpan.textContent = message;

  toast.append(iconSpan, textSpan);
  _toastContainer.appendChild(toast);

  requestAnimationFrame(() => {
    requestAnimationFrame(() => toast.classList.add("visible"));
  });

  setTimeout(() => {
    toast.classList.remove("visible");
    setTimeout(() => toast.remove(), 300);
  }, durationMs);
}

function setStatus(message, isError = false) {
  if (_statusTimer) {
    clearTimeout(_statusTimer);
    _statusTimer = null;
  }

  ui.status.textContent = message;
  _statusBar.classList.remove("status-error", "status-success", "status-working");

  if (isError) {
    _statusBar.classList.add("status-error");
    showToast(message, "error", 5000);
  } else if (message.endsWith("...")) {
    _statusBar.classList.add("status-working");
  } else if (message !== "Ready." && message !== "Refreshed.") {
    _statusBar.classList.add("status-success");
    showToast(message, "success");
    _statusTimer = setTimeout(() => {
      _statusBar.classList.remove("status-success");
    }, 4000);
  }
}

// ── Entry icon (letter avatar) ─────────────────────

const AVATAR_COLORS = [
  "#4a6fa5", "#6b5b95", "#88b04b", "#d65076",
  "#45b8ac", "#e0a84a", "#5b7065", "#b56357",
  "#6c8ebf", "#9b8ec4", "#5dad8a", "#c75d7e",
];

function avatarColor(text) {
  let hash = 0;
  for (let i = 0; i < text.length; i++) {
    hash = ((hash << 5) - hash + text.charCodeAt(i)) | 0;
  }
  return AVATAR_COLORS[Math.abs(hash) % AVATAR_COLORS.length];
}

function createEntryIcon(label) {
  const clean = stripMnemonic(label) || "?";
  const el = document.createElement("div");
  el.className = "entry-icon";
  el.textContent = clean.charAt(0);
  el.style.background = avatarColor(clean);
  return el;
}

async function invoke(command, payload = {}) {
  const globalInvoke = window.__TAURI__?.core?.invoke;
  const internalInvoke = window.__TAURI_INTERNALS__?.invoke;
  const invokeFn = globalInvoke || internalInvoke;

  if (!invokeFn) {
    throw new Error("Tauri runtime not found. Run this app through Tauri.");
  }
  return invokeFn(command, payload);
}

async function nativeConfirm(message, title) {
  const api = window.__TAURI__?.dialog;
  if (api?.ask) {
    return api.ask(message, { title, kind: "warning" });
  }
  return window.confirm(message);
}

function stripMnemonic(label) {
  const token = "\u0000";
  return (label || "")
    .replace(/&&/g, token)
    .replace(/&/g, "")
    .replace(new RegExp(token, "g"), "&")
    .trim();
}

function isGuidLike(value) {
  if (!value) {
    return false;
  }

  const trimmed = value.trim().replace(/^\{/, "").replace(/\}$/, "");
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(trimmed);
}

function friendlyState(stateValue) {
  return stateValue === "enabled" ? "Visible" : "Hidden";
}

function friendlyScope(scope) {
  if (scope === "current_user") {
    return "Just for your account";
  }
  if (scope === "classes_root") {
    return "System-wide";
  }
  if (scope === "local_machine") {
    return "Machine-wide";
  }
  return scope;
}

function friendlyContext(appliesTo) {
  const map = {
    directory: "folders",
    directory_background: "folder background",
    drive: "drives",
    file: "files",
    all_filesystem_objects: "files and folders",
    unknown: "other"
  };

  const unique = [...new Set(appliesTo || [])];
  if (unique.length === 0) {
    return "items";
  }

  const extensions = unique.filter((item) => item.startsWith("."));
  if (extensions.length === unique.length) {
    if (extensions.length > 6) {
      return `${extensions.length} file types`;
    }
    return extensions.join(", ");
  }

  const values = unique.map((item) => map[item] || item);
  if (values.length > 4) {
    return `${values.slice(0, 3).join(", ")} +${values.length - 3} more`;
  }

  return values.join(", ");
}

function extractExecutable(command) {
  if (!command) {
    return "";
  }

  const quoted = command.match(/^\s*"([^"]+\.exe)"/i);
  if (quoted) {
    return quoted[1];
  }

  const unquoted = command.match(/^\s*([^\s]+\.exe)\b/i);
  return unquoted ? unquoted[1] : "";
}

function appNameFromCommand(command) {
  if (isGuidLike(command)) {
    return "";
  }

  const exePath = extractExecutable(command);
  if (!exePath) {
    return "";
  }

  const file = exePath.replace(/^.*[\\/]/, "");
  return file.replace(/\.exe$/i, "");
}

function hasAnyToken(text, tokens) {
  return tokens.some((token) => text.includes(token));
}

function detectCategory(group) {
  const label = (group.label || "").toLowerCase();
  const command = (group.command || "").toLowerCase();
  const keyPaths = group.entries.map((entry) => (entry.key_path || "").toLowerCase()).join(" ");
  const applies = group.applies_to || [];

  const developerTokens = ["git", "bash", "rider", "code", "alacritty", "terminal", "jetbrains", "vscode"];
  if (hasAnyToken(`${label} ${command} ${keyPaths}`, developerTokens)) {
    return "developer";
  }

  const archiveExtensions = new Set([
    ".zip",
    ".7z",
    ".rar",
    ".tar",
    ".gz",
    ".bz2",
    ".xz",
    ".zst",
    ".tgz",
    ".tbz2",
    ".txz",
    ".tzst"
  ]);
  if (
    applies.some((item) => archiveExtensions.has(item.toLowerCase())) ||
    hasAnyToken(label, ["archive", "zip", "compressed"])
  ) {
    return "archive";
  }

  const mediaExtensions = new Set([
    ".mp4",
    ".mkv",
    ".mov",
    ".avi",
    ".webm",
    ".m4v",
    ".mp3",
    ".wav",
    ".flac",
    ".aac",
    ".jpg",
    ".jpeg",
    ".png",
    ".gif",
    ".webp",
    ".heic"
  ]);
  if (
    applies.some((item) => mediaExtensions.has(item.toLowerCase())) ||
    hasAnyToken(`${label} ${command}`, ["convert", "media", "video", "audio", "image", "codec"])
  ) {
    return "media";
  }

  if (hasAnyToken(label, ["open with", "print", "share", "rename", "properties", "pin", "send"])) {
    return "file_ops";
  }

  if (group.entries.some((entry) => entry.key_path.toLowerCase().includes("\\shellex\\contextmenuhandlers\\"))) {
    return "shell_ext";
  }

  if (isAdvancedEntry(group.primary)) {
    return "system";
  }

  return "other";
}

function isAdvancedEntry(entry) {
  const keyPath = (entry.key_path || "").toLowerCase();
  const command = (entry.command || "").toLowerCase();
  const label = (entry.label || "").toLowerCase();
  const isCurrentUserEntry = entry.scope === "current_user";
  const isUserClassesPath = keyPath.startsWith("hkcu\\software\\classes\\");

  // User-level shell entries should stay visible in simple mode, even when they use cmd/powershell wrappers.
  if (isCurrentUserEntry && isUserClassesPath && !keyPath.includes("\\shellex\\contextmenuhandlers\\")) {
    return false;
  }

  const builtInKeyHints = [
    "directory\\background\\shell\\cmd",
    "directory\\shell\\cmd",
    "drive\\shell\\cmd",
    "directory\\background\\shell\\powershell",
    "directory\\shell\\powershell",
    "drive\\shell\\powershell",
    "directory\\background\\shell\\wsl",
    "directory\\shell\\wsl",
    "drive\\shell\\wsl",
    "directory\\shell\\find",
    "drive\\shell\\find",
    "drive\\shell\\pintohome",
    "drive\\shell\\unlock-bde",
    "drive\\shell\\encrypt-bde",
    "drive\\shell\\encrypt-bde-elev",
    "drive\\shell\\resume-bde",
    "drive\\shell\\resume-bde-elev",
    "drive\\shell\\manage-bde",
    "drive\\shell\\change-pin",
    "drive\\shell\\change-passphrase",
    "directory\\shell\\updateencryptionsettings",
    "directory\\shell\\anycode",
    "directory\\background\\shell\\anycode"
  ];

  if (builtInKeyHints.some((hint) => keyPath.includes(hint))) {
    return true;
  }

  const dllResourceLabelPattern = /^[a-z0-9_. -]+\.dll,\s*-?\d+$/i;
  if (dllResourceLabelPattern.test(label)) {
    return true;
  }

  if (isGuidLike(entry.label) && (!entry.command || isGuidLike(entry.command))) {
    return true;
  }

  if (command.includes("%systemroot%") || command.includes("\\windows\\system32")) {
    return true;
  }

  if (command.includes("cmd.exe") || command.includes("powershell.exe") || command.includes("wsl.exe")) {
    return true;
  }

  if (command.includes("explorer.exe") || command.includes("bitlocker")) {
    return true;
  }

  return false;
}

function scopeRank(scope) {
  if (scope === "current_user") {
    return 0;
  }
  if (scope === "classes_root") {
    return 1;
  }
  return 2;
}

function groupEntries(entries) {
  const groups = new Map();

  for (const entry of entries) {
    const key = [
      stripMnemonic((entry.label || "").toLowerCase()),
      (entry.command || "").trim().toLowerCase(),
      entry.state
    ].join("|");

    const existing = groups.get(key);
    if (!existing) {
      groups.set(key, {
        key,
        label: stripMnemonic(entry.label),
        state: entry.state,
        command: entry.command,
        entries: [entry],
        appliesSet: new Set(entry.applies_to || []),
        primary: entry
      });
      continue;
    }

    existing.entries.push(entry);
    for (const applies of entry.applies_to || []) {
      existing.appliesSet.add(applies);
    }

    if (scopeRank(entry.scope) < scopeRank(existing.primary.scope)) {
      existing.primary = entry;
    }
  }

  return [...groups.values()]
    .map((group) => ({
      ...group,
      applies_to: [...group.appliesSet].sort()
    }))
    .map((group) => ({
      ...group,
      category: detectCategory(group)
    }))
    .sort((a, b) => a.label.localeCompare(b.label));
}

function entryMatchesSearch(entry) {
  if (!state.searchTerm) {
    return true;
  }

  const text = [
    stripMnemonic(entry.label),
    entry.command || "",
    entry.key_path || "",
    (entry.applies_to || []).join(" ")
  ]
    .join(" ")
    .toLowerCase();

  return text.includes(state.searchTerm);
}

function getBaseEntries() {
  return state.entries.filter((entry) => {
    if (!state.showAdvanced && isAdvancedEntry(entry)) {
      return false;
    }
    return true;
  });
}

function filterGroupsByCategory(groups) {
  if (state.selectedCategory === "all") {
    return groups;
  }
  return groups.filter((group) => group.category === state.selectedCategory);
}

function renderCategoryFilters(groups) {
  const counts = new Map();
  for (const group of groups) {
    counts.set(group.category, (counts.get(group.category) || 0) + 1);
  }

  if (state.selectedCategory !== "all" && !counts.has(state.selectedCategory)) {
    state.selectedCategory = "all";
  }

  const order = [
    "all",
    "developer",
    "media",
    "archive",
    "file_ops",
    "shell_ext",
    "system",
    "other"
  ];

  ui.categoryFilters.innerHTML = "";
  for (const key of order) {
    const count = key === "all" ? groups.length : counts.get(key) || 0;
    if (count === 0 && key !== "all") {
      continue;
    }

    const button = document.createElement("button");
    button.type = "button";
    button.className = `category-chip${state.selectedCategory === key ? " active" : ""}`;
    button.textContent = `${CATEGORY_META[key] || key} (${count})`;
    button.onclick = () => {
      state.selectedCategory = key;
      renderEntries();
      renderSuggestions();
    };
    ui.categoryFilters.appendChild(button);
  }
}

function getVisibleEntryGroups() {
  return filterGroupsByCategory(getSearchedEntryGroups());
}

function getAllEntryGroupsForSummary() {
  return getSearchedEntryGroups();
}

function getSearchedEntryGroups() {
  const base = getBaseEntries().filter(entryMatchesSearch);
  return groupEntries(base);
}

function getVisibleSuggestions() {
  const suggestions = state.suggestions;
  return suggestions.filter((change) => {
    const entry = change.after || change.before;
    if (!entry) {
      return true;
    }
    if (!state.showAdvanced && isAdvancedEntry(entry)) {
      return false;
    }
    if (!entryMatchesSearch(entry)) {
      return false;
    }

    if (state.selectedCategory !== "all") {
      const category = detectCategory({
        label: stripMnemonic(entry.label),
        command: entry.command,
        entries: [entry],
        applies_to: entry.applies_to || [],
        primary: entry
      });
      return category === state.selectedCategory;
    }

    return true;
  });
}

function buildToggleChanges(entries, targetEnabled) {
  return entries.map((entry, index) => ({
    id: `ui-${Date.now()}-${index}`,
    kind: targetEnabled ? "enable" : "disable",
    before: entry,
    after: {
      ...entry,
      state: targetEnabled ? "enabled" : "disabled"
    },
    risk_level: "low",
    reason: `User requested ${targetEnabled ? "show" : "hide"}`
  }));
}

function buildRemoveChanges(entries) {
  return entries.map((entry, index) => ({
    id: `ui-remove-${Date.now()}-${index}`,
    kind: "remove",
    before: entry,
    after: null,
    risk_level: "medium",
    reason: "User requested delete"
  }));
}

function canDeleteEntry(entry) {
  const keyPath = (entry.key_path || "").toLowerCase();
  return (
    entry.scope === "current_user" &&
    keyPath.startsWith("hkcu\\software\\classes\\") &&
    keyPath.includes("\\shell\\")
  );
}

function canDeleteGroup(group) {
  if (state.showAdvanced) {
    return group.entries.length > 0;
  }
  return group.entries.length > 0 && group.entries.every(canDeleteEntry);
}

function buildDeleteWarning(group) {
  const entryCount = group.entries.length;
  if (state.showAdvanced) {
    return (
      `Delete "${group.label}" from the context menu?\n\n` +
      `This will remove ${entryCount} registry entr${entryCount === 1 ? "y" : "ies"}.\n` +
      "Advanced mode can include system/global actions. Deleting these may break built-in menu behavior and could require manual registry recovery.\n\n" +
      "Continue only if you are sure."
    );
  }

  return (
    `Delete "${group.label}" from the context menu?\n\n` +
    `This removes ${entryCount} registry entr${entryCount === 1 ? "y" : "ies"} (rollback remains available).`
  );
}

async function toggleGroup(group) {
  const targetEnabled = group.state !== "enabled";
  const label = group.label || "Selected action";

  if (group.entries.length === 1) {
    await invoke("toggle_entry", {
      id: group.entries[0].id,
      enabled: targetEnabled
    });
    return;
  }

  const changes = buildToggleChanges(group.entries, targetEnabled);
  setStatus(`${targetEnabled ? "Showing" : "Hiding"} ${label} across ${group.entries.length} entries...`);
  await invoke("apply_changes", { changes });
}

async function deleteGroup(group) {
  const label = group.label || "Selected action";
  const changes = buildRemoveChanges(group.entries);
  setStatus(`Deleting ${label}...`);
  await invoke("apply_changes", { changes });
}

function renderEntries() {
  ui.entriesList.innerHTML = "";
  const groups = getVisibleEntryGroups();
  const allGroups = getAllEntryGroupsForSummary();
  renderCategoryFilters(allGroups);

  const selectedLabel = CATEGORY_META[state.selectedCategory] || state.selectedCategory;
  ui.entriesSummary.textContent =
    state.selectedCategory === "all"
      ? `Showing ${groups.length} of ${allGroups.length} actions`
      : `Showing ${groups.length} of ${allGroups.length} actions in ${selectedLabel}`;

  if (groups.length === 0) {
    ui.entriesList.innerHTML = "<small>No actions match your current view.</small>";
    return;
  }

  for (const group of groups) {
    const row = document.createElement("div");
    row.className = "entry";

    const icon = createEntryIcon(group.label);

    const left = document.createElement("div");
    left.className = "entry-info";
    const title = document.createElement("strong");
    title.textContent = group.label;

    const meta = document.createElement("small");
    meta.textContent = `${friendlyState(group.state)} on ${friendlyContext(group.applies_to)}`;

    const app = appNameFromCommand(group.command);
    const detail = document.createElement("small");
    if (app) {
      detail.textContent = `Opens with ${app}`;
    } else if (group.entries.some((entry) => entry.key_path.toLowerCase().includes("\\shellex\\contextmenuhandlers\\"))) {
      detail.textContent = "Windows extension action";
    } else {
      detail.textContent = "Custom shell action";
    }

    left.append(title, meta, detail);

    const categoryBadge = document.createElement("span");
    categoryBadge.className = "category-badge";
    categoryBadge.textContent = CATEGORY_META[group.category] || "Other";
    left.append(categoryBadge);

    if (group.entries.length > 1) {
      const groupedCount = document.createElement("small");
      groupedCount.textContent = `${group.entries.length} linked registry entries`;
      left.append(groupedCount);
    }

    if (state.showAdvanced) {
      const advancedMeta = document.createElement("small");
      advancedMeta.textContent = `${friendlyScope(group.primary.scope)} | ${group.primary.scope}`;

      const raw = document.createElement("pre");
      raw.textContent = `${group.primary.key_path}${group.primary.command ? `\n${group.primary.command}` : ""}${
        group.entries.length > 1 ? `\n(+${group.entries.length - 1} more keys)` : ""
      }`;
      left.append(advancedMeta, raw);
    }

    const actions = document.createElement("div");
    actions.className = "entry-actions";

    const toggle = document.createElement("button");
    const isEnabled = group.state === "enabled";
    toggle.className = isEnabled ? "button warn" : "button";
    toggle.textContent = isEnabled ? "Hide" : "Show";
    toggle.onclick = async () => {
      try {
        setStatus(`${isEnabled ? "Hiding" : "Showing"} ${group.label}...`);
        await toggleGroup(group);
        await refreshAll();
        setStatus(`Updated: ${group.label}`);
      } catch (error) {
        setStatus(error.message || String(error), true);
      }
    };

    actions.appendChild(toggle);

    if (canDeleteGroup(group)) {
      const remove = document.createElement("button");
      remove.className = "button danger";
      remove.textContent = "Delete";
      remove.onclick = async () => {
        const confirmed = await nativeConfirm(buildDeleteWarning(group), "Delete Action");
        if (!confirmed) {
          return;
        }

        try {
          await deleteGroup(group);
          await refreshAll();
          setStatus(`Deleted: ${group.label}`);
        } catch (error) {
          setStatus(error.message || String(error), true);
        }
      };
      actions.appendChild(remove);
    }

    row.append(icon, left, actions);
    ui.entriesList.appendChild(row);
  }
}

function renderSuggestions() {
  ui.suggestionsList.innerHTML = "";
  const suggestions = getVisibleSuggestions();
  ui.applySuggestionsBtn.disabled = suggestions.length === 0;

  if (suggestions.length === 0) {
    ui.suggestionsList.innerHTML = "<small>No recommended actions for this view.</small>";
    return;
  }

  for (const change of suggestions) {
    const row = document.createElement("div");
    row.className = "entry";

    const entry = change.after || change.before;
    const label = entry ? stripMnemonic(entry.label) : "Suggested action";
    const left = document.createElement("div");
    const title = document.createElement("strong");
    title.textContent = label;
    const reason = document.createElement("small");
    reason.textContent = change.reason;
    left.append(title, reason);
    row.appendChild(left);

    ui.suggestionsList.appendChild(row);
  }
}

function renderCustomChanges() {
  const hasChanges = state.customChanges.length > 0;
  ui.pendingCard.hidden = !hasChanges;
  ui.applyCustomBtn.disabled = !hasChanges;
  ui.customChangesList.innerHTML = "";

  if (!hasChanges) {
    return;
  }

  const count = state.customChanges.length;
  ui.pendingCount.textContent = `${count} registry ${count === 1 ? "entry" : "entries"} will be created`;

  for (const change of state.customChanges) {
    const row = document.createElement("div");
    row.className = "entry";
    row.innerHTML = `
      <div>
        <strong>${stripMnemonic(change.after.label)}</strong>
        <small>Shows on ${friendlyContext(change.after.applies_to)}</small>
        ${state.showAdvanced ? `<pre>${change.after.key_path}\n${change.after.command || ""}</pre>` : ""}
      </div>
    `;
    ui.customChangesList.appendChild(row);
  }
}

function formatChangeCount(count) {
  return `${count} ${count === 1 ? "change" : "changes"}`;
}

function changeKindLabel(kind) {
  if (kind === "add") {
    return "Added";
  }
  if (kind === "remove") {
    return "Deleted";
  }
  if (kind === "disable") {
    return "Hidden";
  }
  if (kind === "enable") {
    return "Shown";
  }
  return kind;
}

function shortChangeSetId(id) {
  if (!id) {
    return "unknown";
  }
  return id.length <= 8 ? id : id.slice(0, 8);
}

function formatRelativeTime(date) {
  const diffMs = date.getTime() - Date.now();
  const absMs = Math.abs(diffMs);
  const units = [
    { name: "day", ms: 24 * 60 * 60 * 1000 },
    { name: "hour", ms: 60 * 60 * 1000 },
    { name: "minute", ms: 60 * 1000 },
    { name: "second", ms: 1000 }
  ];

  for (const unit of units) {
    if (absMs >= unit.ms || unit.name === "second") {
      const value = Math.round(diffMs / unit.ms);
      return new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(value, unit.name);
    }
  }

  return "just now";
}

async function copyToClipboard(text) {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const temp = document.createElement("textarea");
  temp.value = text;
  temp.setAttribute("readonly", "");
  temp.style.position = "fixed";
  temp.style.opacity = "0";
  document.body.appendChild(temp);
  temp.select();
  const copied = document.execCommand("copy");
  document.body.removeChild(temp);

  if (!copied) {
    throw new Error("Unable to copy to clipboard");
  }
}

function renderChangeSetDetails(changeSetId, container) {
  const details = state.changeSetDetails[changeSetId];
  if (!details) {
    container.innerHTML = "<small>Loading changes...</small>";
    return;
  }

  if (!details.changes || details.changes.length === 0) {
    container.innerHTML = "<small>No recorded changes in this set.</small>";
    return;
  }

  const list = document.createElement("div");
  list.className = "change-detail-list";

  for (const change of details.changes) {
    const entry = change.after || change.before;
    const item = document.createElement("div");
    item.className = "change-detail-item";

    const title = document.createElement("strong");
    title.textContent = `${changeKindLabel(change.kind)}: ${stripMnemonic(entry?.label || "Unnamed action")}`;

    const context = document.createElement("small");
    context.textContent = `Visible on ${friendlyContext(entry?.applies_to || [])}`;

    const reason = document.createElement("small");
    reason.textContent = `Reason: ${change.reason || "User action"}`;

    item.append(title, context, reason);

    if (entry?.key_path) {
      const keyPath = document.createElement("small");
      keyPath.textContent = "Registry key: ";
      const keyPathValue = document.createElement("span");
      keyPathValue.className = "mono";
      keyPathValue.textContent = entry.key_path;
      keyPath.appendChild(keyPathValue);
      item.appendChild(keyPath);
    }

    list.appendChild(item);
  }

  container.innerHTML = "";
  container.appendChild(list);
}

function renderChangeSets() {
  ui.changeSetsList.innerHTML = "";
  if (state.changeSets.length === 0) {
    ui.changeSetsList.innerHTML = "<small>No saved change sets yet.</small>";
    return;
  }

  for (const changeSet of state.changeSets) {
    const row = document.createElement("div");
    row.className = "entry change-set-entry";

    const left = document.createElement("div");
    const createdAt = new Date(changeSet.created_at);
    const absoluteTime = createdAt.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
    const relativeTime = formatRelativeTime(createdAt);

    const title = document.createElement("strong");
    title.textContent = `Change Set ${shortChangeSetId(changeSet.id)}`;

    const actionCount = document.createElement("small");
    actionCount.textContent = `Actions: ${formatChangeCount(changeSet.change_count)}`;

    const created = document.createElement("small");
    created.textContent = `Created: ${absoluteTime} (${relativeTime})`;

    const idLine = document.createElement("small");
    idLine.textContent = "ID: ";
    const idValue = document.createElement("span");
    idValue.className = "mono";
    idValue.textContent = changeSet.id;
    idLine.appendChild(idValue);

    left.append(title, actionCount, created, idLine);

    const actions = document.createElement("div");
    actions.className = "entry-actions";

    const copyId = document.createElement("button");
    copyId.className = "button ghost";
    copyId.textContent = "Copy ID";
    copyId.onclick = async () => {
      try {
        await copyToClipboard(changeSet.id);
        setStatus(`Copied change set ID: ${shortChangeSetId(changeSet.id)}`);
      } catch (error) {
        setStatus(error.message || String(error), true);
      }
    };

    const detailsButton = document.createElement("button");
    detailsButton.className = "button ghost";
    const isExpanded = !!state.expandedChangeSets[changeSet.id];
    detailsButton.textContent = isExpanded ? "Hide Changes" : "What Changed";
    detailsButton.onclick = async () => {
      const currentlyExpanded = !!state.expandedChangeSets[changeSet.id];
      state.expandedChangeSets[changeSet.id] = !currentlyExpanded;

      if (!currentlyExpanded && !state.changeSetDetails[changeSet.id]) {
        try {
          const details = await invoke("get_change_set", { changeSetId: changeSet.id });
          state.changeSetDetails[changeSet.id] = details;
        } catch (error) {
          state.expandedChangeSets[changeSet.id] = false;
          setStatus(error.message || String(error), true);
        }
      }

      renderChangeSets();
    };

    const button = document.createElement("button");
    button.className = "button danger";
    button.textContent = "Rollback";
    button.onclick = async () => {
      const confirmed = await nativeConfirm(
        `Rollback ${formatChangeCount(changeSet.change_count)} from "${shortChangeSetId(
          changeSet.id
        )}"?\n\nCreated ${absoluteTime}.`,
        "Rollback Changes"
      );
      if (!confirmed) {
        return;
      }

      try {
        setStatus(`Rolling back ${changeSet.id}...`);
        const result = await invoke("rollback", { changeSetId: changeSet.id });
        await refreshAll();
        setStatus(`Rollback complete. Restored ${result.applied.length} keys.`);
      } catch (error) {
        setStatus(error.message || String(error), true);
      }
    };

    actions.append(copyId, detailsButton, button);
    row.append(left, actions);

    if (isExpanded) {
      const details = document.createElement("div");
      details.className = "change-set-details";
      renderChangeSetDetails(changeSet.id, details);
      row.appendChild(details);
    }

    ui.changeSetsList.appendChild(row);
  }
}

async function refreshAll() {
  const [entries, suggestions, changeSets] = await Promise.all([
    invoke("scan_entries"),
    invoke("suggest_actions"),
    invoke("list_change_sets")
  ]);

  state.entries = entries;
  state.suggestions = suggestions;
  state.changeSets = changeSets;

  const validIds = new Set(changeSets.map((changeSet) => changeSet.id));
  for (const id of Object.keys(state.changeSetDetails)) {
    if (!validIds.has(id)) {
      delete state.changeSetDetails[id];
    }
  }
  for (const id of Object.keys(state.expandedChangeSets)) {
    if (!validIds.has(id)) {
      delete state.expandedChangeSets[id];
    }
  }

  renderEntries();
  renderSuggestions();
  renderCustomChanges();
  renderChangeSets();
}

async function applyChanges(changes, successMessage) {
  const result = await invoke("apply_changes", { changes });
  const summary = `${successMessage} Applied: ${result.applied.length}, failed: ${result.failed.length}`;
  await refreshAll();
  setStatus(summary, result.failed.length > 0);
}

ui.refreshBtn.addEventListener("click", async () => {
  try {
    setStatus("Refreshing...");
    await refreshAll();
    setStatus("Refreshed.");
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
});

ui.applySuggestionsBtn.addEventListener("click", async () => {
  try {
    await applyChanges(getVisibleSuggestions(), "Suggestions applied.");
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
});

ui.showAdvancedToggle.addEventListener("change", () => {
  state.showAdvanced = ui.showAdvancedToggle.checked;
  renderEntries();
  renderSuggestions();
  renderCustomChanges();
});

ui.searchInput.addEventListener("input", () => {
  state.searchTerm = ui.searchInput.value.trim().toLowerCase();
  renderEntries();
  renderSuggestions();
});

ui.targetFiles.addEventListener("change", updateAddActionFormState);
ui.argsInput.addEventListener("input", () => {
  updateCommandPreview();
  updateQuickFillHighlight();
});
ui.executableInput.addEventListener("input", updateCommandPreview);

// File type segmented toggle
for (const btn of ui.fileTypeToggle.querySelectorAll(".toggle-option")) {
  btn.addEventListener("click", () => {
    for (const b of ui.fileTypeToggle.querySelectorAll(".toggle-option")) {
      b.classList.toggle("active", b === btn);
    }
    updateAddActionFormState();
  });
}

// Quick-fill argument buttons
for (const btn of ui.quickFillButtons) {
  btn.addEventListener("click", () => {
    ui.argsInput.value = btn.dataset.args;
    ui.argsInput.focus();
    updateCommandPreview();
    updateQuickFillHighlight();
  });
}

// Reset form
ui.resetFormBtn.addEventListener("click", () => resetCreateForm());

// Discard pending changes
ui.discardChangesBtn.addEventListener("click", () => {
  state.customChanges = [];
  renderCustomChanges();
});

ui.browseExecutableBtn.addEventListener("click", async () => {
  try {
    const picked = await invoke("pick_path", { kind: "executable" });
    if (picked) {
      ui.executableInput.value = picked;
      updateCommandPreview();
    }
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
});

ui.browseIconBtn.addEventListener("click", async () => {
  try {
    const picked = await invoke("pick_path", { kind: "icon" });
    if (picked) {
      ui.iconInput.value = picked;
    }
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
});

ui.customForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    const targets = [];
    if (ui.targetFiles.checked) {
      targets.push("files");
    }
    if (ui.targetFolders.checked) {
      targets.push("folders");
    }
    if (ui.targetFolderBackground.checked) {
      targets.push("folder_background");
    }
    if (ui.targetDrives.checked) {
      targets.push("drives");
    }

    const isAllFiles = !ui.targetFiles.checked || getFileTypeMode() === "all";

    const request = {
      label: document.getElementById("label").value.trim(),
      executable_path: ui.executableInput.value.trim(),
      args: ui.argsInput.value.trim(),
      icon_path: ui.iconInput.value.trim() || null,
      targets,
      extensions: isAllFiles ? [] : extensionChips.getValues(),
      apply_to_all_files: isAllFiles,
      verb: null,
      scope: "current_user"
    };

    setStatus("Generating action changes...");
    state.customChanges = await invoke("create_action", { request });
    renderCustomChanges();
    setStatus(`Generated ${state.customChanges.length} action changes.`);
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
});

ui.applyCustomBtn.addEventListener("click", async () => {
  try {
    await applyChanges(state.customChanges, "Action changes applied.");
    state.customChanges = [];
    renderCustomChanges();
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
});

const _navButtons = document.querySelectorAll(".nav-btn[data-panel]");
const _panels = document.querySelectorAll(".panel[id^='panel-']");

function switchPanel(panelId) {
  state.activePanel = panelId;
  _panels.forEach((p) => p.classList.toggle("active", p.id === `panel-${panelId}`));
  _navButtons.forEach((b) => b.classList.toggle("active", b.dataset.panel === panelId));
}

function initNavigation() {
  _navButtons.forEach((btn) => {
    btn.addEventListener("click", () => switchPanel(btn.dataset.panel));
  });

  switchPanel("entries");
}

const extensionChips = new ChipInput(ui.chipContainer, ui.chipInput, () => {});

// ── Keyboard shortcuts ────────────────────────────────

function isTyping() {
  const el = document.activeElement;
  return el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable);
}

document.addEventListener("keydown", (event) => {
  // Ctrl+F → focus search
  if ((event.ctrlKey || event.metaKey) && event.key === "f") {
    event.preventDefault();
    switchPanel("entries");
    ui.searchInput.focus();
    ui.searchInput.select();
    return;
  }

  // Ctrl+R or F5 → refresh
  if (((event.ctrlKey || event.metaKey) && event.key === "r") || event.key === "F5") {
    event.preventDefault();
    ui.refreshBtn.click();
    return;
  }

  // Escape → clear search or blur active input
  if (event.key === "Escape") {
    if (document.activeElement === ui.searchInput && ui.searchInput.value) {
      ui.searchInput.value = "";
      state.searchTerm = "";
      renderEntries();
      renderSuggestions();
      return;
    }
    if (isTyping()) {
      document.activeElement.blur();
      return;
    }
  }

  // 1/2/3 → switch panels (only when not typing)
  if (!isTyping() && !event.ctrlKey && !event.metaKey && !event.altKey) {
    const panelKeys = { "1": "entries", "2": "create", "3": "history" };
    const panel = panelKeys[event.key];
    if (panel) {
      event.preventDefault();
      switchPanel(panel);
    }
  }
});

(async function init() {
  try {
    initNavigation();
    updateAddActionFormState();
    updateCommandPreview();
    updateQuickFillHighlight();
    setStatus("Loading entries...");
    await refreshAll();
    setStatus("Ready.");
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
})();
