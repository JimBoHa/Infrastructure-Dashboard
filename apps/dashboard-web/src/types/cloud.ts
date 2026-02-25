export type CloudRuntimeRole = "local" | "cloud" | string;

export type CloudAccessConfig = {
  role: CloudRuntimeRole;
  local_site_key: string | null;
  cloud_server_base_url: string | null;
  sync_interval_seconds: number;
  sync_enabled: boolean;
  last_attempt_at: string | null;
  last_success_at: string | null;
  last_error: string | null;
  registered_site_count: number;
};

export type CloudAccessUpdateRequest = {
  cloud_server_base_url?: string;
  sync_interval_seconds?: number;
  sync_enabled?: boolean;
};

export type CloudSite = {
  site_id: string;
  site_name: string;
  key_fingerprint: string;
  enabled: boolean;
  created_at: string | null;
  updated_at: string | null;
  last_ingested_at: string | null;
  last_payload_bytes: number | null;
  last_metrics_count: number | null;
};

export type RegisterCloudSiteRequest = {
  site_name: string;
  site_key: string;
};
