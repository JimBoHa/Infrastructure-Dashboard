export type SetupCredential = {
  name: string;
  has_value: boolean;
  metadata: Record<string, unknown>;
  created_at?: string | null;
  updated_at?: string | null;
};

export type EmporiaDevice = {
  device_gid: string;
  name?: string | null;
  model?: string | null;
  firmware?: string | null;
  address?: string | null;
};

export type EmporiaCircuitSettings = {
  circuit_key: string;
  name: string;
  raw_channel_num?: string | null;
  nested_device_gid?: string | null;
  enabled: boolean;
  hidden: boolean;
  include_in_power_summary: boolean;
  is_mains: boolean;
};

export type EmporiaLoginResult = {
  token_present: boolean;
  site_ids: string[];
  devices: EmporiaDevice[];
};

export type EmporiaDeviceSettings = EmporiaDevice & {
  enabled: boolean;
  hidden?: boolean;
  include_in_power_summary: boolean;
  group_label?: string | null;
  circuits?: EmporiaCircuitSettings[];
};

export type EmporiaDevicesResult = {
  token_present: boolean;
  site_ids: string[];
  devices: EmporiaDeviceSettings[];
};

export type EmporiaDeviceUpdate = {
  device_gid: string;
  enabled?: boolean;
  hidden?: boolean;
  include_in_power_summary?: boolean;
  group_label?: string | null;
  circuits?: Array<{
    circuit_key: string;
    enabled?: boolean;
    hidden?: boolean;
    include_in_power_summary?: boolean;
  }>;
};
