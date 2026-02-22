export type ForecastProviderStatus = {
  status: string;
  last_seen?: string | null;
  details?: string | null;
  meta?: Record<string, unknown>;
};

export type ForecastStatus = {
  enabled: boolean;
  providers: Record<string, ForecastProviderStatus>;
};

export type WeatherForecastConfig = {
  enabled: boolean;
  provider: string | null;
  latitude: number | null;
  longitude: number | null;
  updated_at: string | null;
};

export type PvForecastConfig = {
  enabled: boolean;
  provider: string;
  latitude: number;
  longitude: number;
  tilt_deg: number;
  azimuth_deg: number;
  kwp: number;
  time_format: string;
  updated_at: string;
};

export type ForecastSeriesPoint = {
  timestamp: string;
  value: number;
};

export type ForecastSeriesMetric = {
  unit: string;
  points: ForecastSeriesPoint[];
};

export type ForecastSeriesResponse = {
  provider: string;
  kind: string;
  subject_kind: string;
  subject: string;
  issued_at: string;
  metrics: Record<string, ForecastSeriesMetric>;
};

export type CurrentWeatherMetric = {
  unit: string;
  value: number;
};

export type CurrentWeatherResponse = {
  provider: string;
  latitude: number;
  longitude: number;
  observed_at: string;
  fetched_at: string;
  metrics: Record<string, CurrentWeatherMetric>;
};
