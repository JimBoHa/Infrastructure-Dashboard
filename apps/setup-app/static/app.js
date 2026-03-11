const panels = Array.from(document.querySelectorAll(".panel"));
const indicators = Array.from(document.querySelectorAll(".step"));
const prevBtn = document.getElementById("prev-step");
const nextBtn = document.getElementById("next-step");
const preflightBtn = document.getElementById("run-preflight");
const preflightResults = document.getElementById("preflight-results");
const readySummary = document.getElementById("ready-summary");
const welcomeSummary = document.getElementById("welcome-summary");
const settingsSummary = document.getElementById("settings-summary");
const installOutput = document.getElementById("install-output");
const form = document.getElementById("config-form");
const installBtn = document.getElementById("run-install");
const upgradeBtn = document.getElementById("run-upgrade");
const rollbackBtn = document.getElementById("run-rollback");
const healthBtn = document.getElementById("run-health");
const toggleAdvancedBtn = document.getElementById("toggle-advanced");
const mqttDetectBtn = document.getElementById("detect-mqtt-host");

let currentStep = 0;
let loadedConfig = null;
let preflightRequested = false;

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
  panels.forEach((panel, idx) => panel.classList.toggle("active", idx === currentStep));
  indicators.forEach((indicator, idx) => indicator.classList.toggle("active", idx === currentStep));
  prevBtn.disabled = currentStep === 0;
  nextBtn.textContent = currentStep === panels.length - 1 ? "Finish" : "Next";

  if (currentStep === 2 && !preflightRequested) {
    void runPreflight();
  }
};

const formPayload = (allowedFields = null) => {
  const data = new FormData(form);
  const payload = {};
  for (const [key, value] of data.entries()) {
    if (allowedFields && !allowedFields.has(key)) continue;
    if (value === "") continue;
    if (["core_port", "mqtt_port", "redis_port", "backup_retention_days"].includes(key)) {
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
  loadedConfig = data;
  populateForm(data);
  renderConfigSummaries(data);
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
  loadedConfig = data;
  populateForm(data);
  renderConfigSummaries(data);
  return data;
};

const summaryCard = (title, value, note = "") => `
  <div class="summary-card">
    <span class="summary-label">${title}</span>
    <strong>${value || "Not set"}</strong>
    ${note ? `<span class="summary-note">${note}</span>` : ""}
  </div>
`;

const renderConfigSummaries = (config) => {
  welcomeSummary.innerHTML = [
    summaryCard(
      "Installer bundle",
      config.bundle_path || "Auto-detecting from the installer app",
      "If this field is filled in, Setup Center is ready to use the embedded controller package.",
    ),
    summaryCard(
      "Controller address",
      config.mqtt_host || "127.0.0.1",
      "Nodes on your network will use this address to reach the controller.",
    ),
    summaryCard(
      "App + services",
      config.install_root,
      "Infrastructure Dashboard binaries and managed services live here.",
    ),
    summaryCard(
      "Data + backups",
      config.data_root,
      `Backups save to ${config.backup_root || "the default backup folder"}.`,
    ),
  ].join("");

  settingsSummary.innerHTML = [
    summaryCard("App files", config.install_root),
    summaryCard("Data storage", config.data_root),
    summaryCard("Backups", config.backup_root),
    summaryCard("Controller URL", `http://127.0.0.1:${config.core_port || 8000}/`),
  ].join("");
};

const prettyCheckName = (id) =>
  id
    .replace(/-/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase());

const renderChecks = (checks = []) => {
  if (!checks.length) {
    preflightResults.innerHTML = "<p class=\"muted\">No readiness data returned.</p>";
    return;
  }
  preflightResults.innerHTML = checks
    .map(
      (check) => `
        <div class="check">
          <div>
            <strong>${prettyCheckName(check.id)}</strong>
            <span>${check.message}</span>
          </div>
          <span class="badge ${check.status}">${check.status === "error" ? "fix" : check.status}</span>
        </div>
      `,
    )
    .join("");
};

const renderReadySummary = (summary) => {
  if (!summary) {
    readySummary.innerHTML = "";
    return;
  }
  readySummary.innerHTML = [
    summaryCard(
      "Ready checks",
      String(summary.ready ?? 0),
      "Checks that passed with no action needed.",
    ),
    summaryCard(
      "Warnings",
      String(summary.warnings ?? 0),
      "Warnings do not block install, but they are worth reviewing.",
    ),
    summaryCard(
      "Needs attention",
      String(summary.blocked ?? 0),
      summary.blocked > 0
        ? "Fix these items before running the installer."
        : "No blocking issues were found.",
    ),
    summaryCard(
      "Install status",
      summary.install_ready ? "Ready" : "Action needed",
      "Detailed logs are written automatically for support.",
    ),
  ].join("");
};

const blockingInstallDetail = (checks = []) => {
  const blocked = checks.filter((check) => check.status === "error");
  if (!blocked.length) {
    return "Setup Center moved you back to the readiness step so you can review the blocked items.";
  }
  return blocked
    .map((check) =>
      check.id === "bundle-path"
        ? "The installer bundle is missing or stale. Re-open the Infrastructure Dashboard Installer app so it can refresh the embedded controller package."
        : check.message,
    )
    .join(" ");
};

const renderInstallState = ({
  title,
  tone = "ok",
  message,
  detail = "",
  actionLabel = "",
  actionHref = "",
  actions = [],
}) => {
  const actionMarkup = [
    actionLabel && actionHref
      ? `<a class="btn primary" href="${actionHref}" target="_blank" rel="noreferrer">${actionLabel}</a>`
      : "",
    ...actions.map(
      (action) => `
        <button
          type="button"
          class="btn ${action.tone || "ghost"}"
          data-install-action="${action.key}"
        >
          ${action.label}
        </button>
      `,
    ),
  ]
    .filter(Boolean)
    .join("");

  installOutput.innerHTML = `
    <div class="status-card ${tone}">
      <div class="status-copy">
        <span class="summary-label">${title}</span>
        <strong>${message}</strong>
        ${detail ? `<span class="summary-note">${detail}</span>` : ""}
      </div>
      ${actionMarkup ? `<div class="actions">${actionMarkup}</div>` : ""}
    </div>
  `;
};

const renderFailureRecovery = ({ title, message, detail }) => {
  renderInstallState({
    title,
    tone: "error",
    message,
    detail: `${detail} Choose whether to keep the current install for troubleshooting or remove it before retrying.`,
    actions: [
      { key: "keep-failed-install", label: "Keep current install", tone: "ghost" },
      { key: "remove-failed-install", label: "Remove failed install", tone: "primary" },
    ],
  });
};

const apiJson = async (url, options = {}) => {
  const response = await fetch(url, options);
  const data = await response.json();
  if (!response.ok) {
    throw new Error(data?.error || "Request failed");
  }
  return data;
};

const runPreflight = async () => {
  preflightRequested = true;
  preflightResults.innerHTML = "<p class=\"muted\">Checking this Mac…</p>";
  try {
    await saveConfig();
    const data = await apiJson("/api/preflight");
    renderReadySummary(data.summary);
    renderChecks(data.checks || []);
    return data;
  } catch (err) {
    renderInstallState({
      title: "Readiness check",
      tone: "error",
      message: "Setup Center could not finish the readiness check.",
      detail: "Detailed diagnostics were written to the local setup activity log.",
    });
    preflightResults.innerHTML = `<p class="muted">${err?.message || err}</p>`;
    return null;
  }
};

const ensureReadyForInstall = async () => {
  const data = await runPreflight();
  if (!data?.summary?.install_ready) {
    showStep(2);
    renderInstallState({
      title: "Install blocked",
      tone: "warn",
      message: "Fix the readiness items before continuing.",
      detail: blockingInstallDetail(data?.checks || []),
    });
    return false;
  }
  return true;
};

const waitForHandoff = async () => {
  const deadline = Date.now() + 20000;
  let sawDown = false;
  while (Date.now() < deadline) {
    try {
      const res = await fetch("/healthz", { cache: "no-store" });
      if (res.ok && sawDown) {
        return true;
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
  return false;
};

const renderHealthReport = (report, dashboardUrl = "") => {
  const entries = [
    { key: "core_api", label: "Controller API" },
    { key: "dashboard", label: "Dashboard UI" },
    { key: "mqtt", label: "MQTT broker" },
    { key: "database", label: "Database" },
    { key: "redis", label: "Redis" },
    { key: "qdrant", label: "Qdrant" },
  ];
  const checksHtml = entries
    .map(({ key, label }) => {
      const entry = report?.[key] || { status: "unknown", message: "No data returned" };
      const tone = entry.status === "ok" ? "ok" : entry.status === "error" ? "error" : "warn";
      return `
        <div class="check">
          <div>
            <strong>${label}</strong>
            <span>${entry.message}</span>
          </div>
          <span class="badge ${tone}">${entry.status}</span>
        </div>
      `;
    })
    .join("");
  const openButton =
    dashboardUrl && report?.dashboard?.status === "ok"
      ? `<a class="btn primary" href="${dashboardUrl}" target="_blank" rel="noreferrer">Open Dashboard</a>`
      : "";

  installOutput.innerHTML = `
    <div class="status-card ok">
      <div class="status-copy">
        <span class="summary-label">Service status</span>
        <strong>Infrastructure Dashboard service check complete.</strong>
        <span class="summary-note">Detailed logs remain available in the local setup activity log.</span>
      </div>
      ${openButton}
    </div>
    <div class="checklist">${checksHtml}</div>
  `;
};

const runHealthCheck = async (dashboardUrl = "") => {
  renderInstallState({
    title: "Service check",
    tone: "ok",
    message: "Verifying Infrastructure Dashboard services…",
  });
  try {
    const data = await apiJson("/api/health-report");
    if (!data.ok) {
      const failures = [
        data?.report?.core_api,
        data?.report?.dashboard,
        data?.report?.database,
        data?.report?.mqtt,
        data?.report?.redis,
        data?.report?.qdrant,
      ]
        .filter((entry) => entry?.status === "error")
        .map((entry) => entry.message)
        .join(" ");
      renderFailureRecovery({
        title: "Service check",
        message: "Service verification failed.",
        detail: failures || "Detailed diagnostics were saved automatically.",
      });
      return;
    }
    renderHealthReport(data.report || {}, dashboardUrl);
  } catch (err) {
    renderFailureRecovery({
      title: "Service check",
      message: "Service verification failed.",
      detail: err?.message || String(err),
    });
  }
};

const removeFailedInstall = async () => {
  renderInstallState({
    title: "Cleanup",
    tone: "warn",
    message: "Removing the failed install…",
    detail: "Stopping services and deleting the current install roots.",
  });
  try {
    const data = await apiJson("/api/remove-failed-install", { method: "POST" });
    if (!data.ok) {
      renderFailureRecovery({
        title: "Cleanup",
        message: "Cleanup failed.",
        detail: "The current install could not be removed automatically. Review the local setup activity log.",
      });
      return;
    }
    showStep(2);
    await runPreflight();
    renderInstallState({
      title: "Cleanup",
      tone: "ok",
      message: data.message || "Removed the failed install.",
      detail: "Readiness has been refreshed. Re-run install after the blocked items are clear.",
    });
  } catch (err) {
    renderFailureRecovery({
      title: "Cleanup",
      message: "Cleanup failed.",
      detail: err?.message || String(err),
    });
  }
};

const runInstallerAction = async (endpoint, title) => {
  renderInstallState({
    title,
    tone: "ok",
    message: "Saving settings and preparing the install…",
  });
  try {
    await saveConfig();
    if (!(await ensureReadyForInstall())) {
      return;
    }
    showStep(3);
    renderInstallState({
      title,
      tone: "ok",
      message: `${title} is running…`,
      detail: "Setup Center is applying the bundle and starting managed services.",
    });

    const data = await apiJson(endpoint, { method: "POST" });
    if (!data.ok) {
      renderFailureRecovery({
        title,
        message: data.message || `${title} failed.`,
        detail: "Detailed diagnostics were saved automatically.",
      });
      return;
    }
    renderInstallState({
      title,
      tone: "ok",
      message: data.message || `${title} complete.`,
      detail: "Detailed command output was saved automatically for support.",
      actionLabel: "Open Dashboard",
      actionHref: data.dashboard_url || "",
    });

    if (data.handoff) {
      renderInstallState({
        title,
        tone: "ok",
        message: "Infrastructure Dashboard is switching from Setup Center to managed services…",
        detail: "Waiting for the local service handoff to finish.",
      });
      await waitForHandoff();
      await runHealthCheck(data.dashboard_url || "");
      return;
    }

    if (data.ok) {
      await runHealthCheck(data.dashboard_url || "");
    }
  } catch (err) {
    renderInstallState({
      title,
      tone: "error",
      message: `${title} failed.`,
      detail: err?.message || String(err),
    });
  }
};

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

prevBtn.addEventListener("click", () => showStep(currentStep - 1));
nextBtn.addEventListener("click", () => showStep(currentStep + 1));

preflightBtn.addEventListener("click", () => {
  void runPreflight();
});

installBtn.addEventListener("click", () => {
  void runInstallerAction("/api/install", "Install");
});

upgradeBtn.addEventListener("click", () => {
  void runInstallerAction("/api/upgrade", "Upgrade");
});

rollbackBtn.addEventListener("click", () => {
  void runInstallerAction("/api/rollback", "Rollback");
});

healthBtn.addEventListener("click", () => {
  void runHealthCheck(loadedConfig ? `http://127.0.0.1:${loadedConfig.core_port || 8000}/` : "");
});

installOutput.addEventListener("click", (event) => {
  const action = event.target.closest("[data-install-action]")?.dataset.installAction;
  if (!action) return;
  if (action === "keep-failed-install") {
    renderInstallState({
      title: "Install paused",
      tone: "warn",
      message: "The current install files were left in place.",
      detail: "Review the local setup activity log, or remove the failed install before retrying.",
      actions: [{ key: "remove-failed-install", label: "Remove failed install", tone: "primary" }],
    });
    return;
  }
  if (action === "remove-failed-install") {
    void removeFailedInstall();
  }
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
    mqttDetectBtn.textContent = "Detecting…";
    try {
      const data = await apiJson("/api/local-ip");
      const recommended = data?.recommended;
      if (typeof recommended === "string" && recommended.trim()) {
        field.value = recommended.trim();
        await saveConfig();
      }
    } finally {
      mqttDetectBtn.disabled = false;
      mqttDetectBtn.textContent = originalText;
    }
  });
}

loadConfig()
  .catch((err) => {
    renderInstallState({
      title: "Setup Center",
      tone: "error",
      message: "Unable to load installer settings.",
      detail: err?.message || String(err),
    });
  });

showStep(0);
