"use client";

import { useMemo } from "react";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import NodeButton from "@/features/nodes/components/NodeButton";
import type { MapLayer } from "@/types/map";

type BaseMapChoice = {
  key: "streets" | "satellite" | "topo";
  label: string;
  offline: MapLayer | null;
  online: MapLayer | null;
  effective: MapLayer | null;
};

type MapBaseLayerCardProps = {
  canEdit: boolean;
  baseLayers: MapLayer[];
  baseLayerBySystemKey: Map<string, MapLayer>;
  selectedBaseLayer: MapLayer | null;
  swantonInstalled: boolean;
  showAllBaseLayers: boolean;
  onToggleShowAllBaseLayers: () => void;
  onSelectBaseLayer: (layerId: number) => void;
  onEditLayer: (layer: MapLayer) => void;
};

export default function MapBaseLayerCard({
  canEdit,
  baseLayers,
  baseLayerBySystemKey,
  selectedBaseLayer,
  swantonInstalled,
  showAllBaseLayers,
  onToggleShowAllBaseLayers,
  onSelectBaseLayer,
  onEditLayer,
}: MapBaseLayerCardProps) {
  const baseMapChoices = useMemo(() => {
    const buildChoice = (
      key: BaseMapChoice["key"],
      label: string,
      offlineKey: string,
      onlineKey: string,
    ): BaseMapChoice => {
      const offline = baseLayerBySystemKey.get(offlineKey) ?? null;
      const online = baseLayerBySystemKey.get(onlineKey) ?? null;
      const effective = swantonInstalled && offline ? offline : online ?? offline;
      return { key, label, offline, online, effective };
    };
    return [
      buildChoice("streets", "Streets", "offline_streets", "streets"),
      buildChoice("satellite", "Satellite", "offline_satellite", "satellite"),
      buildChoice("topo", "Topo", "offline_topo", "topo"),
    ];
  }, [baseLayerBySystemKey, swantonInstalled]);

  return (
    <CollapsibleCard
      density="sm"
      title="Base map"
      description="Toggle between streets, satellite, and topo. Offline map tiles require installing the Swanton pack above."
      actions={
        <div className="flex items-center gap-2">
          <NodeButton size="sm" onClick={onToggleShowAllBaseLayers} disabled={!baseLayers.length}>
            {showAllBaseLayers ? "Hide list" : "Show list"}
          </NodeButton>
          <NodeButton
            size="sm"
            onClick={() => (selectedBaseLayer ? onEditLayer(selectedBaseLayer) : null)}
            disabled={!canEdit || !selectedBaseLayer}
          >
            Edit source
          </NodeButton>
        </div>
      }
    >
      <div className="mt-3 grid grid-cols-3 gap-2">
        {baseMapChoices.map((choice) => {
          const effective = choice.effective;
          if (!effective) return null;
          const active = selectedBaseLayer?.id === effective.id;
          const isOffline = effective.system_key?.startsWith("offline_");
          const needsPack = isOffline && !swantonInstalled;

          return (
            <button
              key={choice.key}
              type="button"
              onClick={() => onSelectBaseLayer(effective.id)}
              disabled={needsPack}
              className={[
                "rounded-lg border px-3 py-2 text-left text-xs font-semibold transition disabled:opacity-50",
                active
 ? "border-transparent bg-indigo-600 text-white shadow-xs hover:bg-indigo-700"
 : "border-border bg-white text-foreground shadow-xs hover:bg-muted",
              ].join(" ")}
            >
              <div className="truncate">{choice.label}</div>
              <div className="mt-1 flex items-center gap-2 text-[10px] font-semibold">
                <span
                  className={[
                    "rounded-full px-2 py-0.5",
                    isOffline
                      ? "bg-white/20 text-white"
                      : active
                        ? "bg-white/20 text-white"
 : "bg-muted text-muted-foreground",
                  ].join(" ")}
                >
                  {isOffline ? "Offline" : "Internet"}
                </span>
 {needsPack ? <span className="text-rose-200">Install pack</span> : null}
              </div>
            </button>
          );
        })}
      </div>

      {!swantonInstalled ? (
        <InlineBanner tone="warning" className="mt-3 px-3 py-2 text-xs">
          Offline tiles are not installed yet. Install the Swanton pack above so the map works without internet after setup.
        </InlineBanner>
      ) : null}

      {showAllBaseLayers ? (
        <div className="mt-4 space-y-2">
 <div className="text-xs font-semibold text-foreground">All base layers</div>
          <div className="grid grid-cols-2 gap-2">
            {baseLayers.map((layer) => {
              const active = selectedBaseLayer?.id === layer.id;
              const config = (layer.config ?? {}) as Record<string, unknown>;
              const requiresInternet = config.requires_internet === true;
              const packId = typeof config.offline_pack_id === "string" ? config.offline_pack_id : null;
              const disabled = packId === "swanton_ca" && !swantonInstalled;
              return (
                <button
                  key={layer.id}
                  type="button"
                  disabled={disabled}
                  onClick={() => onSelectBaseLayer(layer.id)}
                  className={[
                    "rounded-lg border px-3 py-2 text-left text-xs font-semibold transition disabled:opacity-50",
                    active
 ? "border-transparent bg-indigo-600 text-white shadow-xs hover:bg-indigo-700"
 : "border-border bg-white text-foreground shadow-xs hover:bg-muted",
                  ].join(" ")}
                >
                  <div className="truncate">{layer.name}</div>
 <div className="mt-1 text-[10px] text-muted-foreground">
                    {packId
                      ? `Offline pack: ${packId}`
                      : requiresInternet
                        ? "Requires internet"
                        : layer.source_type.toUpperCase()}
                  </div>
                </button>
              );
            })}
          </div>
        </div>
      ) : null}
    </CollapsibleCard>
  );
}
