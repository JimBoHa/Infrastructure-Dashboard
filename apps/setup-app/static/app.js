const panels = Array.from(document.querySelectorAll(".panel"));
const indicators = Array.from(document.querySelectorAll(".step"));
const prevBtn = document.getElementById("prev-step");
const nextBtn = document.getElementById("next-step");
const preflightBtn = document.getElementById("run-preflight");
const planBtn = document.getElementById("generate-plan");
const preflightResults = document.getElementById("preflight-results");
const planOutput = document.getElementById("plan-output");
const installOutput = document.getElementById("install-output");
const form = document.getElementById("config-form");
const installBtn = document.getElementById("run-install");
const upgradeBtn = document.getElementById("run-upgrade");
const rollbackBtn = document.getElementById("run-rollback");
const healthBtn = document.getElementById("run-health");
const diagnosticsBtn = document.getElementById("export-diagnostics");
const toggleAdvancedBtn = document.getElementById("toggle-advanced");
const mqttDetectBtn = document.getElementById("detect-mqtt-host");

let currentStep = 0;

const SAVE_FIELDS = new Set([
  "bundle_path",
  "install_root",
  "data_root",
  "logs_root",
  "backup_root",
  "core_port",
  "mqtt_host",
  "mqtt_port",
  "redis_port",
  "database_url",
  "backup_retention_days",
]);

const showStep = (index) => {
  currentStep = Math.max(0, Math.min(index, panels.length - 1));
  panels.forEach((panel, idx) => {
    panel.classList.toggle("active", idx === currentStep);
  });
  indicators.forEach((indicator, idx) => {
    indicator.classList.toggle("active", idx === currentStep);
  });
  prevBtn.disabled = currentStep === 0;
  nextBtn.textContent = currentStep === panels.length - 1 ? "Done" : "Next";
};

const formPayload = (allowedFields = null) => {
  const data = new FormData(form);
  const payload = {};
  for (const [key, value] of data.entries()) {
    if (allowedFields && !allowedFields.has(key)) continue;
    if (value === "") continue;
    if (
      ["core_port", "mqtt_port", "redis_port", "backup_retention_days"].includes(key)
    ) {
      payload[key] = Number(value);
    } else {
      payload[key] = value;
    }
  }
  return payload;
};

const loadConfig = async () => {
  const response = await fetch("/api/config", { cache: "no-store" });
  const data = await response.json();
  if (!response.ok) {
    throw new Error(data?.error || "Failed to load config");
  }
  populateForm(data);
  return data;
};

const populateForm = (config) => {
  Object.entries(config).forEach(([key, value]) => {
    const field = form.elements.namedItem(key);
    if (field) {
      field.value = value ?? "";
    }
  });
};

const saveConfig = async () => {
  const payload = formPayload(SAVE_FIELDS);
  const response = await fetch("/api/config", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  const data = await response.json();
  if (!response.ok) {
    throw new Error(data?.error || "Failed to save config");
  }
  populateForm(data);
  return data;
};

const renderChecks = (checks) => {
  preflightResults.innerHTML = "";
  checks.forEach((check) => {
    const row = document.createElement("div");
    row.className = "check";
    row.innerHTML = `
      <div>
        <strong>${check.id.replace(/-/g, " ")}</strong>
        <span>${check.message}</span>
      </div>
      <span class="badge ${check.status}">${check.status}</span>
    `;
    preflightResults.appendChild(row);
  });
};

const renderPlan = (plan) => {
  const warnings = plan.warnings?.length
    ? `<div class="check"><div><strong>Warnings</strong><span>${plan.warnings.join(
        " | "
      )}</span></div><span class="badge warn">warn</span></div>`
    : "";
  const commands = plan.commands?.length ? plan.commands.join("\n") : "No commands generated.";
  const loadCommands = plan.load_commands?.length
    ? plan.load_commands.join("\n")
    : "No launchctl commands generated.";

  planOutput.innerHTML = `
    ${warnings}
    <div class="check">
      <div>
        <strong>Staging directory</strong>
        <span>${plan.staging_dir}</span>
      </div>
      <span class="badge ok">ready</span>
    </div>
    <pre>${commands}</pre>
    <pre>${loadCommands}</pre>
  `;
};

const renderCommandResults = (title, results = []) => {
  if (!installOutput) return;
  const rows =
    results.length === 0
      ? `<p class="muted">No output captured.</p>`
      : results
          .map(
            (item) => `
      <div class="check">
        <div>
          <strong>${item.command}</strong>
          <span>${item.stdout || item.stderr || "No output"}</span>
        </div>
        <span class="badge ${item.ok ? "ok" : "error"}">${item.ok ? "ok" : "error"}</span>
      </div>
    `
          )
          .join("");
  installOutput.innerHTML = `
    <div class="check">
      <div>
        <strong>${title}</strong>
        <span>Latest installer activity.</span>
      </div>
      <span class="badge ok">ready</span>
    </div>
    ${rows}
  `;
};

prevBtn.addEventListener("click", () => showStep(currentStep - 1));
nextBtn.addEventListener("click", () => showStep(currentStep + 1));

preflightBtn.addEventListener("click", async () => {
  preflightResults.innerHTML = "<p class=\"muted\">Running checks...</p>";
  try {
    await saveConfig();
  } catch (err) {
    preflightResults.innerHTML = `<p class="muted">Unable to save config: ${err?.message || err}</p>`;
    return;
  }
  const response = await fetch("/api/preflight");
  const data = await response.json();
  renderChecks(data.checks || []);
});

planBtn.addEventListener("click", async () => {
  planOutput.innerHTML = "<p class=\"muted\">Generating plan...</p>";
  try {
    await saveConfig();
  } catch (err) {
    planOutput.innerHTML = `<p class="muted">Unable to save config: ${err?.message || err}</p>`;
    return;
  }
  const response = await fetch("/api/plan", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({}),
  });
  const data = await response.json();
  renderPlan(data);
});

const runInstallerAction = async (endpoint, title) => {
  installOutput.innerHTML = "<p class=\"muted\">Running...</p>";
  try {
    await saveConfig();
  } catch (err) {
    installOutput.innerHTML = `<p class="muted">Unable to save config: ${err?.message || err}</p>`;
    return;
  }
  const response = await fetch(endpoint, { method: "POST" });
  const data = await response.json();
  const results = [...(data.farmctl || []), ...(data.launchd || [])];
  renderCommandResults(title, results);
  try {
    await loadConfig();
  } catch {
    // ignore; action output is more important than config refresh here
  }
  if (data.handoff) {
    installOutput.insertAdjacentHTML(
      "beforeend",
      "<p class=\"muted\">Restarting Setup daemon (handoff to launchd)...</p>",
    );
    await waitForHandoff();
  }
};

installBtn.addEventListener("click", () => runInstallerAction("/api/install", "Install"));
upgradeBtn.addEventListener("click", () => runInstallerAction("/api/upgrade", "Upgrade"));
rollbackBtn.addEventListener("click", () => runInstallerAction("/api/rollback", "Rollback"));

healthBtn.addEventListener("click", async () => {
  installOutput.innerHTML = "<p class=\"muted\">Checking health...</p>";
  const response = await fetch("/api/health-report");
  const data = await response.json();
  const report = data.report || {};
  const rows = ["core_api", "dashboard", "mqtt", "database", "redis"]
    .map((key) => report[key] || { status: "unknown", message: "No data" })
    .map(
      (entry) => `
    <div class="check">
      <div>
        <strong>${entry.message}</strong>
        <span>${entry.status}</span>
      </div>
      <span class="badge ${entry.status === "ok" ? "ok" : "error"}">${entry.status}</span>
    </div>
  `
    )
    .join("");
  installOutput.innerHTML = rows || "<p class=\"muted\">No health data returned.</p>";
});

diagnosticsBtn.addEventListener("click", async () => {
  installOutput.innerHTML = "<p class=\"muted\">Exporting diagnostics...</p>";
  const response = await fetch("/api/diagnostics", { method: "POST" });
  const data = await response.json();
  renderCommandResults("Diagnostics", data.logs || []);
});

toggleAdvancedBtn.addEventListener("click", () => {
  const enabled = form.dataset.advanced === "true";
  form.dataset.advanced = enabled ? "false" : "true";
  toggleAdvancedBtn.textContent = enabled ? "Show advanced" : "Hide advanced";
});

if (mqttDetectBtn) {
  mqttDetectBtn.addEventListener("click", async () => {
    const field = form.elements.namedItem("mqtt_host");
    if (!field) return;
    mqttDetectBtn.disabled = true;
    const originalText = mqttDetectBtn.textContent;
    mqttDetectBtn.textContent = "Detecting...";
    try {
      const response = await fetch("/api/local-ip");
      const data = await response.json();
      const recommended = data?.recommended;
      if (typeof recommended === "string" && recommended.trim()) {
        field.value = recommended.trim();
        await saveConfig();
      }
    } catch {
      // ignore; leave the field unchanged
    } finally {
      mqttDetectBtn.disabled = false;
      mqttDetectBtn.textContent = originalText;
    }
  });
}

loadConfig().catch(() => null);

showStep(0);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const waitForHandoff = async () => {
  const deadline = Date.now() + 20000;
  let sawDown = false;
  while (Date.now() < deadline) {
    try {
      const res = await fetch("/healthz", { cache: "no-store" });
      if (res.ok && sawDown) {
        window.location.reload();
        return;
      }
      if (res.ok) {
        await sleep(350);
        continue;
      }
    } catch {
      sawDown = true;
    }
    await sleep(350);
  }
};
