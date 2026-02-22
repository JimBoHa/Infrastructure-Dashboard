/**
 * useAnalysisJob - Encapsulates job lifecycle management for analysis jobs
 *
 * Handles:
 * - Job creation with deduplication via job_key
 * - Polling for status updates
 * - Cancellation
 * - Result fetching
 * - Progress tracking
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  cancelAnalysisJob,
  createAnalysisJob,
  fetchAnalysisJob,
  fetchAnalysisJobEvents,
  fetchAnalysisJobResult,
} from "@/lib/api";
import type {
  AnalysisJobCreateRequest,
  AnalysisJobPublic,
  AnalysisJobProgress,
  AnalysisJobStatus,
} from "@/types/analysis";

export type UseAnalysisJobOptions = {
  /** Callback when job completes successfully */
  onComplete?: (result: unknown) => void;
  /** Callback when job fails */
  onError?: (error: string) => void;
  /** Callback when job is cancelled */
  onCancel?: () => void;
};

export type UseAnalysisJobState = {
  jobId: string | null;
  status: AnalysisJobStatus | null;
  progress: AnalysisJobProgress | null;
  progressMessage: string | null;
  error: string | null;
  requestedAt: Date | null;
  completedAt: Date | null;
  isSubmitting: boolean;
  isRunning: boolean;
  isCompleted: boolean;
  isFailed: boolean;
  isCanceled: boolean;
  canCancel: boolean;
};

export type UseAnalysisJobActions = {
  run: <TResult = unknown>(
    jobType: string,
    params: unknown,
    jobKey?: string,
  ) => Promise<TResult | null>;
  cancel: () => Promise<void>;
  reset: () => void;
};

export type UseAnalysisJobResult<TResult = unknown> = UseAnalysisJobState &
  UseAnalysisJobActions & {
    result: TResult | null;
    job: AnalysisJobPublic | null;
  };

const POLL_INTERVAL_MS = 2000;
const EVENTS_POLL_INTERVAL_MS = 2000;

/**
 * Hook for managing analysis job lifecycle
 *
 * @example
 * ```tsx
 * const { run, cancel, status, result, progress } = useAnalysisJob<MyResultType>();
 *
 * const handleRun = async () => {
 *   const result = await run("related_sensors_v1", params, jobKey);
 *   if (result) {
 *     // Handle result
 *   }
 * };
 * ```
 */
export function useAnalysisJob<TResult = unknown>(
  options: UseAnalysisJobOptions = {},
): UseAnalysisJobResult<TResult> {
  const { onComplete, onError, onCancel } = options;
  const queryClient = useQueryClient();

  // Core state
  const [jobId, setJobId] = useState<string | null>(null);
  const [requestedAt, setRequestedAt] = useState<Date | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [runError, setRunError] = useState<string | null>(null);

  // Track whether we've already fired callbacks for the current job
  const firedCallbacksRef = useRef<Set<string>>(new Set());

  // Job status polling
  const jobStatusQuery = useQuery({
    queryKey: ["analysis", "jobs", jobId ?? "none"],
    queryFn: () => fetchAnalysisJob(jobId as string),
    enabled: Boolean(jobId),
    refetchInterval: (query) => {
      const status = query.state.data?.job.status;
      if (!status) return POLL_INTERVAL_MS;
      return status === "completed" || status === "failed" || status === "canceled"
        ? false
        : POLL_INTERVAL_MS;
    },
  });

  const job = jobStatusQuery.data?.job ?? null;
  const status = job?.status ?? null;

  // Job events polling (for progress messages)
  const jobEventsQuery = useQuery({
    queryKey: ["analysis", "jobs", jobId ?? "none", "events"],
    queryFn: () => fetchAnalysisJobEvents(jobId as string, { limit: 20 }),
    enabled: Boolean(jobId) && (status === "running" || status === "pending"),
    refetchInterval:
      status === "running" || status === "pending" ? EVENTS_POLL_INTERVAL_MS : false,
    staleTime: 10_000,
  });

  // Extract latest event message
  const progressMessage = useMemo(() => {
    const events = jobEventsQuery.data?.events ?? [];
    if (!events.length) return null;
    const latest = events.reduce(
      (acc, event) => (event.id > acc.id ? event : acc),
      events[0]!,
    );
    const payload = latest.payload;
    if (payload && typeof payload === "object" && "message" in payload) {
      const message = (payload as { message?: unknown }).message;
      if (typeof message === "string" && message.trim()) return message;
    }
    return latest.kind?.replace(/_/g, " ") ?? null;
  }, [jobEventsQuery.data?.events]);

  // Job result fetching (only when completed)
  const jobResultQuery = useQuery({
    queryKey: ["analysis", "jobs", jobId ?? "none", "result"],
    queryFn: () => fetchAnalysisJobResult<TResult>(jobId as string),
    enabled: status === "completed",
    staleTime: 60_000,
  });

  const result = jobResultQuery.data?.result ?? null;

  // Derived state
  const isRunning = status === "running" || status === "pending";
  const isCompleted = status === "completed";
  const isFailed = status === "failed";
  const isCanceled = status === "canceled";
  const canCancel = Boolean(jobId) && isRunning;

  const progress = job?.progress ?? null;
  const completedAt = job?.completed_at ? new Date(job.completed_at) : null;

  // Combined error (from job or run attempt)
  const error = useMemo(() => {
    if (runError) return runError;
    if (job?.error?.message) return job.error.message;
    return null;
  }, [runError, job?.error?.message]);

  // Fire callbacks on state changes
  useEffect(() => {
    if (!jobId) return;

    const callbackKey = `${jobId}-${status}`;
    if (firedCallbacksRef.current.has(callbackKey)) return;

    if (status === "completed" && result) {
      firedCallbacksRef.current.add(callbackKey);
      onComplete?.(result);
    } else if (status === "failed") {
      firedCallbacksRef.current.add(callbackKey);
      onError?.(job?.error?.message ?? "Job failed");
    } else if (status === "canceled") {
      firedCallbacksRef.current.add(callbackKey);
      onCancel?.();
    }
  }, [jobId, status, result, job?.error?.message, onComplete, onError, onCancel]);

  // Run a new job
  const run = useCallback(
    async <T = TResult>(
      jobType: string,
      params: unknown,
      jobKey?: string,
    ): Promise<T | null> => {
      setRunError(null);
      setIsSubmitting(true);

      try {
        const request: AnalysisJobCreateRequest = {
          job_type: jobType,
          params,
          job_key: jobKey ?? null,
          dedupe: Boolean(jobKey),
        };

        const response = await createAnalysisJob(request);
        const newJobId = response.job.id;

        setJobId(newJobId);
        setRequestedAt(new Date());

        // Prime the cache with the response
        queryClient.setQueryData(["analysis", "jobs", newJobId], response);
        // Clear any stale result
        queryClient.removeQueries({ queryKey: ["analysis", "jobs", newJobId, "result"] });

        // Clear fired callbacks for new job
        firedCallbacksRef.current = new Set();

        // If the job was already completed (dedupe hit), return result immediately
        if (response.job.status === "completed") {
          const resultResponse = await fetchAnalysisJobResult<T>(newJobId);
          return resultResponse.result;
        }

        // For pending/running jobs, the result will come through the query
        return null;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to start analysis job";
        setRunError(message);
        onError?.(message);
        return null;
      } finally {
        setIsSubmitting(false);
      }
    },
    [queryClient, onError],
  );

  // Cancel the current job
  const cancel = useCallback(async () => {
    if (!jobId) return;

    try {
      const response = await cancelAnalysisJob(jobId);
      queryClient.setQueryData(["analysis", "jobs", jobId], response);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to cancel job";
      setRunError(message);
    }
  }, [jobId, queryClient]);

  // Reset state
  const reset = useCallback(() => {
    setJobId(null);
    setRequestedAt(null);
    setIsSubmitting(false);
    setRunError(null);
    firedCallbacksRef.current = new Set();
  }, []);

  return {
    // State
    jobId,
    status,
    progress,
    progressMessage,
    error,
    requestedAt,
    completedAt,
    isSubmitting,
    isRunning,
    isCompleted,
    isFailed,
    isCanceled,
    canCancel,
    // Data
    result,
    job,
    // Actions
    run,
    cancel,
    reset,
  };
}

/**
 * Generate a stable job key from parameters
 *
 * @example
 * ```ts
 * const jobKey = generateJobKey({
 *   v: 1,
 *   strategy: "similarity",
 *   focus: focusSensorId,
 *   start: window.startIso,
 *   end: window.endIso,
 *   interval: intervalSeconds,
 *   params: strategyParams,
 * });
 * ```
 */
export function generateJobKey(params: Record<string, unknown>): string {
  // Sort keys for stability
  const sorted = Object.keys(params)
    .sort()
    .reduce(
      (acc, key) => {
        acc[key] = params[key];
        return acc;
      },
      {} as Record<string, unknown>,
    );
  return JSON.stringify(sorted);
}

export default useAnalysisJob;
