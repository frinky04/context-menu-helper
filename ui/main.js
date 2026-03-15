const state = {
  entries: [],
  suggestions: [],
  customChanges: [],
  changeSets: [],
  showAdvanced: false,
  searchTerm: ""
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
  searchInput: document.getElementById("search-input")
};

ui.showAdvancedToggle.checked = state.showAdvanced;

function setStatus(message, isError = false) {
  ui.status.textContent = message;
  ui.status.style.color = isError ? "#ff9b9b" : "#8bd1ac";
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

function stripMnemonic(label) {
  const token = "\u0000";
  return (label || "")
    .replace(/&&/g, token)
    .replace(/&/g, "")
    .replace(new RegExp(token, "g"), "&")
    .trim();
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
    unknown: "other"
  };

  const values = (appliesTo || []).map((item) => map[item] || item);
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
  const exePath = extractExecutable(command);
  if (!exePath) {
    return "";
  }

  const file = exePath.replace(/^.*[\\/]/, "");
  return file.replace(/\.exe$/i, "");
}

function isAdvancedEntry(entry) {
  const keyPath = (entry.key_path || "").toLowerCase();
  const command = (entry.command || "").toLowerCase();
  const label = (entry.label || "").toLowerCase();

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

function dedupeEntries(entries) {
  const bySignature = new Map();

  for (const entry of entries) {
    const applies = [...(entry.applies_to || [])].sort().join(",");
    const signature = [
      stripMnemonic((entry.label || "").toLowerCase()),
      applies,
      (entry.command || "").trim().toLowerCase()
    ].join("|");

    const current = bySignature.get(signature);
    if (!current || scopeRank(entry.scope) < scopeRank(current.scope)) {
      bySignature.set(signature, entry);
    }
  }

  return [...bySignature.values()];
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

function getVisibleEntries() {
  const deduped = dedupeEntries(state.entries);

  return deduped.filter((entry) => {
    if (!state.showAdvanced && isAdvancedEntry(entry)) {
      return false;
    }
    return entryMatchesSearch(entry);
  });
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
    return entryMatchesSearch(entry);
  });
}

function renderEntries() {
  ui.entriesList.innerHTML = "";
  const entries = getVisibleEntries();
  const total = dedupeEntries(state.entries).length;

  ui.entriesSummary.textContent = `Showing ${entries.length} of ${total} actions`;

  if (entries.length === 0) {
    ui.entriesList.innerHTML = "<small>No actions match your current view.</small>";
    return;
  }

  for (const entry of entries) {
    const row = document.createElement("div");
    row.className = "entry";

    const left = document.createElement("div");
    const title = document.createElement("strong");
    title.textContent = stripMnemonic(entry.label);

    const meta = document.createElement("small");
    meta.textContent = `${friendlyState(entry.state)} on ${friendlyContext(entry.applies_to)}`;

    const app = appNameFromCommand(entry.command);
    const detail = document.createElement("small");
    detail.textContent = app ? `Opens with ${app}` : "Custom shell action";

    left.append(title, meta, detail);

    if (state.showAdvanced) {
      const advancedMeta = document.createElement("small");
      advancedMeta.textContent = `${friendlyScope(entry.scope)} | ${entry.scope}`;

      const raw = document.createElement("pre");
      raw.textContent = `${entry.key_path}${entry.command ? `\n${entry.command}` : ""}`;
      left.append(advancedMeta, raw);
    }

    const toggle = document.createElement("button");
    const isEnabled = entry.state === "enabled";
    toggle.className = isEnabled ? "button warn" : "button";
    toggle.textContent = isEnabled ? "Hide" : "Show";
    toggle.onclick = async () => {
      try {
        setStatus(`${isEnabled ? "Hiding" : "Showing"} ${stripMnemonic(entry.label)}...`);
        await invoke("toggle_entry", { id: entry.id, enabled: !isEnabled });
        await refreshAll();
        setStatus(`Updated: ${stripMnemonic(entry.label)}`);
      } catch (error) {
        setStatus(error.message || String(error), true);
      }
    };

    row.append(left, toggle);
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
  ui.customChangesList.innerHTML = "";
  ui.applyCustomBtn.disabled = state.customChanges.length === 0;

  if (state.customChanges.length === 0) {
    ui.customChangesList.innerHTML = "<small>No generated custom changes yet.</small>";
    return;
  }

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

function renderChangeSets() {
  ui.changeSetsList.innerHTML = "";
  if (state.changeSets.length === 0) {
    ui.changeSetsList.innerHTML = "<small>No saved change sets yet.</small>";
    return;
  }

  for (const changeSet of state.changeSets) {
    const row = document.createElement("div");
    row.className = "entry";

    const createdAt = new Date(changeSet.created_at).toLocaleString();
    const left = document.createElement("div");
    left.innerHTML = `
      <strong>${changeSet.id}</strong>
      <small>${changeSet.change_count} changes | ${createdAt}</small>
    `;

    const button = document.createElement("button");
    button.className = "button danger";
    button.textContent = "Rollback";
    button.onclick = async () => {
      try {
        setStatus(`Rolling back ${changeSet.id}...`);
        const result = await invoke("rollback", { change_set_id: changeSet.id });
        await refreshAll();
        setStatus(`Rollback complete. Restored ${result.applied.length} keys.`);
      } catch (error) {
        setStatus(error.message || String(error), true);
      }
    };

    row.append(left, button);
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

ui.customForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    const payload = {
      label: document.getElementById("label").value.trim(),
      executable_path: document.getElementById("executable").value.trim(),
      args: document.getElementById("args").value.trim(),
      icon_path: document.getElementById("icon").value.trim() || null,
      extensions: document
        .getElementById("extensions")
        .value.split(",")
        .map((x) => x.trim())
        .filter(Boolean),
      verb: null,
      scope: "current_user"
    };

    setStatus("Generating custom changes...");
    state.customChanges = await invoke("create_custom_entry", { payload });
    renderCustomChanges();
    setStatus(`Generated ${state.customChanges.length} custom changes.`);
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
});

ui.applyCustomBtn.addEventListener("click", async () => {
  try {
    await applyChanges(state.customChanges, "Custom changes applied.");
    state.customChanges = [];
    renderCustomChanges();
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
});

(async function init() {
  try {
    setStatus("Loading entries...");
    await refreshAll();
    setStatus("Ready.");
  } catch (error) {
    setStatus(error.message || String(error), true);
  }
})();
