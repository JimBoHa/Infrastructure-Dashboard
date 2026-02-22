"use client";

import { useEffect, useMemo } from "react";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import type {
  CorrelationMatrixJobParamsV1,
  CorrelationMatrixResultV1,
} from "@/types/analysis";
import { useAnalysisJob } from "../hooks/useAnalysisJob";
import { generateCorrelationJobKey } from "../strategies/correlation";
import { CorrelationMatrix } from "./relationshipFinder";

type SelectedSensorsCorrelationMatrixCardProps = {
  selectedSensorIds: string[];
  labelMap: Map<string, string>;
  intervalSeconds: number;
  rangeHours: number;
  rangeSelect: string;
  customStartIso: string | null;
  customEndIso: string | null;
  customRangeValid: boolean;
};

type WindowRange = {
  startIso: string;
  endIso: string;
};

export default function SelectedSensorsCorrelationMatrixCard({
  selectedSensorIds,
  labelMap,
  intervalSeconds,
  rangeHours,
  rangeSelect,
  customStartIso,
  customEndIso,
  customRangeValid,
}: SelectedSensorsCorrelationMatrixCardProps) {
  const matrixJob = useAnalysisJob<CorrelationMatrixResultV1>();
  const runMatrixJob = matrixJob.run;

  const matrixSensorIds = useMemo(() => {
    const deduped: string[] = [];
    for (const id of selectedSensorIds) {
      if (!id || deduped.includes(id)) continue;
      deduped.push(id);
    }
    return deduped;
  }, [selectedSensorIds]);

  const canComputeWindow = useMemo(() => {
    if (rangeSelect === "custom") {
      return Boolean(customStartIso && customEndIso && customRangeValid);
    }
    return Number.isFinite(rangeHours) && rangeHours > 0;
  }, [customEndIso, customRangeValid, customStartIso, rangeHours, rangeSelect]);

  const window = useMemo((): WindowRange | null => {
    if (!canComputeWindow) return null;
    if (rangeSelect === "custom" && customStartIso && customEndIso) {
      return { startIso: customStartIso, endIso: customEndIso };
    }

    const end = new Date();
    end.setMilliseconds(0);
    const start = new Date(end.getTime() - rangeHours * 60 * 60 * 1000);
    return { startIso: start.toISOString(), endIso: end.toISOString() };
  }, [canComputeWindow, customEndIso, customStartIso, rangeHours, rangeSelect]);

  useEffect(() => {
    if (!window) return;
    if (matrixSensorIds.length < 2) return;

    const params: CorrelationMatrixJobParamsV1 = {
      sensor_ids: matrixSensorIds,
      start: window.startIso,
      end: window.endIso,
      interval_seconds: intervalSeconds,
      method: "pearson",
      min_overlap: 10,
      min_significant_n: 10,
      significance_alpha: 0.05,
      min_abs_r: 0.2,
      bucket_aggregation_mode: "auto",
      max_sensors: matrixSensorIds.length,
    };

    void runMatrixJob(
      "correlation_matrix_v1",
      params,
      generateCorrelationJobKey(params),
    );
  }, [intervalSeconds, matrixSensorIds, runMatrixJob, window]);

  return (
    <CollapsibleCard
      title="Selected Sensors Correlation Matrix"
      description="Pairwise correlation matrix for the sensors currently selected in the chart."
      className="mt-6"
      defaultOpen={true}
      data-testid="selected-sensors-correlation-matrix"
    >
      <div className="space-y-3">
        {matrixSensorIds.length < 2 ? (
          <Card className="rounded-lg gap-0 border-dashed px-4 py-8 text-center">
            <p className="text-sm text-muted-foreground">
              Select at least 2 sensors in the Sensor picker to render this matrix.
            </p>
          </Card>
        ) : !window ? (
          <Card className="rounded-lg gap-0 border-dashed px-4 py-8 text-center">
            <p className="text-sm text-muted-foreground">
              Choose a valid time window to compute correlations.
            </p>
          </Card>
        ) : matrixJob.isRunning || matrixJob.isSubmitting ? (
          <LoadingState label="Computing selected-sensor correlations…" />
        ) : matrixJob.isFailed ? (
          <ErrorState
            message={matrixJob.error ?? "Selected-sensor correlation matrix failed."}
          />
        ) : matrixJob.result ? (
          <CorrelationMatrix
            result={matrixJob.result}
            labelMap={labelMap}
            title="Matrix"
            description={
              <>
                Visual scan for currently selected sensors. Blue = negative correlation, Red =
                positive.
              </>
            }
          />
        ) : (
          <Card className="rounded-lg gap-0 border-dashed px-4 py-8 text-center text-sm text-muted-foreground">
            Correlation matrix is not available yet.
          </Card>
        )}
      </div>
    </CollapsibleCard>
  );
}

