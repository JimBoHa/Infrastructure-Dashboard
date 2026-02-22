"use client";

import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { pollForecasts, updateWeatherForecastConfig } from "@/lib/api";
import { queryKeys, useWeatherForecastConfigQuery } from "@/lib/queries";

import type { Message } from "../types";

export type WeatherForecastDraftModel = {
  enabled: boolean;
  latitude: string;
  longitude: string;
  isSaving: boolean;
  savedAt: string | null;
  weatherLocation: { latitude: string; longitude: string } | null;
  setEnabled: (value: boolean) => void;
  setLatitude: (value: string) => void;
  setLongitude: (value: string) => void;
  save: () => Promise<void>;
  refreshNow: () => Promise<void>;
};

export const useWeatherForecastDraft = (
  onMessage: (message: Message) => void,
): WeatherForecastDraftModel => {
  const queryClient = useQueryClient();
  const weatherForecastConfigQuery = useWeatherForecastConfigQuery();

  const [enabled, setEnabled] = useState(false);
  const [latitude, setLatitude] = useState("");
  const [longitude, setLongitude] = useState("");
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    const cfg = weatherForecastConfigQuery.data;
    if (!cfg) return;
    setEnabled(Boolean(cfg.enabled));
    setLatitude(cfg.latitude != null ? String(cfg.latitude) : "");
    setLongitude(cfg.longitude != null ? String(cfg.longitude) : "");
  }, [weatherForecastConfigQuery.data]);

  const savedAt = weatherForecastConfigQuery.data?.updated_at ?? null;

  const weatherLocation = useMemo(() => {
    const lat =
      latitude.trim() ||
      (weatherForecastConfigQuery.data?.latitude != null
        ? String(weatherForecastConfigQuery.data.latitude)
        : "");
    const lon =
      longitude.trim() ||
      (weatherForecastConfigQuery.data?.longitude != null
        ? String(weatherForecastConfigQuery.data.longitude)
        : "");
    if (!lat || !lon) return null;
    return { latitude: lat, longitude: lon };
  }, [
    latitude,
    longitude,
    weatherForecastConfigQuery.data?.latitude,
    weatherForecastConfigQuery.data?.longitude,
  ]);

  const save = async () => {
    const parsedLatitude = Number.parseFloat(latitude);
    const parsedLongitude = Number.parseFloat(longitude);
    if (!Number.isFinite(parsedLatitude) || !Number.isFinite(parsedLongitude)) {
      onMessage({ type: "error", text: "Enter numeric latitude/longitude (degrees)." });
      return;
    }
    setIsSaving(true);
    try {
      await updateWeatherForecastConfig({
        enabled,
        latitude: parsedLatitude,
        longitude: parsedLongitude,
        provider: "open_meteo",
      });
      await queryClient.invalidateQueries({ queryKey: queryKeys.weatherForecastConfig });
      onMessage({ type: "success", text: "Saved weather forecast location." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to save weather forecast config.";
      onMessage({ type: "error", text });
    } finally {
      setIsSaving(false);
    }
  };

  const refreshNow = async () => {
    try {
      await pollForecasts();
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.forecastStatus }),
        queryClient.invalidateQueries({ queryKey: queryKeys.weatherForecastHourly(72) }),
        queryClient.invalidateQueries({ queryKey: queryKeys.weatherForecastDaily(7) }),
        queryClient.invalidateQueries({ queryKey: ["forecast", "pv", "hourly"] }),
        queryClient.invalidateQueries({ queryKey: ["forecast", "pv", "daily"] }),
      ]);
      onMessage({ type: "success", text: "Triggered forecast refresh." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to refresh forecasts.";
      onMessage({ type: "error", text });
    }
  };

  return {
    enabled,
    latitude,
    longitude,
    isSaving,
    savedAt,
    weatherLocation,
    setEnabled,
    setLatitude,
    setLongitude,
    save,
    refreshNow,
  };
};

