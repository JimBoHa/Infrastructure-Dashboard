"use client";

import { useMemo, useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Card } from "@/components/ui/card";
import InlineBanner from "@/components/InlineBanner";
import NodeButton from "@/features/nodes/components/NodeButton";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import type { MapLayer, MapLayerKind, MapLayerSourceType, MapLayerUpsertPayload } from "@/types/map";
import { kml as kmlToGeojson } from "@tmcw/togeojson";

export type LayerDraft = {
  name: string;
  kind: MapLayerKind;
  source_type: MapLayerSourceType;
  enabled: boolean;
  opacity: number;
  z_index: number;
  xyz: { url_template: string; attribution?: string; tile_size?: number; max_zoom?: number };
  wms: {
    base_url: string;
    layers: string;
    attribution?: string;
    styles?: string;
    format?: string;
    version?: string;
    transparent?: boolean;
  };
  arcgis: {
    base_url: string;
    attribution?: string;
    mode: "tile" | "export";
    format?: string;
    transparent?: boolean;
    max_zoom?: number;
  };
  geojson: {
    file_name: string;
    data: Record<string, unknown> | null;
  };
};

const ensureString = (value: unknown): string =>
  typeof value === "string" ? value : value == null ? "" : String(value);

const ensureNumber = (value: unknown, fallback: number): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;

const ensureOptionalInt = (value: unknown): number | undefined => {
  if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
  return Math.floor(value);
};

export function layerDraftFromLayer(layer: MapLayer): LayerDraft {
  const config = (layer.config ?? {}) as Record<string, unknown>;
  return {
    name: layer.name ?? "",
    kind: layer.kind,
    source_type: layer.source_type,
    enabled: layer.enabled,
    opacity: layer.opacity,
    z_index: layer.z_index,
    xyz: {
      url_template: ensureString(config.url_template),
      attribution: ensureString(config.attribution) || undefined,
      tile_size: config.tile_size != null ? ensureNumber(config.tile_size, 256) : undefined,
      max_zoom: ensureOptionalInt(config.max_zoom),
    },
    wms: {
      base_url: ensureString(config.base_url),
      layers: ensureString(config.layers),
      attribution: ensureString(config.attribution) || undefined,
      styles: ensureString(config.styles) || undefined,
      format: ensureString(config.format) || undefined,
      version: ensureString(config.version) || undefined,
      transparent: typeof config.transparent === "boolean" ? config.transparent : undefined,
    },
    arcgis: {
      base_url: ensureString(config.base_url),
      attribution: ensureString(config.attribution) || undefined,
      mode: (ensureString(config.mode) === "export" ? "export" : "tile") as "tile" | "export",
      format: ensureString(config.format) || undefined,
      transparent: typeof config.transparent === "boolean" ? config.transparent : undefined,
      max_zoom: ensureOptionalInt(config.max_zoom),
    },
    geojson: {
      file_name: ensureString(config.file_name),
      data: (config.data as Record<string, unknown> | null) ?? null,
    },
  };
}

export function layerDraftToUpsertPayload(draft: LayerDraft): MapLayerUpsertPayload | null {
  const name = draft.name.trim();
  if (!name) return null;

  const base: MapLayerUpsertPayload = {
    name,
    kind: draft.kind,
    source_type: draft.source_type,
    config: {},
    enabled: draft.enabled,
    opacity: draft.opacity,
    z_index: draft.z_index,
  };

  if (draft.source_type === "xyz") {
    const urlTemplate = draft.xyz.url_template.trim();
    if (!urlTemplate) return null;
    const maxZoom = ensureOptionalInt(draft.xyz.max_zoom);
    base.config = {
      url_template: urlTemplate,
      tile_size: draft.xyz.tile_size ?? 256,
      attribution: draft.xyz.attribution?.trim() || undefined,
      max_zoom: maxZoom,
    };
  } else if (draft.source_type === "wms") {
    const baseUrl = draft.wms.base_url.trim();
    const layers = draft.wms.layers.trim();
    if (!baseUrl || !layers) return null;
    base.config = {
      base_url: baseUrl,
      layers,
      styles: draft.wms.styles?.trim() || "",
      format: draft.wms.format?.trim() || "image/png",
      version: draft.wms.version?.trim() || "1.3.0",
      transparent: draft.wms.transparent ?? true,
      attribution: draft.wms.attribution?.trim() || undefined,
    };
  } else if (draft.source_type === "arcgis") {
    const baseUrl = draft.arcgis.base_url.trim();
    if (!baseUrl) return null;
    const maxZoom = ensureOptionalInt(draft.arcgis.max_zoom);
    base.config = {
      base_url: baseUrl,
      mode: draft.arcgis.mode,
      format: draft.arcgis.format?.trim() || "png32",
      transparent: draft.arcgis.transparent ?? true,
      attribution: draft.arcgis.attribution?.trim() || undefined,
      max_zoom: maxZoom,
    };
  } else if (draft.source_type === "geojson") {
    if (!draft.geojson.data) return null;
    base.config = {
      file_name: draft.geojson.file_name || name,
      data: draft.geojson.data,
    };
  }

  return base;
}

type LayerPreset = {
  id: string;
  label: string;
  hint: string;
  draft: Partial<LayerDraft>;
};

const PRESETS: LayerPreset[] = [
  {
    id: "sccgis-usgs-quads",
    label: "Santa Cruz County GIS — USGS Quads (topo)",
    hint: "ArcGIS cached tiles from https://sccgis.santacruzcountyca.gov (example public topo source).",
    draft: {
      name: "Topo (SCCGIS USGS Quads)",
      source_type: "arcgis",
      arcgis: {
        base_url: "https://sccgis.santacruzcountyca.gov/server/rest/services/Cache/USGS_Quads/MapServer",
        mode: "tile",
      },
      opacity: 0.7,
    },
  },
  {
    id: "sccgis-hillshade",
    label: "Santa Cruz County GIS — Hillshade (overlay)",
    hint: "Hillshade overlay (improves terrain readability over satellite).",
    draft: {
      name: "Hillshade (SCCGIS 2020)",
      source_type: "arcgis",
      arcgis: {
        base_url:
          "https://sccgis.santacruzcountyca.gov/server/rest/services/Cache/Hillshade_2020/MapServer",
        mode: "tile",
      },
      opacity: 0.45,
    },
  },
  {
    id: "sccgis-imagery-2020",
    label: "Santa Cruz County GIS — Imagery 2020 (satellite alternative)",
    hint: "Local imagery tiles (as an alternative to global imagery providers).",
    draft: {
      name: "Satellite (SCCGIS Imagery 2020)",
      kind: "base",
      source_type: "arcgis",
      arcgis: {
        base_url:
          "https://sccgis.santacruzcountyca.gov/server/rest/services/Cache/Imagery_2020/MapServer",
        mode: "tile",
      },
      opacity: 1,
    },
  },
];

async function readTextFile(file: File): Promise<string> {
  return await file.text();
}

async function parseGeoJsonOrKml(file: File): Promise<Record<string, unknown>> {
  const name = file.name.toLowerCase();
  const text = await readTextFile(file);

  if (name.endsWith(".kml")) {
    const doc = new DOMParser().parseFromString(text, "text/xml");
    const fc = kmlToGeojson(doc) as unknown as Record<string, unknown>;
    return fc;
  }

  const parsed = JSON.parse(text);
  if (!parsed || typeof parsed !== "object") {
    throw new Error("Invalid GeoJSON payload.");
  }
  return parsed as Record<string, unknown>;
}

export function LayerEditorModal({
  mode,
  draft,
  existing,
  onClose,
  onCreate,
  onUpdate,
}: {
  mode: "create" | "edit";
  draft: LayerDraft;
  existing: MapLayer | null;
  onClose: () => void;
  onCreate: (draft: LayerDraft) => Promise<MapLayer | void>;
  onUpdate: (draft: LayerDraft) => Promise<void>;
}) {
  const [local, setLocal] = useState<LayerDraft>(draft);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [presetId, setPresetId] = useState<string>("");

  const title = mode === "create" ? "Add map layer" : "Edit map layer";

  const showGeoUpload = local.source_type === "geojson";
  const canSubmit = Boolean(layerDraftToUpsertPayload(local));

  const presetsForKind = useMemo(() => {
    if (local.kind === "base") return PRESETS;
    return PRESETS.filter((p) => p.draft.kind !== "base");
  }, [local.kind]);

  const applyPreset = (id: string) => {
    const preset = PRESETS.find((p) => p.id === id);
    if (!preset) return;
    setPresetId(id);
    setLocal((prev) => ({
      ...prev,
      ...preset.draft,
      xyz: { ...prev.xyz, ...(preset.draft.xyz ?? {}) },
      wms: { ...prev.wms, ...(preset.draft.wms ?? {}) },
      arcgis: { ...prev.arcgis, ...(preset.draft.arcgis ?? {}) },
      geojson: { ...prev.geojson, ...(preset.draft.geojson ?? {}) },
    }));
  };

  const handleUpload = async (file: File | null) => {
    if (!file) return;
    setError(null);
    try {
      const data = await parseGeoJsonOrKml(file);
      setLocal((prev) => ({
        ...prev,
        source_type: "geojson",
        geojson: {
          file_name: file.name,
          data,
        },
      }));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to parse file.");
    }
  };

  const subtitle = existing?.system_key
    ? `Default layer: ${existing.system_key} (editable, not deletable)`
    : existing?.id != null
      ? `Layer #${existing.id}`
      : "Create a new map layer";

  return (
    <Dialog open onOpenChange={(v) => { if (!v && !busy) onClose(); }}>
      <DialogContent className="max-w-2xl gap-0">
        <div className="flex items-start justify-between gap-3">
          <div>
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription className="mt-1">{subtitle}</DialogDescription>
          </div>
          <NodeButton size="sm" onClick={onClose}>
            Close
          </NodeButton>
        </div>

        <div className="mt-5 space-y-4">
          {mode === "create" ? (
            <Card className="rounded-lg gap-0 bg-card-inset p-4 text-sm">
              <div className="font-semibold">Presets (optional)</div>
              <div className="mt-2 flex flex-col gap-2 sm:flex-row sm:items-center">
                <Select
                  value={presetId}
                  onChange={(e) => applyPreset(e.target.value)}
                >
                  <option value="">Choose a preset…</option>
                  {presetsForKind.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.label}
                    </option>
                  ))}
                </Select>
                {presetId ? (
 <div className="text-xs text-muted-foreground">
                    {PRESETS.find((p) => p.id === presetId)?.hint}
                  </div>
                ) : null}
              </div>
            </Card>
          ) : null}

          <div className="grid gap-4 sm:grid-cols-2">
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Layer name
              </label>
              <Input
                value={local.name}
                onChange={(e) => setLocal((prev) => ({ ...prev, name: e.target.value }))}
                placeholder="e.g., Topo overlay, Field boundaries, Utilities"
                className="mt-1"
              />
 <p className="mt-1 text-xs text-muted-foreground">
                Use descriptive names so operators can toggle layers quickly.
              </p>
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Kind
                </label>
                <Select
                  value={local.kind}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, kind: e.target.value as MapLayerKind }))
                  }
                  className="mt-1"
                >
                  <option value="overlay">Overlay</option>
                  <option value="base">Base map</option>
                </Select>
 <p className="mt-1 text-xs text-muted-foreground">
                  Base maps are exclusive; overlays stack.
                </p>
              </div>

              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Source type
                </label>
                <Select
                  value={local.source_type}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, source_type: e.target.value as MapLayerSourceType }))
                  }
                  className="mt-1"
                >
                  <option value="xyz">XYZ tiles (raster)</option>
                  <option value="arcgis">ArcGIS REST (tile/export)</option>
                  <option value="wms">WMS (GetMap)</option>
                  <option value="geojson">Upload (GeoJSON/KML)</option>
                </Select>
 <p className="mt-1 text-xs text-muted-foreground">
                  Use WMS/ArcGIS for public topo portals; upload for survey exports.
                </p>
              </div>
            </div>
          </div>

          {local.source_type === "xyz" ? (
            <div className="grid gap-4 sm:grid-cols-2">
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Tile URL template
                </label>
                <Input
                  value={local.xyz.url_template}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, xyz: { ...prev.xyz, url_template: e.target.value } }))
                  }
                  placeholder="https://…/{z}/{x}/{y}.png"
                  className="mt-1"
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  Supports `{`z`}`/`{`x`}`/`{`y`}` placeholders.
                </p>
              </div>
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Attribution (optional)
                </label>
                <Input
                  value={local.xyz.attribution ?? ""}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, xyz: { ...prev.xyz, attribution: e.target.value } }))
                  }
                  placeholder="© Provider"
                  className="mt-1"
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  Displayed in the map attribution control (bottom-right).
                </p>
              </div>
              <div className="sm:col-span-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Max zoom (optional)
                </label>
                <NumericDraftInput
                  value={local.xyz.max_zoom ?? undefined}
                  onValueChange={(next) =>
                    setLocal((prev) => ({
                      ...prev,
                      xyz: {
                        ...prev.xyz,
                        max_zoom: typeof next === "number" ? next : undefined,
                      },
                    }))
                  }
                  emptyValue={undefined}
                  integer
                  min={0}
                  max={24}
                  enforceRange
                  clampOnBlur
                  inputMode="numeric"
                  placeholder="19"
 className="mt-1 block w-full rounded-lg border border-border bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:ring-indigo-500"
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  Prevents blank maps when you zoom beyond the tile server&apos;s max zoom (MapLibre will overscale above this).
                </p>
              </div>
            </div>
          ) : null}

          {local.source_type === "arcgis" ? (
            <div className="grid gap-4 sm:grid-cols-3">
              <div className="sm:col-span-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  ArcGIS MapServer URL
                </label>
                <Input
                  value={local.arcgis.base_url}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, arcgis: { ...prev.arcgis, base_url: e.target.value } }))
                  }
                  placeholder="https://…/rest/services/…/MapServer"
                  className="mt-1"
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  For cached services use tile mode; for dynamic services use export mode.
                </p>
              </div>
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Mode
                </label>
                <Select
                  value={local.arcgis.mode}
                  onChange={(e) =>
                    setLocal((prev) => ({
                      ...prev,
                      arcgis: { ...prev.arcgis, mode: e.target.value as "tile" | "export" },
                    }))
                  }
                  className="mt-1"
                >
                  <option value="tile">tile/{`{z}`}/{`{y}`}/{`{x}`}</option>
                  <option value="export">export (bbox tiles)</option>
                </Select>
              </div>
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Max zoom (tile mode)
                </label>
                <NumericDraftInput
                  value={local.arcgis.max_zoom ?? undefined}
                  onValueChange={(next) =>
                    setLocal((prev) => ({
                      ...prev,
                      arcgis: {
                        ...prev.arcgis,
                        max_zoom: typeof next === "number" ? next : undefined,
                      },
                    }))
                  }
                  emptyValue={undefined}
                  integer
                  min={0}
                  max={24}
                  enforceRange
                  clampOnBlur
                  inputMode="numeric"
                  placeholder="16"
 className="mt-1 block w-full rounded-lg border border-border bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:ring-indigo-500"
                />
              </div>
              <div className="sm:col-span-3">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Attribution (optional)
                </label>
                <Input
                  value={local.arcgis.attribution ?? ""}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, arcgis: { ...prev.arcgis, attribution: e.target.value } }))
                  }
                  placeholder="© Provider"
                  className="mt-1"
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  Displayed in the map attribution control (bottom-right).
                </p>
              </div>
            </div>
          ) : null}

          {local.source_type === "wms" ? (
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="sm:col-span-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  WMS base URL
                </label>
                <Input
                  value={local.wms.base_url}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, wms: { ...prev.wms, base_url: e.target.value } }))
                  }
                  placeholder="https://…/wms"
                  className="mt-1"
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  Must support EPSG:3857 tiles via `{`{bbox-epsg-3857}`}`.
                </p>
              </div>
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Layers
                </label>
                <Input
                  value={local.wms.layers}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, wms: { ...prev.wms, layers: e.target.value } }))
                  }
                  placeholder="layer1,layer2"
                  className="mt-1"
                />
              </div>
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Version / format
                </label>
                <div className="mt-1 grid grid-cols-2 gap-2">
                  <Input
                    value={local.wms.version ?? "1.3.0"}
                    onChange={(e) =>
                      setLocal((prev) => ({ ...prev, wms: { ...prev.wms, version: e.target.value } }))
                    }
                    placeholder="1.3.0"
                  />
                  <Input
                    value={local.wms.format ?? "image/png"}
                    onChange={(e) =>
                      setLocal((prev) => ({ ...prev, wms: { ...prev.wms, format: e.target.value } }))
                    }
                    placeholder="image/png"
                  />
                </div>
              </div>
              <div className="sm:col-span-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Attribution (optional)
                </label>
                <Input
                  value={local.wms.attribution ?? ""}
                  onChange={(e) =>
                    setLocal((prev) => ({ ...prev, wms: { ...prev.wms, attribution: e.target.value } }))
                  }
                  placeholder="© Provider"
                  className="mt-1"
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  Displayed in the map attribution control (bottom-right).
                </p>
              </div>
            </div>
          ) : null}

          {showGeoUpload ? (
            <Card className="rounded-lg gap-0 bg-card-inset p-4">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <div className="text-sm font-semibold text-card-foreground">
                    Upload survey overlay
                  </div>
 <div className="mt-1 text-xs text-muted-foreground">
                    Supported: GeoJSON (`.geojson` / `.json`) and KML (`.kml`). Large rasters (GeoTIFF) should be served via WMS/tiles.
                  </div>
                </div>
                <input
                  type="file"
                  accept=".json,.geojson,.kml,application/json,application/vnd.google-earth.kml+xml"
                  onChange={(e) => void handleUpload(e.target.files?.[0] ?? null)}
 className="text-xs text-foreground file:mr-3 file:rounded-lg file:border-0 file:bg-indigo-600 file:px-3 file:py-2 file:text-xs file:font-semibold file:text-white hover:file:bg-indigo-700"
                />
              </div>

              <div className="mt-3 grid gap-3 sm:grid-cols-2">
                <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    File name
                  </label>
                  <Input
                    value={local.geojson.file_name}
                    onChange={(e) =>
                      setLocal((prev) => ({ ...prev, geojson: { ...prev.geojson, file_name: e.target.value } }))
                    }
                    placeholder="survey.geojson"
                    className="mt-1"
                  />
                </div>
 <div className="text-xs text-muted-foreground">
                  {local.geojson.data ? (
                    <InlineBanner tone="success" className="px-3 py-2">
                      Parsed upload: ready to save.
                    </InlineBanner>
                  ) : (
                    <Card className="rounded-lg gap-0 px-3 py-2">
                      No file loaded yet.
                    </Card>
                  )}
                </div>
              </div>
            </Card>
          ) : null}

          <div className="grid gap-4 sm:grid-cols-2">
 <label className="flex items-center gap-2 text-sm text-foreground">
              <input
                type="checkbox"
                checked={local.enabled}
                onChange={(e) => setLocal((prev) => ({ ...prev, enabled: e.target.checked }))}
 className="rounded border-input text-indigo-600 focus:ring-indigo-500"
              />
              Enabled
            </label>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Opacity ({Math.round(local.opacity * 100)}%)
              </label>
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                value={local.opacity}
                onChange={(e) =>
                  setLocal((prev) => ({ ...prev, opacity: Number.parseFloat(e.target.value) }))
                }
                className="mt-2 w-full"
              />
            </div>
          </div>

          {error ? (
            <InlineBanner tone="danger" className="rounded-lg px-3 py-2">{error}</InlineBanner>
          ) : null}
        </div>

        <div className="mt-6 flex items-center justify-end gap-3">
          <NodeButton onClick={onClose}>Cancel</NodeButton>
          <NodeButton
            variant="primary"
            disabled={busy || !canSubmit}
            onClick={async () => {
              setBusy(true);
              setError(null);
              try {
                if (mode === "create") {
                  await onCreate(local);
                } else {
                  await onUpdate(local);
                }
              } catch (err) {
                setError(err instanceof Error ? err.message : "Failed to save layer.");
                setBusy(false);
                return;
              }
              setBusy(false);
            }}
          >
            {busy ? "Saving\u2026" : mode === "create" ? "Add layer" : "Save changes"}
          </NodeButton>
        </div>
      </DialogContent>
    </Dialog>
  );
}
