const state = {
  entries: [],
  suggestions: [],
  customChanges: [],
  changeSets: [],
  showAdvanced: false
};

const ui = {
  status: document.getElementById("status-text"),
  entriesList: document.getElementById("entries-list"),
  suggestionsList: document.getElementById("suggestions-list"),
  customChangesList: document.getElementById("custom-changes-list"),
  changeSetsList: document.getElementById("change-sets-list"),
  applySuggestionsBtn: document.getElementById("apply-suggestions-btn"),
  applyCustomBtn: document.getElementById("apply-custom-btn"),
  refreshBtn: document.getElementById("refresh-btn"),
  customForm: document.getElementById("custom-form"),
  showAdvancedToggle: document.getElementById("show-advanced-toggle")
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

function isAdvancedEntry(entry) {
  const keyPath = (entry.key_path || "").toLowerCase();
  const command = (entry.command || "").toLowerCase();
  const label = (entry.label || "").toLowerCase();

  const builtInKeyHints = [
    "\\directory\\background\\shell\\cmd",
    "\\directory\\shell\\cmd",
    "\\drive\\shell\\cmd",
    "\\directory\\background\\shell\\powershell",
    "\\directory\\shell\\powershell",
    "\\drive\\shell\\powershell",
    "\\directory\\background\\shell\\wsl",
    "\\directory\\shell\\wsl",
    "\\drive\\shell\\wsl",
    "\\directory\\shell\\find",
    "\\drive\\shell\\find",
    "\\drive\\shell\\pintohome",
    "\\drive\\shell\\unlock-bde",
    "\\drive\\shell\\encrypt-bde",
    "\\drive\\shell\\encrypt-bde-elev",
    "\\drive\\shell\\resume-bde",
    "\\drive\\shell\\resume-bde-elev",
    "\\drive\\shell\\manage-bde",
    "\\drive\\shell\\change-pin",
    "\\drive\\shell\\change-passphrase",
    "\\directory\\shell\\updateencryptionsettings",
    "\\directory\\shell\\anycode",
    "\\directory\\background\\shell\\anycode"
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
      (entry.label || "").trim().toLowerCase(),
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

function getVisibleEntries() {
  const deduped = dedupeEntries(state.entries);
  if (state.showAdvanced) {
    return deduped;
  }
  return deduped.filter((entry) => !isAdvancedEntry(entry));
}

function getVisibleSuggestions() {
  const suggestions = state.suggestions;
  if (state.showAdvanced) {
    return suggestions;
  }

  return suggestions.filter((change) => {
    const entry = change.after || change.before;
    return entry ? !isAdvancedEntry(entry) : true;
  });
}

function renderEntries() {
  ui.entriesList.innerHTML = "";
  const entries = getVisibleEntries();

  if (entries.length === 0) {
    ui.entriesList.innerHTML = "<small>No entries to show with current filter.</small>";
    return;
  }

  for (const entry of entries) {
    const row = document.createElement("div");
    row.className = "entry";

    const left = document.createElement("div");
    left.innerHTML = `
      <strong>${entry.label}</strong>
      <small>${entry.state} | ${entry.scope} | ${entry.applies_to.join(", ")}</small>
      <pre>${entry.key_path}${entry.command ? `\n${entry.command}` : ""}</pre>
    `;

    const toggle = document.createElement("button");
    toggle.className = entry.state === "enabled" ? "button warn" : "button";
    toggle.textContent = entry.state === "enabled" ? "Disable" : "Enable";
    toggle.onclick = async () => {
      try {
        setStatus(`Applying ${entry.state === "enabled" ? "disable" : "enable"} for ${entry.label}...`);
        await invoke("toggle_entry", { id: entry.id, enabled: entry.state !== "enabled" });
        await refreshAll();
        setStatus(`Updated: ${entry.label}`);
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
    ui.suggestionsList.innerHTML = "<small>No automatic recommendations right now.</small>";
    return;
  }

  for (const change of suggestions) {
    const row = document.createElement("div");
    row.className = "entry";
    row.innerHTML = `
      <div>
        <strong>${change.reason}</strong>
        <small>${change.kind} | risk: ${change.risk_level}</small>
        <pre>${change.after?.key_path || change.before?.key_path || ""}</pre>
      </div>
    `;
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
        <strong>${change.after.label}</strong>
        <small>${change.after.applies_to.join(", ")} | risk: ${change.risk_level}</small>
        <pre>${change.after.key_path}\n${change.after.command || ""}</pre>
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
