"use client";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import NodeButton from "@/features/nodes/components/NodeButton";

import { useSetupDaemonConfig } from "../hooks/useSetupDaemonConfig";
import type { Message } from "../types";

export default function ControllerConfigurationSection({
  onMessage,
}: {
  onMessage: (message: Message) => void;
}) {
  const model = useSetupDaemonConfig(onMessage);

  return (
    <CollapsibleCard
      title="Controller configuration"
      description="Configure ports, MQTT reachability, backups, and the controller bundle used by install/upgrade actions."
      defaultOpen={false}
      bodyClassName="space-y-4"
      actions={
        <div className="flex flex-wrap gap-2">
          <NodeButton size="xs" onClick={model.loadConfig} disabled={model.busy === "loading"}>
            {model.busy === "loading" ? "Refreshing..." : "Refresh"}
          </NodeButton>
          <NodeButton
            size="xs"
            variant="primary"
            onClick={model.saveConfig}
            disabled={model.busy != null || !model.config}
          >
            {model.busy === "saving" ? "Saving..." : "Save"}
          </NodeButton>
        </div>
      }
    >
      {model.error && <p className="mt-3 text-sm text-rose-600">{model.error}</p>}

      <div className="mt-4 grid gap-4 lg:grid-cols-3">
        <div className="space-y-4 lg:col-span-2">
          <div className="grid gap-4 md:grid-cols-2">
            <Card className="rounded-lg gap-0 bg-card-inset p-4">
              <p className="text-sm font-semibold text-card-foreground">
                Network + ports
              </p>
 <p className="text-xs text-muted-foreground">
                Remote nodes must reach the controller&apos;s MQTT broker; localhost will not work.
              </p>
              <div className="mt-3 grid gap-3">
                <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Core API port
                  </span>
                  <Input
                    type="number"
                    min={1}
                    max={65535}
                    value={model.draft.core_port}
                    onChange={(event) => model.updateDraft({ core_port: event.target.value })}
                  />
                </label>

                <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    MQTT host (LAN-reachable)
                  </span>
                  <Input
                    type="text"
                    placeholder="192.168.1.50"
                    value={model.draft.mqtt_host}
                    onChange={(event) => model.updateDraft({ mqtt_host: event.target.value })}
                  />
                </label>

                {(() => {
                  const host = model.draft.mqtt_host.trim().toLowerCase();
                  const loopback = host === "" || host === "127.0.0.1" || host === "localhost";
                  if (!loopback) return null;
                  return (
 <p className="text-xs text-amber-700">
                      Warning: localhost MQTT only works on the controller. Remote nodes need the
                      controller&apos;s LAN IP.
                    </p>
                  );
                })()}

                <div className="flex flex-wrap items-center gap-2">
                  <NodeButton
                    size="xs"
                    onClick={model.useRecommendedMqttHost}
                    disabled={model.localIpBusy}
                  >
                    Use this Mac&apos;s IP
                  </NodeButton>
                  {model.localIp?.recommended && (
 <span className="text-xs text-muted-foreground">
                      Recommended: {model.localIp.recommended}
                    </span>
                  )}
                </div>

                {model.localIp?.candidates?.length ? (
 <p className="text-xs text-muted-foreground">
                    Candidates: {model.localIp.candidates.join(", ")}
                  </p>
                ) : null}

                {model.localIpError && (
                  <p className="text-xs text-rose-600">{model.localIpError}</p>
                )}

                <div className="grid gap-3 md:grid-cols-2">
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      MQTT port
                    </span>
                    <Input
                      type="number"
                      min={1}
                      max={65535}
                      value={model.draft.mqtt_port}
                      onChange={(event) => model.updateDraft({ mqtt_port: event.target.value })}
                    />
                  </label>
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Redis port
                    </span>
                    <Input
                      type="number"
                      min={1}
                      max={65535}
                      value={model.draft.redis_port}
                      onChange={(event) => model.updateDraft({ redis_port: event.target.value })}
                    />
                  </label>
                </div>
              </div>
            </Card>

            <Card className="rounded-lg gap-0 bg-card-inset p-4">
              <p className="text-sm font-semibold text-card-foreground">Backups</p>
 <p className="text-xs text-muted-foreground">
                Controller stores daily backups of node configs under this path.
              </p>
              <div className="mt-3 grid gap-3">
                <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Backup root path
                  </span>
                  <Input
                    type="text"
                    value={model.draft.backup_root}
                    onChange={(event) => model.updateDraft({ backup_root: event.target.value })}
                  />
                </label>
                <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Retention (days)
                  </span>
                  <Input
                    type="number"
                    min={1}
                    value={model.draft.backup_retention_days}
                    onChange={(event) =>
                      model.updateDraft({ backup_retention_days: event.target.value })
                    }
                  />
                </label>
              </div>
            </Card>

            <Card className="rounded-lg gap-0 bg-card-inset p-4 md:col-span-2">
              <p className="text-sm font-semibold text-card-foreground">
                Controller bundle DMG
              </p>
 <p className="text-xs text-muted-foreground">
                Used by Install/Upgrade/Rollback. Must be a local path (Public installer embeds
                bundles; no remote downloads).
              </p>
              <div className="mt-3 grid gap-3">
                <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Bundle path (DMG)
                  </span>
                  <Input
                    type="text"
                    placeholder="/Users/Shared/FarmDashboardController-0.1.9.xx.dmg"
                    value={model.draft.bundle_path}
                    onChange={(event) => model.updateDraft({ bundle_path: event.target.value })}
                  />
                </label>
                {model.config?.bundle_path && (
 <p className="text-xs text-muted-foreground">
                    Current bundle: {model.config.bundle_path}
                  </p>
                )}
              </div>
            </Card>
          </div>

          <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
 <label className="flex items-center gap-2 text-sm text-foreground">
              <input
                type="checkbox"
                checked={model.advanced}
                onChange={(event) => model.setAdvanced(event.target.checked)}
              />
              Advanced settings
            </label>
            {model.config && (
 <p className="text-xs text-muted-foreground">
                Profile: <span className="font-semibold">{model.config.profile}</span>
              </p>
            )}
          </div>

          {model.advanced && (
            <div className="grid gap-4 md:grid-cols-2">
              <Card className="rounded-lg gap-0 bg-card-inset p-4">
                <p className="text-sm font-semibold text-card-foreground">Paths</p>
                <div className="mt-3 grid gap-3">
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Install root
                    </span>
                    <Input
                      type="text"
                      value={model.draft.install_root}
                      onChange={(event) => model.updateDraft({ install_root: event.target.value })}
                    />
                  </label>
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Data root
                    </span>
                    <Input
                      type="text"
                      value={model.draft.data_root}
                      onChange={(event) => model.updateDraft({ data_root: event.target.value })}
                    />
                  </label>
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Logs root
                    </span>
                    <Input
                      type="text"
                      value={model.draft.logs_root}
                      onChange={(event) => model.updateDraft({ logs_root: event.target.value })}
                    />
                  </label>
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      farmctl path
                    </span>
                    <Input
                      type="text"
                      value={model.draft.farmctl_path}
                      onChange={(event) => model.updateDraft({ farmctl_path: event.target.value })}
                    />
                  </label>
                </div>
              </Card>

              <Card className="rounded-lg gap-0 bg-card-inset p-4">
                <p className="text-sm font-semibold text-card-foreground">Services</p>
                <div className="mt-3 grid gap-3">
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Core server binary
                    </span>
                    <Input
                      type="text"
                      value={model.draft.core_binary}
                      onChange={(event) => model.updateDraft({ core_binary: event.target.value })}
                    />
                  </label>
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Telemetry sidecar binary
                    </span>
                    <Input
                      type="text"
                      value={model.draft.sidecar_binary}
                      onChange={(event) => model.updateDraft({ sidecar_binary: event.target.value })}
                    />
                  </label>
                  <div className="grid gap-3 md:grid-cols-2">
                    <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Service user
                      </span>
                      <Input
                        type="text"
                        value={model.draft.service_user}
                        onChange={(event) => model.updateDraft({ service_user: event.target.value })}
                      />
                    </label>
                    <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Service group
                      </span>
                      <Input
                        type="text"
                        value={model.draft.service_group}
                        onChange={(event) =>
                          model.updateDraft({ service_group: event.target.value })
                        }
                      />
                    </label>
                  </div>
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Launchd label prefix
                    </span>
                    <Input
                      type="text"
                      value={model.draft.launchd_label_prefix}
                      onChange={(event) =>
                        model.updateDraft({ launchd_label_prefix: event.target.value })
                      }
                    />
                  </label>
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Setup daemon port
                    </span>
                    <Input
                      type="number"
                      min={1}
                      max={65535}
                      value={model.draft.setup_port}
                      onChange={(event) => model.updateDraft({ setup_port: event.target.value })}
                    />
                  </label>
                  <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Database URL (sensitive)
                    </span>
                    <Input
                      type="text"
                      value={model.draft.database_url}
                      onChange={(event) => model.updateDraft({ database_url: event.target.value })}
                    />
                  </label>
                </div>
              </Card>

              <Card className="rounded-lg gap-0 bg-card-inset p-4">
                <p className="text-sm font-semibold text-card-foreground">
                  Core runtime
                </p>
 <p className="mt-1 text-xs text-muted-foreground">
                  Optional MQTT auth and background polling settings. Changes apply after running{" "}
                  <strong>Upgrade</strong>.
                </p>

                <div className="mt-4 space-y-4">
                  <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      MQTT authentication (optional)
                    </p>
                    <div className="mt-2 grid gap-3">
                      <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          MQTT username
                        </span>
                        <Input
                          type="text"
                          placeholder="Leave blank for no auth"
                          value={model.draft.mqtt_username}
                          onChange={(event) => model.updateDraft({ mqtt_username: event.target.value })}
                        />
                      </label>
                      <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          MQTT password (sensitive)
                        </span>
                        <Input
                          type="password"
                          placeholder="Leave blank to keep existing"
                          value={model.draft.mqtt_password}
                          onChange={(event) => {
                            model.setMqttPasswordClear(false);
                            model.updateDraft({ mqtt_password: event.target.value });
                          }}
                        />
                      </label>
                      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
 <div className="text-xs text-muted-foreground">
                          {model.config?.mqtt_password_configured
                            ? "A saved MQTT password is configured."
                            : "No saved MQTT password."}
                        </div>
                        <NodeButton
                          size="xs"
                          onClick={() => {
                            model.setMqttPasswordClear(true);
                            model.updateDraft({ mqtt_password: "" });
                          }}
                          disabled={
                            !model.config?.mqtt_password_configured &&
                            !model.draft.mqtt_password.trim().length
                          }
                        >
                          Clear saved password
                        </NodeButton>
                      </div>
                    </div>
                  </div>

                  <div className="border-t border-border pt-4">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Background services
                    </p>
                    <div className="mt-3 space-y-3">
 <label className="flex items-center gap-2 text-sm text-foreground">
                        <input
                          type="checkbox"
                          checked={model.draft.enable_analytics_feeds}
                          onChange={(event) =>
                            model.updateDraft({ enable_analytics_feeds: event.target.checked })
                          }
                        />
                        Enable analytics feeds (power/water integrations)
                      </label>
                      <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          Analytics poll interval (seconds)
                        </span>
                        <Input
                          type="number"
                          min={60}
                          value={model.draft.analytics_feed_poll_interval_seconds}
                          onChange={(event) =>
                            model.updateDraft({
                              analytics_feed_poll_interval_seconds: event.target.value,
                            })
                          }
                        />
                      </label>

 <label className="flex items-center gap-2 text-sm text-foreground">
                        <input
                          type="checkbox"
                          checked={model.draft.enable_forecast_ingestion}
                          onChange={(event) =>
                            model.updateDraft({ enable_forecast_ingestion: event.target.checked })
                          }
                        />
                        Enable forecast ingestion (Open-Meteo + Forecast.Solar)
                      </label>
                      <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          Forecast poll interval (seconds)
                        </span>
                        <Input
                          type="number"
                          min={300}
                          value={model.draft.forecast_poll_interval_seconds}
                          onChange={(event) =>
                            model.updateDraft({ forecast_poll_interval_seconds: event.target.value })
                          }
                        />
 <p className="mt-1 text-xs text-muted-foreground">
                          Forecast.Solar public <code className="px-1">/estimate</code> is rate
                          limited; keep this conservative if you have multiple PV nodes.
                        </p>
                      </label>

                      <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          Schedule poll interval (seconds)
                        </span>
                        <Input
                          type="number"
                          min={5}
                          value={model.draft.schedule_poll_interval_seconds}
                          onChange={(event) =>
                            model.updateDraft({ schedule_poll_interval_seconds: event.target.value })
                          }
                        />
                      </label>

                      <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          Offline threshold (seconds)
                        </span>
                        <Input
                          type="number"
                          min={1}
                          value={model.draft.offline_threshold_seconds}
                          onChange={(event) =>
                            model.updateDraft({ offline_threshold_seconds: event.target.value })
                          }
                        />
 <p className="mt-1 text-xs text-muted-foreground">
                          Used by <strong>telemetry-sidecar</strong> to mark nodes/sensors offline
                          (default: 5s).
                        </p>
                      </label>

                      <div className="border-t border-border pt-4">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          Telemetry-sidecar tuning (advanced)
                        </p>
 <p className="mt-1 text-xs text-muted-foreground">
                          Controls ingestion throughput and MQTT behavior. Defaults are fine for
                          most installs.
                        </p>
                        <div className="mt-3 grid gap-3 md:grid-cols-2">
                          <label className="grid gap-1 md:col-span-2">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              MQTT topic prefix
                            </span>
                            <Input
                              type="text"
                              placeholder="iot"
                              value={model.draft.sidecar_mqtt_topic_prefix}
                              onChange={(event) =>
                                model.updateDraft({ sidecar_mqtt_topic_prefix: event.target.value })
                              }
                            />
 <p className="mt-1 text-xs text-muted-foreground">
                              Must match node publishing topics. Leave blank to reset to default (
                              <code className="px-1">iot</code>).
                            </p>
                          </label>

                          <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              MQTT keepalive (seconds)
                            </span>
                            <Input
                              type="number"
                              min={5}
                              value={model.draft.sidecar_mqtt_keepalive_secs}
                              onChange={(event) =>
                                model.updateDraft({
                                  sidecar_mqtt_keepalive_secs: event.target.value,
                                })
                              }
                            />
                          </label>

 <label className="flex items-center gap-2 text-sm text-foreground md:col-span-2">
                            <input
                              type="checkbox"
                              checked={model.draft.sidecar_enable_mqtt_listener}
                              onChange={(event) =>
                                model.updateDraft({
                                  sidecar_enable_mqtt_listener: event.target.checked,
                                })
                              }
                            />
                            Enable MQTT listener in telemetry-sidecar
                          </label>

                          <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              DB batch size
                            </span>
                            <Input
                              type="number"
                              min={10}
                              value={model.draft.sidecar_batch_size}
                              onChange={(event) =>
                                model.updateDraft({ sidecar_batch_size: event.target.value })
                              }
                            />
                          </label>

                          <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              Flush interval (ms)
                            </span>
                            <Input
                              type="number"
                              min={50}
                              value={model.draft.sidecar_flush_interval_ms}
                              onChange={(event) =>
                                model.updateDraft({ sidecar_flush_interval_ms: event.target.value })
                              }
                            />
                          </label>

                          <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              Max queue
                            </span>
                            <Input
                              type="number"
                              min={10}
                              value={model.draft.sidecar_max_queue}
                              onChange={(event) =>
                                model.updateDraft({ sidecar_max_queue: event.target.value })
                              }
                            />
 <p className="mt-1 text-xs text-muted-foreground">
                              Queue length limit before dropping/pressure. Default is ~10× batch
                              size.
                            </p>
                          </label>

                          <label className="grid gap-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              Status poll interval (ms)
                            </span>
                            <Input
                              type="number"
                              min={100}
                              value={model.draft.sidecar_status_poll_interval_ms}
                              onChange={(event) =>
                                model.updateDraft({
                                  sidecar_status_poll_interval_ms: event.target.value,
                                })
                              }
                            />
                          </label>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </Card>
            </div>
          )}

 <p className="text-xs text-muted-foreground">
            Changing ports/paths writes the setup-daemon config. Run <strong>Upgrade</strong> to
            restart services and apply updated environment settings.
          </p>
        </div>

        <Card className="rounded-lg gap-0 bg-card-inset p-4">
          <div className="flex items-start justify-between gap-3">
            <div>
              <p className="text-sm font-semibold text-card-foreground">
                Preflight checks
              </p>
 <p className="text-xs text-muted-foreground">
                Sanity checks for ports, paths, and bundle availability.
              </p>
            </div>
            <NodeButton size="xs" onClick={model.loadPreflight} disabled={model.preflightBusy}>
              {model.preflightBusy ? "Refreshing..." : "Refresh"}
            </NodeButton>
          </div>

          {model.preflightError && <p className="mt-2 text-xs text-rose-600">{model.preflightError}</p>}

          {!model.preflight && !model.preflightBusy && (
 <p className="mt-3 text-sm text-muted-foreground">No preflight results yet.</p>
          )}

          {model.preflight && (
            <div className="mt-3 space-y-2">
              {model.preflight.map((check) => {
                const tone =
                  check.status === "ok"
 ? "text-emerald-600"
                    : check.status === "error"
 ? "text-rose-600"
                      : check.status === "warn"
 ? "text-amber-600"
 : "text-muted-foreground";
                return (
                  <Card
                    key={check.id}
                    className="flex-row items-start justify-between gap-3 rounded-lg px-3 py-2"
                  >
                    <div className="min-w-0">
 <p className="truncate text-xs font-semibold uppercase tracking-wide text-foreground">
                        {check.id.replace(/-/g, " ")}
                      </p>
 <p className="text-xs text-muted-foreground">{check.message}</p>
                    </div>
                    <p className={`text-xs font-semibold uppercase tracking-wide ${tone}`}>
                      {check.status}
                    </p>
                  </Card>
                );
              })}
            </div>
          )}
        </Card>
      </div>
    </CollapsibleCard>
  );
}

