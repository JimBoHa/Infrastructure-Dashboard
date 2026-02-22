"use client";

import Link from "next/link";
import { MermaidDiagram } from "@/components/MermaidDiagram";
import CollapsibleCard from "@/components/CollapsibleCard";
import PageHeaderCard from "@/components/PageHeaderCard";
import LocalSensorVisualizations from "@/features/overview/components/LocalSensorVisualizations";

export default function OverviewPage() {
  const siteMapTooltips: Record<string, string> = {
    "Farm Dashboard": "High-level entry point to the controller UI.",
    "Operations tabs": "Day-to-day monitoring and configuration surfaces.",
    "Analytics tabs": "Analytical dashboards and deep-dive tools.",
    Overview: "System-wide status and quick entry points.",
    Nodes: "Scan, adopt, and inspect node health and inventory.",
    Map: "Place nodes/sensors; draw polygons/lines; manage saved map views.",
    "Sensors & Outputs": "Rename sensors/outputs, set decimals, alarms, and send output commands.",
    Alarms: "Create and manage rule-based alarms, monitor active incidents, and acknowledge events.",
    Schedules: "Create automation schedules (calendar-style blocks).",
    "Analytics Overview": "System totals and provider health; forecasts and rollups.",
    Trends: "Ad-hoc charts + correlation; export CSV.",
    "Temp Compensation": "Assisted workflow to compensate temperature-driven sensor drift.",
    Power: "Per-node electrical dashboards (kW/W, V, A) for Renogy/Emporia.",
    Backups: "Backups/retention and controller settings export/restore.",
    "Admin tabs": "Installer, credentials, and administrative configuration.",
    "Setup Center": "Credentials and integrations (Emporia, Forecast.Solar, weather).",
    Deployment: "Deploy and adopt new Pi nodes over SSH.",
    Connection: "Local/cloud endpoints and connectivity status.",
    Users: "User accounts and capabilities.",
    "Key services": "Core runtime services in the controller stack.",
    "Core server": "API + auth, config orchestration, storage, analytics composition.",
    Postgres: "Stores sensors, configs, and time-series samples.",
    MQTT: "Telemetry transport between nodes and controller.",
    "Node agents": "Run on each node; read hardware and publish telemetry.",
  };

  const siteMapDiagram = `
flowchart LR
  FD[Farm Dashboard]:::root

  FD --> OPS[Operations]:::group
  FD --> ANA[Analytics]:::group
  FD --> ADM[Admin]:::group
  FD --> SYS[System]:::group

  subgraph OPS[Operations tabs]
    OVR[Overview]
    NODES[Nodes]
    MAP[Map]
    SO[Sensors & Outputs]
    ALM[Alarms]
    SCH[Schedules]
    BK[Backups]
  end

  subgraph ANA[Analytics tabs]
    AN_OVR[Analytics Overview]
    AN_TR[Trends]
    AN_COMP[Temp Compensation]
    AN_PWR[Power]
  end

  subgraph ADM[Admin tabs]
    SETUP[Setup Center]
    DEP[Deployment]
    CONN[Connection]
    USERS[Users]
  end

  subgraph SYS[Key services]
    CORE[Core server]
    DB[Postgres]
    MQTT[MQTT]
    NAGENT[Node agents]
  end

  CORE --> DB
  CORE --> MQTT
  CORE --> NAGENT

  click OVR "/overview" "System-wide status and quick entry points."
  click NODES "/nodes" "Scan, adopt, and inspect node health and sensors."
  click MAP "/map" "Place nodes/sensors; draw polygons/lines; manage saved map views."
  click SO "/sensors" "Rename sensors/outputs, decimals, alarms, and command outputs."
  click ALM "/alarms" "Create and manage rule-based alarms and incident history."
  click SCH "/schedules" "Create automation schedules (calendar-style blocks)."
  click BK "/backups" "Backups/retention and controller settings export/restore."

  click AN_OVR "/analytics" "System totals and provider health; forecasts and rollups."
  click AN_TR "/analytics/trends" "Ad-hoc charts + correlation; export CSV."
  click AN_COMP "/analytics/compensation" "Assisted temperature drift compensation."
  click AN_PWR "/analytics/power" "Per-node electrical dashboards (kW/W, V, A) for Renogy/Emporia."

  click SETUP "/setup" "Credentials and integrations (Emporia, Forecast.Solar, weather)."
  click DEP "/deployment" "Deploy and adopt new Pi nodes over SSH."
  click CONN "/connection" "Local/cloud endpoints and connectivity status."
  click USERS "/users" "User accounts and capabilities."

  click CORE "#" "API + auth, config orchestration, storage, analytics composition."
  click DB "#" "Stores sensors, configs, and time-series samples."
  click MQTT "#" "Telemetry transport between nodes and controller."
  click NAGENT "#" "Runs on each node; reads hardware and publishes telemetry."

  classDef root fill:#4f46e5,stroke:#4f46e5,color:#ffffff;
  classDef group fill:#eef2ff,stroke:#c7d2fe,color:#111827;
  classDef box fill:#ffffff,stroke:#d1d5db,color:#111827;

  class OPS,ANA,ADM,SYS group;
  class OVR,NODES,MAP,SO,ALM,SCH,BK,AN_OVR,AN_TR,AN_COMP,AN_PWR box;
  class SETUP,DEP,CONN,USERS box;
  class CORE,DB,MQTT,NAGENT box;
`;

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Overview"
        description="System-wide status and quick entry points. Use the dedicated tabs for detailed configuration."
      />

      <LocalSensorVisualizations />

      <CollapsibleCard
        title="Where things live"
        description="High-level site map. Hover nodes for details; click to navigate."
        defaultOpen={false}
        actions={
          <Link
            href="/setup"
 className="inline-flex items-center justify-center rounded-lg border border-border bg-white px-3 py-2 text-sm font-semibold text-foreground shadow-xs hover:bg-muted focus:outline-hidden focus:bg-card-inset"
          >
            Setup Center
          </Link>
        }
      >
        <MermaidDiagram
          diagram={siteMapDiagram}
          ariaLabel="Farm Dashboard site map"
          tooltips={siteMapTooltips}
        />
      </CollapsibleCard>
    </div>
  );
}
