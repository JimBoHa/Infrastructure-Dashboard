"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import InlineBanner from "@/components/InlineBanner";
import CollapsibleCard from "@/components/CollapsibleCard";
import AnalyticsHeaderCard from "@/features/analytics/components/AnalyticsHeaderCard";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useAuth } from "@/components/AuthProvider";
import { postJson } from "@/lib/http";
import { queryKeys, useMetricsQuery, useNodesQuery, useSensorsQuery } from "@/lib/queries";
import { formatNumber } from "@/lib/format";
import { formatDateTimeInputValue, parseDateTimeInputValueToIso, useControllerTimeZone } from "@/lib/siteTime";
import { AnalyticsRangeSelect, type AnalyticsHistoryRangeHours } from "@/features/analytics/components/AnalyticsShared";
import {
  alignSeriesByTimestamp,
  applyTemperatureCompensation,
  buildTempCompensationExpression,
  computeTemperatureDriftCorrection,
  fitTemperatureDriftModel,
  suggestTemperatureLagSeconds,
  type DriftModelDegree,
  type TempCompensationFitResult,
} from "@/lib/tempCompensation";
import {
  TemperatureCompensationCorrectionChart,
  TemperatureCompensationScatterChart,
  TemperatureCompensationTimeSeriesChart,
  formatLiveCorrectionPreview,
} from "@/features/compensation/components/TemperatureCompensationCharts";

function defaultIntervalForRangeHours(rangeHours: number): number {
  if (rangeHours <= 24) return 60; // 1 min
  if (rangeHours <= 72) return 300; // 5 min
  if (rangeHours <= 168) return 900; // 15 min
  if (rangeHours <= 720) return 3600; // 1 hour
  if (rangeHours <= 2160) return 21600; // 6 hours
  if (rangeHours <= 8760) return 43200; // 12 hours
  return 86400; // 1 day
}

const INTERVAL_OPTIONS_SECONDS: Array<{ value: number; label: string }> = [
  { value: 30, label: "30s" },
  { value: 60, label: "1 min" },
  { value: 300, label: "5 min" },
  { value: 900, label: "15 min" },
  { value: 3600, label: "1 hour" },
  { value: 21600, label: "6 hours" },
  { value: 43200, label: "12 hours" },
  { value: 86400, label: "1 day" },
];

const DEGREE_OPTIONS: Array<{ value: DriftModelDegree; label: string; hint: string }> = [
  { value: 1, label: "Linear", hint: "Best first try. Stable and easy to reason about." },
  { value: 2, label: "Quadratic", hint: "Handles gentle curvature when drift is not linear." },
  { value: 3, label: "Cubic", hint: "More flexible but easier to overfit; use with care." },
];

function looksLikeTemperatureSensor(sensor: { type: string; unit: string; name: string }) {
  const unit = sensor.unit.trim().toLowerCase();
  const type = sensor.type.trim().toLowerCase();
  const name = sensor.name.trim().toLowerCase();
  if (unit === "c" || unit === "°c" || unit === "degc") return true;
  if (unit === "f" || unit === "°f" || unit === "degf") return true;
  if (type.includes("temp")) return true;
  if (name.includes("temp")) return true;
  return false;
}

function formatSigned(value: number, digits = 4): string {
  if (!Number.isFinite(value)) return "\u2014";
  const sign = value > 0 ? "+" : value < 0 ? "\u2212" : "";
  return `${sign}${formatNumber(Math.abs(value), { maximumFractionDigits: digits })}`;
}

export default function TemperatureCompensationPageClient() {
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));
  const timeZone = useControllerTimeZone();

  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();

  const [rawSensorId, setRawSensorId] = useState("");
  const [tempSensorId, setTempSensorId] = useState("");
  const [sensorSearch, setSensorSearch] = useState("");

  const [rangeHours, setRangeHours] = useState<AnalyticsHistoryRangeHours>(72);
  const [rangeMode, setRangeMode] = useState<"preset" | "custom">("preset");
  const [rangeStartLocal, setRangeStartLocal] = useState<string>("");
  const [rangeEndLocal, setRangeEndLocal] = useState<string>("");
  const [intervalMode, setIntervalMode] = useState<"auto" | "custom">("auto");
  const [intervalSeconds, setIntervalSeconds] = useState<number>(defaultIntervalForRangeHours(72));

  const [degree, setDegree] = useState<DriftModelDegree>(1);
  const [centerMode, setCenterMode] = useState<"auto" | "custom">("auto");
  const [customCenterTemp, setCustomCenterTemp] = useState<string>("");
  const [clampAbsRaw, setClampAbsRaw] = useState<string>("");
  const [outlierTrimPctRaw, setOutlierTrimPctRaw] = useState<string>("0");
  const [includeTimeSlope, setIncludeTimeSlope] = useState<boolean>(true);
  const [includeTimeSlopeTouched, setIncludeTimeSlopeTouched] = useState<boolean>(false);
  const [tempLagMode, setTempLagMode] = useState<"auto" | "custom">("auto");
  const [tempLagMinutesRaw, setTempLagMinutesRaw] = useState<string>("");

  const [outputName, setOutputName] = useState("");
  const [outputIntervalSecondsRaw, setOutputIntervalSecondsRaw] = useState<string>("");
  const [outputRollingAvgSecondsRaw, setOutputRollingAvgSecondsRaw] = useState<string>("");
  const [creating, setCreating] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string; linkHref?: string } | null>(null);

  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);

  const nodeNameById = useMemo(() => new Map(nodes.map((n) => [n.id, n.name])), [nodes]);

  const rawSensor = useMemo(
    () => sensors.find((s) => s.sensor_id === rawSensorId) ?? null,
    [rawSensorId, sensors],
  );
  const tempSensor = useMemo(
    () => sensors.find((s) => s.sensor_id === tempSensorId) ?? null,
    [tempSensorId, sensors],
  );

  const customRange = useMemo(() => {
    if (rangeMode !== "custom") {
      return { startIso: null, endIso: null, hours: null, error: null as string | null };
    }

    const startIso = parseDateTimeInputValueToIso(rangeStartLocal, timeZone);
    const endIso = parseDateTimeInputValueToIso(rangeEndLocal, timeZone);
    if (!startIso || !endIso) {
      return { startIso: null, endIso: null, hours: null, error: "Enter a valid start and end date/time." };
    }

    const start = new Date(startIso);
    const end = new Date(endIso);
    if (!(start < end)) {
      return { startIso: null, endIso: null, hours: null, error: "Start must be before end." };
    }

    const hours = (end.getTime() - start.getTime()) / (60 * 60 * 1000);
    if (hours > 8760) {
      return { startIso: null, endIso: null, hours: null, error: "Max range is 365d." };
    }

    return { startIso, endIso, hours, error: null as string | null };
  }, [rangeEndLocal, rangeMode, rangeStartLocal, timeZone]);

  const effectiveRangeHours = useMemo(() => {
    if (rangeMode === "custom" && customRange.hours != null) return customRange.hours;
    return rangeHours;
  }, [customRange.hours, rangeHours, rangeMode]);

  const effectiveIntervalSeconds = useMemo(() => {
    if (intervalMode === "auto") return defaultIntervalForRangeHours(effectiveRangeHours);
    return intervalSeconds;
  }, [effectiveRangeHours, intervalMode, intervalSeconds]);

  useEffect(() => {
    if (includeTimeSlopeTouched) return;
    setIncludeTimeSlope(effectiveRangeHours >= 48);
  }, [effectiveRangeHours, includeTimeSlopeTouched]);

  const metricsQueryEnabled =
    Boolean(rawSensorId && tempSensorId) &&
    (rangeMode === "preset" ||
      (customRange.error == null && customRange.startIso != null && customRange.endIso != null));

  const metricsQuery = useMetricsQuery({
    sensorIds: rawSensorId && tempSensorId ? [rawSensorId, tempSensorId] : [],
    rangeHours: effectiveRangeHours,
    interval: effectiveIntervalSeconds,
    start: rangeMode === "custom" && customRange.error == null ? customRange.startIso ?? undefined : undefined,
    end: rangeMode === "custom" && customRange.error == null ? customRange.endIso ?? undefined : undefined,
    enabled: metricsQueryEnabled,
  });

  const metricsSeries = metricsQuery.data;

  const metricsBySensorId = useMemo(() => {
    const map = new Map<string, NonNullable<typeof metricsSeries>[number]>();
    for (const series of metricsSeries ?? []) {
      map.set(series.sensor_id, series);
    }
    return map;
  }, [metricsSeries]);

  const lagSuggestion = useMemo(() => {
    const rawSeries = rawSensorId ? metricsBySensorId.get(rawSensorId) ?? null : null;
    const tempSeries = tempSensorId ? metricsBySensorId.get(tempSensorId) ?? null : null;
    if (!rawSeries || !tempSeries) return null;
    const clampAbsForLag = (() => {
      const parsed = Number.parseFloat(clampAbsRaw.trim());
      if (!Number.isFinite(parsed) || parsed <= 0) return null;
      return parsed;
    })();
    return suggestTemperatureLagSeconds({
      rawSeries,
      temperatureSeries: tempSeries,
      intervalSeconds: effectiveIntervalSeconds,
      degree,
      includeTimeSlope,
      clampAbs: clampAbsForLag,
    });
  }, [clampAbsRaw, degree, effectiveIntervalSeconds, includeTimeSlope, metricsBySensorId, rawSensorId, tempSensorId]);

  const effectiveTemperatureLagSeconds = useMemo(() => {
    if (tempLagMode === "custom") {
      const parsed = Number.parseFloat(tempLagMinutesRaw.trim());
      if (!Number.isFinite(parsed) || parsed <= 0) return 0;
      return Math.max(0, Math.round(parsed * 60));
    }
    return lagSuggestion?.lagSeconds ?? 0;
  }, [lagSuggestion?.lagSeconds, tempLagMinutesRaw, tempLagMode]);

  const alignedPoints = useMemo(() => {
    const rawSeries = rawSensorId ? metricsBySensorId.get(rawSensorId) ?? null : null;
    const tempSeries = tempSensorId ? metricsBySensorId.get(tempSensorId) ?? null : null;
    const points = alignSeriesByTimestamp(rawSeries, tempSeries, { temperatureLagSeconds: effectiveTemperatureLagSeconds });
    return points.slice().sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());
  }, [effectiveTemperatureLagSeconds, metricsBySensorId, rawSensorId, tempSensorId]);

  const centerTempValue = useMemo(() => {
    if (centerMode !== "custom") return undefined;
    const parsed = Number.parseFloat(customCenterTemp.trim());
    return Number.isFinite(parsed) ? parsed : undefined;
  }, [centerMode, customCenterTemp]);

  const clampAbs = useMemo(() => {
    const parsed = Number.parseFloat(clampAbsRaw.trim());
    if (!Number.isFinite(parsed) || parsed <= 0) return null;
    return parsed;
  }, [clampAbsRaw]);

  const outlierTrimPct = useMemo(() => {
    const parsed = Number.parseFloat(outlierTrimPctRaw.trim());
    if (!Number.isFinite(parsed) || parsed <= 0) return 0;
    return Math.max(0, Math.min(10, parsed));
  }, [outlierTrimPctRaw]);

  const fit = useMemo<TempCompensationFitResult | null>(() => {
    if (alignedPoints.length < 20) return null;
    const initial = fitTemperatureDriftModel({
      points: alignedPoints,
      degree,
      centerTemp: centerTempValue,
      includeTimeSlope,
    });
    if (!initial) return null;
    if (outlierTrimPct <= 0) return initial;

    const yHat = alignedPoints.map((pt) => {
      const x = pt.temperature - initial.centerTemp;
      let pred = initial.coefficients[0] ?? 0;
      let xPow = x;
      for (let k = 1; k < initial.coefficients.length; k += 1) {
        pred += (initial.coefficients[k] ?? 0) * xPow;
        xPow *= x;
      }
      if (initial.timeSlopePerDay != null && Number.isFinite(initial.centerTimeMs)) {
        const timeDays = (pt.timestamp.getTime() - initial.centerTimeMs) / 86_400_000;
        pred += initial.timeSlopePerDay * timeDays;
      }
      return pred;
    });

    const residuals = alignedPoints.map((pt, i) => Math.abs(pt.raw - (yHat[i] ?? pt.raw)));
    const sorted = residuals.slice().sort((a, b) => a - b);
    const keepPct = Math.max(0.5, 1 - outlierTrimPct / 100);
    const cutoffIndex = Math.max(0, Math.min(sorted.length - 1, Math.floor(sorted.length * keepPct)));
    const cutoff = sorted[cutoffIndex] ?? sorted[sorted.length - 1] ?? Infinity;
    const trimmed = alignedPoints.filter((_pt, i) => residuals[i] <= cutoff);

    const refined =
      fitTemperatureDriftModel({
        points: trimmed,
        degree,
        centerTemp: initial.centerTemp,
        includeTimeSlope,
      }) ?? initial;
    return refined;
  }, [alignedPoints, centerTempValue, degree, includeTimeSlope, outlierTrimPct]);

  const expressionPreview = useMemo(() => {
    if (!fit) return null;
    return buildTempCompensationExpression({
      rawVar: "raw",
      temperatureVar: "t",
      centerTemp: fit.centerTemp,
      coefficients: fit.coefficients,
      clampAbs,
    });
  }, [clampAbs, fit]);

  const livePreview = useMemo(() => {
    const rawLatest = rawSensor?.latest_value ?? null;
    const temperatureLatest = tempSensor?.latest_value ?? null;
    return formatLiveCorrectionPreview({ rawLatest, temperatureLatest, fit });
  }, [fit, rawSensor?.latest_value, tempSensor?.latest_value]);

  const selectableSensors = useMemo(() => {
    const needle = sensorSearch.trim().toLowerCase();
    return sensors
      .filter((sensor) => {
        if (!needle) return true;
        const nodeName = nodeNameById.get(sensor.node_id) ?? sensor.node_id;
        return (
          sensor.name.toLowerCase().includes(needle) ||
          sensor.type.toLowerCase().includes(needle) ||
          sensor.unit.toLowerCase().includes(needle) ||
          nodeName.toLowerCase().includes(needle) ||
          sensor.sensor_id.toLowerCase().includes(needle)
        );
      })
      .slice()
      .sort((a, b) => {
        const nodeA = nodeNameById.get(a.node_id) ?? a.node_id;
        const nodeB = nodeNameById.get(b.node_id) ?? b.node_id;
        if (nodeA !== nodeB) return nodeA.localeCompare(nodeB);
        return a.name.localeCompare(b.name);
      });
  }, [nodeNameById, sensorSearch, sensors]);

  const temperatureCandidates = useMemo(() => selectableSensors.filter(looksLikeTemperatureSensor), [selectableSensors]);

  const sensorGroups = useMemo(() => {
    const byNode = new Map<string, typeof selectableSensors>();
    selectableSensors.forEach((sensor) => {
      const list = byNode.get(sensor.node_id) ?? [];
      list.push(sensor);
      byNode.set(sensor.node_id, list);
    });
    return Array.from(byNode.entries()).sort((a, b) => {
      const nameA = nodeNameById.get(a[0]) ?? a[0];
      const nameB = nodeNameById.get(b[0]) ?? b[0];
      return nameA.localeCompare(nameB);
    });
  }, [nodeNameById, selectableSensors]);

  const temperatureGroups = useMemo(() => {
    const byNode = new Map<string, typeof temperatureCandidates>();
    temperatureCandidates.forEach((sensor) => {
      const list = byNode.get(sensor.node_id) ?? [];
      list.push(sensor);
      byNode.set(sensor.node_id, list);
    });
    return Array.from(byNode.entries()).sort((a, b) => {
      const nameA = nodeNameById.get(a[0]) ?? a[0];
      const nameB = nodeNameById.get(b[0]) ?? b[0];
      return nameA.localeCompare(nameB);
    });
  }, [nodeNameById, temperatureCandidates]);

  const fitSummary = useMemo(() => {
    if (!fit) return null;
    const degreeLabel = DEGREE_OPTIONS.find((opt) => opt.value === fit.degree)?.label ?? `Degree ${fit.degree}`;
    return {
      degreeLabel,
      r2: fit.r2,
      samples: fit.sampleCount,
      tempRange: `${formatNumber(fit.tempMin, { maximumFractionDigits: 2 })} .. ${formatNumber(fit.tempMax, { maximumFractionDigits: 2 })}`,
      centerTemp: fit.centerTemp,
    };
  }, [fit]);

  const fitDiagnostics = useMemo(() => {
    if (!fit || !rawSensor) return null;
    if (alignedPoints.length < 20) return null;

    const percentile = (values: number[], p: number): number | null => {
      if (!values.length) return null;
      const pp = Math.max(0, Math.min(1, p));
      const sorted = values.slice().sort((a, b) => a - b);
      const idx = Math.max(0, Math.min(sorted.length - 1, Math.floor(pp * (sorted.length - 1))));
      return sorted[idx] ?? null;
    };

    const rawValues = alignedPoints.map((pt) => pt.raw).filter((v) => Number.isFinite(v));
    const correctedValues = alignedPoints
      .map((pt) => (fit ? applyTemperatureCompensation(pt.raw, pt.temperature, fit) : null))
      .filter((v): v is number => typeof v === "number" && Number.isFinite(v));

    const rawP5 = percentile(rawValues, 0.05);
    const rawP95 = percentile(rawValues, 0.95);
    const corrP5 = percentile(correctedValues, 0.05);
    const corrP95 = percentile(correctedValues, 0.95);

    const rawSwing = rawP5 != null && rawP95 != null ? rawP95 - rawP5 : null;
    const correctedSwing = corrP5 != null && corrP95 != null ? corrP95 - corrP5 : null;
    const reductionPct =
      rawSwing != null && correctedSwing != null && rawSwing > 0
        ? Math.max(0, Math.min(100, (1 - correctedSwing / rawSwing) * 100))
        : null;

    const correctionSamples: number[] = [];
    const steps = 60;
    const span = fit.tempMax - fit.tempMin;
    for (let i = 0; i < steps; i += 1) {
      const t = fit.tempMin + (span * i) / (steps - 1);
      const corr = computeTemperatureDriftCorrection(t, fit);
      if (corr != null && Number.isFinite(corr)) correctionSamples.push(corr);
    }
    const correctionMin = correctionSamples.length ? Math.min(...correctionSamples) : null;
    const correctionMax = correctionSamples.length ? Math.max(...correctionSamples) : null;
    const correctionSpan =
      correctionMin != null && correctionMax != null ? correctionMax - correctionMin : null;

    return {
      rawSwing,
      correctedSwing,
      reductionPct,
      correctionSpan,
      timeSlopePerDay: fit.timeSlopePerDay,
      unit: rawSensor.unit,
    };
  }, [alignedPoints, fit, rawSensor]);

  const recommendedOutputName = useMemo(() => {
    if (!rawSensor) return "";
    return `${rawSensor.name} (temp compensated)`;
  }, [rawSensor]);

  const effectiveOutputName = outputName.trim() || recommendedOutputName;

  const effectiveOutputIntervalSeconds = useMemo(() => {
    const parsed = Number.parseInt(outputIntervalSecondsRaw.trim(), 10);
    if (Number.isFinite(parsed) && parsed > 0) return parsed;
    return rawSensor?.interval_seconds ?? effectiveIntervalSeconds;
  }, [effectiveIntervalSeconds, outputIntervalSecondsRaw, rawSensor?.interval_seconds]);

  const effectiveOutputRollingAvgSeconds = useMemo(() => {
    const parsed = Number.parseInt(outputRollingAvgSecondsRaw.trim(), 10);
    if (Number.isFinite(parsed) && parsed >= 0) return parsed;
    return rawSensor?.rolling_avg_seconds ?? 0;
  }, [outputRollingAvgSecondsRaw, rawSensor?.rolling_avg_seconds]);

  const canCreate =
    canEdit &&
    !creating &&
    Boolean(rawSensor && tempSensor && fit && expressionPreview && effectiveOutputName.trim());

  const createCompensatedSensor = async () => {
    if (!rawSensor || !tempSensor || !fit || !expressionPreview) return;
    if (!canEdit) return;
    setCreating(true);
    setMessage(null);

    try {
      const lagSeconds = Math.max(0, Math.round(effectiveTemperatureLagSeconds));
      const payload = {
        node_id: rawSensor.node_id,
        name: effectiveOutputName.trim(),
        type: rawSensor.type,
        unit: rawSensor.unit,
        interval_seconds: effectiveOutputIntervalSeconds,
        rolling_avg_seconds: effectiveOutputRollingAvgSeconds,
        config: {
          source: "derived",
          derived: {
            template: "temp_compensation_v1",
            expression: expressionPreview.expression,
            inputs: [
              { sensor_id: rawSensor.sensor_id, var: "raw" },
              { sensor_id: tempSensor.sensor_id, var: "t", lag_seconds: lagSeconds },
            ],
            params: {
              degree: fit.degree,
              center_temp: fit.centerTemp,
              coefficients: fit.coefficients,
              clamp_abs: clampAbs,
              temperature_lag_seconds: lagSeconds,
              include_time_slope: includeTimeSlope,
              time_slope_per_day: fit.timeSlopePerDay,
              center_time_ms: fit.centerTimeMs,
              range_hours: effectiveRangeHours,
              interval_seconds: effectiveIntervalSeconds,
              raw_sensor_id: rawSensor.sensor_id,
              temperature_sensor_id: tempSensor.sensor_id,
              ...(rangeMode === "custom" && customRange.error == null && customRange.startIso && customRange.endIso
                ? { window_start: customRange.startIso, window_end: customRange.endIso }
                : {}),
            },
          },
        },
      };

      const created = (await postJson<unknown>("/api/sensors", payload)) as { sensor_id?: string };
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
      ]);
      const createdId = typeof created?.sensor_id === "string" ? created.sensor_id : null;
      const linkHref = createdId ? `/sensors/detail?id=${encodeURIComponent(createdId)}` : undefined;
      setMessage({
        type: "success",
        text: "Created compensated sensor (derived).",
        linkHref,
      });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Failed to create compensated sensor.",
      });
    } finally {
      setCreating(false);
    }
  };

  if (nodesQuery.isLoading || sensorsQuery.isLoading) {
    return <LoadingState label="Loading sensors..." />;
  }
  if (nodesQuery.error || sensorsQuery.error) {
    return (
      <ErrorState
        message={
          (nodesQuery.error instanceof Error && nodesQuery.error.message) ||
          (sensorsQuery.error instanceof Error && sensorsQuery.error.message) ||
          "Failed to load sensors."
        }
      />
    );
  }

  return (
    <div className="space-y-5">
      <AnalyticsHeaderCard
        tab="compensation"
        actions={
          <Badge tone="accent" className="hidden sm:inline-flex">
            Assisted
          </Badge>
        }
      >
        {!canEdit ? (
          <InlineBanner tone="warning">
            You are in read-only mode. You can explore the fit and previews, but creating a compensated sensor requires{" "}
            <code>config.write</code>.
          </InlineBanner>
        ) : null}
      </AnalyticsHeaderCard>

      {message ? (
        <InlineBanner tone={message.type === "success" ? "success" : "danger"}>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>{message.text}</div>
            {message.linkHref ? (
              <Link
                href={message.linkHref}
                className="inline-flex items-center justify-center rounded-lg border border-border bg-white px-3 py-2 text-sm font-semibold text-foreground shadow-xs hover:bg-muted focus:outline-hidden focus:bg-card-inset"
              >
                View sensor
              </Link>
            ) : null}
          </div>
        </InlineBanner>
      ) : null}

      <CollapsibleCard
        title="1) Select sensors"
        description="Pick the drifting sensor to compensate and a reference temperature sensor."
        defaultOpen
      >
        <div className="space-y-4">
          <InlineBanner tone="info">
            Drift compensation is best for sensors that should be stable but appear to shift as temperature changes. If
            the real-world signal truly depends on temperature, this tool may remove real behavior.
          </InlineBanner>

          <div className="grid gap-4 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-2">
            <div className="space-y-2">
              <p className="text-sm font-semibold text-foreground">Sensor to compensate</p>
              <Input
                value={sensorSearch}
                onChange={(e) => setSensorSearch(e.target.value)}
                placeholder="Search sensors (name, type, unit, node...)"
              />
              <Select value={rawSensorId} onChange={(e) => setRawSensorId(e.target.value)}>
                <option value="">Select a sensor…</option>
                {sensorGroups.map(([nodeId, list]) => (
                  <optgroup key={nodeId} label={nodeNameById.get(nodeId) ?? nodeId}>
                    {list.map((sensor) => (
                      <option key={sensor.sensor_id} value={sensor.sensor_id}>
                        {sensor.name} · {sensor.type} · {sensor.unit}
                      </option>
                    ))}
                  </optgroup>
                ))}
              </Select>
              {rawSensor ? (
                <p className="text-xs text-muted-foreground">
                  Selected: <span className="font-semibold text-foreground">{rawSensor.name}</span> (
                  {nodeNameById.get(rawSensor.node_id) ?? rawSensor.node_id})
                </p>
              ) : null}
            </div>

            <div className="space-y-2">
              <p className="text-sm font-semibold text-foreground">Temperature reference sensor</p>
              <Select value={tempSensorId} onChange={(e) => setTempSensorId(e.target.value)}>
                <option value="">Select a temperature sensor…</option>
                {temperatureGroups.length ? (
                  temperatureGroups.map(([nodeId, list]) => (
                    <optgroup key={nodeId} label={nodeNameById.get(nodeId) ?? nodeId}>
                      {list.map((sensor) => (
                        <option key={sensor.sensor_id} value={sensor.sensor_id}>
                          {sensor.name} · {sensor.type} · {sensor.unit}
                        </option>
                      ))}
                    </optgroup>
                  ))
                ) : (
                  <option value="" disabled>
                    No temperature-like sensors matched your search.
                  </option>
                )}
              </Select>
              {tempSensor ? (
                <p className="text-xs text-muted-foreground">
                  Selected: <span className="font-semibold text-foreground">{tempSensor.name}</span> (
                  {nodeNameById.get(tempSensor.node_id) ?? tempSensor.node_id})
                </p>
              ) : null}
            </div>
          </div>

          {rawSensor && tempSensor ? (
            <Card className="p-4">
              <div className="grid gap-3 md:grid-cols-3">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Live raw</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {rawSensor.latest_value != null
                      ? `${formatNumber(rawSensor.latest_value, { maximumFractionDigits: 4 })} ${rawSensor.unit}`
                      : "\u2014"}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Live temperature</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {tempSensor.latest_value != null
                      ? `${formatNumber(tempSensor.latest_value, { maximumFractionDigits: 2 })} ${tempSensor.unit}`
                      : "\u2014"}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Live adjustment</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {livePreview.correction != null
                      ? `${formatSigned(livePreview.correction, 4)} ${rawSensor.unit}`
                      : "\u2014"}
                  </p>
                </div>
              </div>
            </Card>
          ) : null}
        </div>
      </CollapsibleCard>

      <CollapsibleCard
        title="2) Training data and model"
        description="Choose a window and fit a drift model. Start with Linear."
        defaultOpen
      >
        <div className="space-y-4">
          <div className="grid gap-4 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-3">
            <div className="space-y-2">
              <div className="flex items-start justify-between gap-2">
                <AnalyticsRangeSelect
                  value={rangeHours}
                  onChange={(next) => {
                    setRangeHours(next);
                    setRangeMode("preset");
                  }}
                />
                <Button
                  type="button"
                  size="xs"
                  variant={rangeMode === "custom" ? "primary" : "ghost"}
                  onClick={() => {
                    if (rangeMode === "custom") {
                      setRangeMode("preset");
                      return;
                    }
                    const end = new Date();
                    const start = new Date(end.getTime() - rangeHours * 60 * 60 * 1000);
                    setRangeStartLocal(formatDateTimeInputValue(start, timeZone));
                    setRangeEndLocal(formatDateTimeInputValue(end, timeZone));
                    setRangeMode("custom");
                  }}
                >
                  Custom
                </Button>
              </div>

              {rangeMode === "custom" ? (
                <div className="space-y-2">
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Start / end (site time)
                  </p>
                  <div className="grid gap-2 sm:grid-cols-2">
                    <Input
                      type="datetime-local"
                      value={rangeStartLocal}
                      onChange={(event) => setRangeStartLocal(event.target.value)}
                      aria-label="Training window start"
                    />
                    <Input
                      type="datetime-local"
                      value={rangeEndLocal}
                      onChange={(event) => setRangeEndLocal(event.target.value)}
                      aria-label="Training window end"
                    />
                  </div>
                  {customRange.error ? (
                    <p className="text-xs text-rose-600">{customRange.error}</p>
                  ) : (
                    <p className="text-xs text-muted-foreground">
                      Window length:{" "}
                      <span className="font-semibold text-foreground">
                        {formatNumber(effectiveRangeHours, { maximumFractionDigits: 2 })}h
                      </span>
                      . Choose a window with real temperature swing.
                    </p>
                  )}
                </div>
              ) : (
                <p className="text-xs text-muted-foreground">Tip: choose a window with real temperature swing.</p>
              )}
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2">
                <p className="text-sm font-semibold text-foreground">Interval</p>
                <Button
                  type="button"
                  size="xs"
                  variant={intervalMode === "auto" ? "primary" : "ghost"}
                  onClick={() => setIntervalMode(intervalMode === "auto" ? "custom" : "auto")}
                >
                  {intervalMode === "auto" ? "Auto" : "Custom"}
                </Button>
              </div>

              {intervalMode === "auto" ? (
                <Card className="p-3">
                  <p className="text-sm font-semibold text-foreground">
                    {INTERVAL_OPTIONS_SECONDS.find((opt) => opt.value === effectiveIntervalSeconds)?.label ??
                      `${effectiveIntervalSeconds}s`}
                  </p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    Auto interval adjusts to the chosen range for performance and clarity.
                  </p>
                </Card>
              ) : (
                <Select value={String(intervalSeconds)} onChange={(e) => setIntervalSeconds(Number.parseInt(e.target.value, 10))}>
                  {INTERVAL_OPTIONS_SECONDS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </Select>
              )}
            </div>

            <div className="space-y-2">
              <p className="text-sm font-semibold text-foreground">Model</p>
              <Select value={String(degree)} onChange={(e) => setDegree(Number.parseInt(e.target.value, 10) as DriftModelDegree)}>
                {DEGREE_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </Select>
              <p className="text-xs text-muted-foreground">
                {DEGREE_OPTIONS.find((opt) => opt.value === degree)?.hint ?? ""}
              </p>
            </div>
          </div>

          <div className="grid gap-4 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-3">
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2">
                <p className="text-sm font-semibold text-foreground">Base temperature</p>
                <Button
                  type="button"
                  size="xs"
                  variant={centerMode === "auto" ? "primary" : "ghost"}
                  onClick={() => setCenterMode(centerMode === "auto" ? "custom" : "auto")}
                >
                  {centerMode === "auto" ? "Auto" : "Custom"}
                </Button>
              </div>
              {centerMode === "auto" ? (
                <p className="text-xs text-muted-foreground">Auto uses the mean temperature in the training window.</p>
              ) : (
                <Input
                  value={customCenterTemp}
                  onChange={(e) => setCustomCenterTemp(e.target.value)}
                  placeholder="e.g. 20.0"
                  inputMode="decimal"
                />
              )}
            </div>

            <div className="space-y-2">
              <p className="text-sm font-semibold text-foreground">Clamp adjustment (optional)</p>
              <Input
                value={clampAbsRaw}
                onChange={(e) => setClampAbsRaw(e.target.value)}
                placeholder="e.g. 5 (max abs adjustment)"
                inputMode="decimal"
              />
              <p className="text-xs text-muted-foreground">
                Safety cap to prevent extreme corrections when temperature is out-of-range.
              </p>
            </div>

            <div className="space-y-2">
              <p className="text-sm font-semibold text-foreground">Outlier trim (optional)</p>
              <Input
                value={outlierTrimPctRaw}
                onChange={(e) => setOutlierTrimPctRaw(e.target.value)}
                placeholder="0"
                inputMode="decimal"
              />
              <p className="text-xs text-muted-foreground">Trims the largest residuals (0–10%). Helpful with spikes.</p>
            </div>
          </div>

          <Card className="p-4">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="space-y-1">
                <p className="text-sm font-semibold text-foreground">Detrend slow changes over time</p>
                <p className="text-xs text-muted-foreground">
                  Useful for sensors like reservoir depth that may change gradually over days. Detrending helps the temperature
                  fit avoid being diluted by slow real movement.
                </p>
              </div>
              <Button
                type="button"
                size="xs"
                variant={includeTimeSlope ? "primary" : "ghost"}
                onClick={() => {
                  setIncludeTimeSlopeTouched(true);
                  setIncludeTimeSlope(!includeTimeSlope);
                }}
              >
                {includeTimeSlope ? "On" : "Off"}
              </Button>
            </div>
          </Card>

          <Card className="p-4">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="space-y-1">
                <p className="text-sm font-semibold text-foreground">Temperature lag (thermal delay)</p>
                <p className="text-xs text-muted-foreground">
                  Many sensors drift with a delay relative to air temperature (e.g., the transducer or enclosure warms up later).
                  Adding a lag can materially improve drift compensation.
                </p>
                {tempLagMode === "auto" && lagSuggestion?.best?.reductionPct != null ? (
                  <p className="text-xs text-muted-foreground">
                    Auto picked{" "}
                    <span className="font-semibold text-foreground">
                      {formatNumber((lagSuggestion.lagSeconds ?? 0) / 60, { maximumFractionDigits: 0 })}
                    </span>{" "}
                    min (P95–P5 reduction{" "}
                    <span className="font-semibold text-foreground">
                      {formatNumber(lagSuggestion.best.reductionPct, { maximumFractionDigits: 0 })}
                    </span>
                    %)
                    {lagSuggestion.baseline?.reductionPct != null ? (
                      <>
                        {" "}
                        vs 0 min{" "}
                        <span className="font-semibold text-foreground">
                          {formatNumber(lagSuggestion.baseline.reductionPct, { maximumFractionDigits: 0 })}
                        </span>
                        %
                      </>
                    ) : null}
                    .
                  </p>
                ) : null}
              </div>
              <Button
                type="button"
                size="xs"
                variant={tempLagMode === "auto" ? "primary" : "ghost"}
                onClick={() => setTempLagMode(tempLagMode === "auto" ? "custom" : "auto")}
              >
                {tempLagMode === "auto" ? "Auto" : "Custom"}
              </Button>
            </div>
            {tempLagMode === "custom" ? (
              <div className="mt-3 space-y-2">
                <Input
                  value={tempLagMinutesRaw}
                  onChange={(e) => setTempLagMinutesRaw(e.target.value)}
                  placeholder="e.g. 165 (minutes)"
                  inputMode="decimal"
                />
                <p className="text-xs text-muted-foreground">
                  Positive values mean temperature is read from the past (raw[t] aligns to temp[t − lag]).
                </p>
              </div>
            ) : null}
          </Card>

          {metricsQuery.isLoading && rawSensorId && tempSensorId ? (
            <LoadingState label="Loading training data..." />
          ) : null}
          {metricsQuery.error ? (
            <ErrorState message={metricsQuery.error instanceof Error ? metricsQuery.error.message : "Failed to load metrics."} />
          ) : null}

          {fitSummary ? (
            <Card className="p-4">
              <div className="grid gap-3 md:grid-cols-4">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Fit</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">{fitSummary.degreeLabel}</p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">R²</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {fitSummary.r2 != null ? formatNumber(fitSummary.r2, { maximumFractionDigits: 3 }) : "\u2014"}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Samples</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">{fitSummary.samples}</p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Temp range</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">{fitSummary.tempRange}</p>
                </div>
              </div>
              <p className="mt-3 text-xs text-muted-foreground">
                Base temperature: <span className="font-semibold text-foreground">{formatNumber(fitSummary.centerTemp, { maximumFractionDigits: 2 })}</span>
                {tempSensor ? ` ${tempSensor.unit}` : ""}
              </p>
            </Card>
          ) : rawSensorId && tempSensorId && !metricsQuery.isLoading ? (
            <InlineBanner tone="warning">
              Not enough overlapping samples to fit a model yet. Try a longer range, smaller interval, or confirm both sensors have data.
            </InlineBanner>
          ) : null}

          {fitDiagnostics ? (
            <Card className="p-4">
              <div className="grid gap-3 md:grid-cols-4">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Raw swing (P95–P5)</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {fitDiagnostics.rawSwing != null
                      ? `${formatNumber(fitDiagnostics.rawSwing, { maximumFractionDigits: 4 })} ${fitDiagnostics.unit}`
                      : "\u2014"}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Comp swing (P95–P5)</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {fitDiagnostics.correctedSwing != null
                      ? `${formatNumber(fitDiagnostics.correctedSwing, { maximumFractionDigits: 4 })} ${fitDiagnostics.unit}`
                      : "\u2014"}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Range reduction</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {fitDiagnostics.reductionPct != null
                      ? `${formatNumber(fitDiagnostics.reductionPct, { maximumFractionDigits: 1 })}%`
                      : "\u2014"}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Correction span</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {fitDiagnostics.correctionSpan != null
                      ? `${formatNumber(fitDiagnostics.correctionSpan, { maximumFractionDigits: 4 })} ${fitDiagnostics.unit}`
                      : "\u2014"}
                  </p>
                </div>
              </div>
              {includeTimeSlope && fitDiagnostics.timeSlopePerDay != null ? (
                <p className="mt-3 text-xs text-muted-foreground">
                  Time trend (fit only):{" "}
                  <span className="font-semibold text-foreground">
                    {formatSigned(fitDiagnostics.timeSlopePerDay, 4)} {fitDiagnostics.unit}/day
                  </span>
                </p>
              ) : null}
            </Card>
          ) : null}

          {expressionPreview ? (
            <CollapsibleCard
              density="sm"
              title="Advanced: expression preview"
              description="This is the exact derived-sensor expression that will run in the controller."
              defaultOpen={false}
            >
              <div className="space-y-2">
                <p className="text-xs font-semibold text-muted-foreground">Expression</p>
                <pre className="overflow-auto rounded-lg border border-border bg-card-inset p-3 text-xs text-foreground">
                  {expressionPreview.expression}
                </pre>
                <p className="text-xs text-muted-foreground">
                  Variables: <code>raw</code> = target sensor value, <code>t</code> = temperature reference value.
                </p>
              </div>
            </CollapsibleCard>
          ) : null}
        </div>
      </CollapsibleCard>

      <CollapsibleCard
        title="3) Preview (visual)"
        description="Inspect the correction before applying it. Raw vs compensated should be less temperature-correlated."
        defaultOpen={Boolean(fit)}
      >
        {!rawSensor || !tempSensor ? (
          <InlineBanner tone="info">Select sensors first to unlock previews.</InlineBanner>
        ) : !fit ? (
          <InlineBanner tone="info">Fit a model (Step 2) to see charts.</InlineBanner>
        ) : (
          <div className="space-y-6">
            <div className="grid gap-4 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-2">
              <Card className="p-4">
                <p className="text-sm font-semibold text-foreground">Raw vs compensated over time</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  Includes temperature on a secondary axis for context.
                </p>
              </Card>
              <Card className="p-4">
                <p className="text-sm font-semibold text-foreground">Adjustment vs temperature</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  This is the amount subtracted from the raw sensor at each temperature.
                </p>
              </Card>
            </div>

            <TemperatureCompensationTimeSeriesChart
              points={alignedPoints}
              fit={fit}
              rawLabel="Raw"
              compensatedLabel="Compensated"
              rawUnit={rawSensor.unit}
              temperatureUnit={tempSensor.unit}
            />

            <div className="grid gap-6 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-2">
              <div className="space-y-2">
                <p className="text-sm font-semibold text-foreground">Raw value vs temperature</p>
                <p className="text-xs text-muted-foreground">
                  Scatter shows your historical samples; the line is the fitted drift model.
                </p>
                <TemperatureCompensationScatterChart
                  points={alignedPoints}
                  fit={fit}
                  rawLabel={rawSensor.name}
                  rawUnit={rawSensor.unit}
                  temperatureLabel={tempSensor.name}
                  temperatureUnit={tempSensor.unit}
                />
              </div>

              <div className="space-y-2">
                <p className="text-sm font-semibold text-foreground">Adjustment curve</p>
                <p className="text-xs text-muted-foreground">
                  Use clamping if the curve becomes extreme outside the observed range.
                </p>
                <TemperatureCompensationCorrectionChart fit={fit} temperatureUnit={tempSensor.unit} rawUnit={rawSensor.unit} />
              </div>
            </div>
          </div>
        )}
      </CollapsibleCard>

      <CollapsibleCard
        title="4) Create compensated sensor"
        description="Applies the drift model by creating a new derived sensor. The original sensor remains unchanged."
        defaultOpen={Boolean(fit)}
      >
        {!rawSensor || !tempSensor ? (
          <InlineBanner tone="info">Select sensors first.</InlineBanner>
        ) : !fit || !expressionPreview ? (
          <InlineBanner tone="info">Fit a model first.</InlineBanner>
        ) : (
          <div className="space-y-4">
            <div className="grid gap-4 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-2">
              <div className="space-y-2">
                <p className="text-sm font-semibold text-foreground">Name</p>
                <Input
                  value={outputName}
                  onChange={(e) => setOutputName(e.target.value)}
                  placeholder={recommendedOutputName}
                />
                <p className="text-xs text-muted-foreground">
                  A new sensor will be created under <span className="font-semibold text-foreground">{nodeNameById.get(rawSensor.node_id) ?? rawSensor.node_id}</span>.
                </p>
              </div>

              <div className="space-y-2">
                <p className="text-sm font-semibold text-foreground">Sampling</p>
                <div className="grid gap-3 sm:grid-cols-2">
                  <div className="space-y-1">
                    <p className="text-xs font-semibold text-muted-foreground">Interval (seconds)</p>
                    <Input
                      value={outputIntervalSecondsRaw}
                      onChange={(e) => setOutputIntervalSecondsRaw(e.target.value)}
                      placeholder={String(rawSensor.interval_seconds)}
                      inputMode="numeric"
                    />
                  </div>
                  <div className="space-y-1">
                    <p className="text-xs font-semibold text-muted-foreground">Rolling avg (seconds)</p>
                    <Input
                      value={outputRollingAvgSecondsRaw}
                      onChange={(e) => setOutputRollingAvgSecondsRaw(e.target.value)}
                      placeholder={String(rawSensor.rolling_avg_seconds)}
                      inputMode="numeric"
                    />
                  </div>
                </div>
              </div>
            </div>

            <Card className="p-4">
              <div className="grid gap-3 md:grid-cols-3">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Current adjustment</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {livePreview.correction != null ? `${formatSigned(livePreview.correction)} ${rawSensor.unit}` : "\u2014"}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Current compensated</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">
                    {livePreview.corrected != null
                      ? `${formatNumber(livePreview.corrected, { maximumFractionDigits: 4 })} ${rawSensor.unit}`
                      : "\u2014"}
                  </p>
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Output</p>
                  <p className="mt-1 text-sm font-semibold text-foreground">{effectiveOutputName || "\u2014"}</p>
                </div>
              </div>
            </Card>

            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="text-xs text-muted-foreground">
                Creates a derived sensor (safe): you can always delete or ignore it if the correction looks wrong.
              </div>
              <Button type="button" variant="primary" disabled={!canCreate} loading={creating} onClick={createCompensatedSensor}>
                Create compensated sensor
              </Button>
            </div>
          </div>
        )}
      </CollapsibleCard>
    </div>
  );
}
