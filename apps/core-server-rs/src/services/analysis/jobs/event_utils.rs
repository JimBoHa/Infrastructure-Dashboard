use crate::services::analysis::parquet_duckdb::MetricsBucketRow;
use crate::services::analysis::signal_semantics::{self, DeltaMode};
use crate::services::analysis::tsse::robust;
use crate::services::analysis::tsse::types::{
    AdaptiveThresholdConfigV1, EventDetectorModeV1, EventDirectionV1, EventPolarityV1,
    EventSuppressionModeV1, EventThresholdModeV1,
};
use chrono::{DateTime, Timelike, Utc};

#[derive(Debug, Clone)]
pub struct EventPoint {
    pub ts: DateTime<Utc>,
    pub ts_epoch: i64,
    pub z: f64,
    pub direction: EventDirectionV1,
    pub delta: f64,
    pub is_boundary: bool,
}

#[derive(Debug, Clone)]
pub struct DetectedEvents {
    pub events: Vec<EventPoint>,
    pub gap_skipped_deltas: u64,
    pub points_total: u64,
    pub peak_abs_z: Option<f64>,
    pub up_events: u64,
    pub down_events: u64,
    pub boundary_events: u64,
    pub z_threshold_used: f64,
}

#[derive(Debug, Clone)]
pub struct EventDetectOptionsV1 {
    pub interval_seconds: i64,
    pub z_threshold: f64,
    pub min_separation_buckets: i64,
    pub gap_max_buckets: i64,
    pub polarity: EventPolarityV1,
    pub max_events: usize,
    pub suppression_mode: EventSuppressionModeV1,
    pub threshold_mode: EventThresholdModeV1,
    pub adaptive: Option<AdaptiveThresholdConfigV1>,
    pub exclude_boundary_events: bool,
    pub detector_mode: EventDetectorModeV1,
    pub sparse_point_events_enabled: bool,
}

impl EventDetectOptionsV1 {
    pub fn fixed_default(
        interval_seconds: i64,
        z_threshold: f64,
        min_separation_buckets: i64,
        gap_max_buckets: i64,
        polarity: EventPolarityV1,
        max_events: usize,
    ) -> Self {
        Self {
            interval_seconds,
            z_threshold,
            min_separation_buckets,
            gap_max_buckets,
            polarity,
            max_events,
            suppression_mode: EventSuppressionModeV1::NmsWindow,
            threshold_mode: EventThresholdModeV1::FixedZ,
            adaptive: None,
            exclude_boundary_events: false,
            detector_mode: EventDetectorModeV1::BucketDeltas,
            sparse_point_events_enabled: false,
        }
    }
}

pub fn hour_of_day_mean_residual_rows(rows: &[MetricsBucketRow]) -> Vec<MetricsBucketRow> {
    if rows.is_empty() {
        return Vec::new();
    }

    let mut sums: [f64; 24] = [0.0; 24];
    let mut counts: [u64; 24] = [0; 24];
    let mut overall_sum = 0.0;
    let mut overall_count = 0_u64;

    for row in rows {
        if !row.value.is_finite() {
            continue;
        }
        let hour = row.bucket.hour() as usize;
        sums[hour] += row.value;
        counts[hour] = counts[hour].saturating_add(1);
        overall_sum += row.value;
        overall_count = overall_count.saturating_add(1);
    }

    let overall_mean = if overall_count > 0 {
        overall_sum / (overall_count as f64)
    } else {
        0.0
    };

    let mut means: [f64; 24] = [overall_mean; 24];
    for hour in 0..24 {
        if counts[hour] > 0 {
            means[hour] = sums[hour] / (counts[hour] as f64);
        }
    }

    rows.iter()
        .map(|row| {
            let mut out = row.clone();
            if out.value.is_finite() {
                let hour = out.bucket.hour() as usize;
                out.value = out.value - means[hour];
            }
            out
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct TimeOfDayEntropy {
    pub h_norm: f64,
    pub weight: f64,
}

pub fn time_of_day_entropy(events: &[EventPoint]) -> Option<TimeOfDayEntropy> {
    if events.is_empty() {
        return None;
    }

    let mut counts: [u64; 24] = [0; 24];
    for evt in events {
        let hour = evt.ts.hour() as usize;
        counts[hour] = counts[hour].saturating_add(1);
    }

    let total = events.len() as f64;
    if total <= 0.0 || !total.is_finite() {
        return None;
    }

    let mut h = 0.0;
    for count in counts {
        if count == 0 {
            continue;
        }
        let p = (count as f64) / total;
        if !p.is_finite() || p <= 0.0 {
            continue;
        }
        h -= p * p.ln();
    }

    let denom = (24.0_f64).ln();
    if !h.is_finite() || !denom.is_finite() || denom <= 0.0 {
        return None;
    }

    let h_norm = (h / denom).clamp(0.0, 1.0);
    let weight = h_norm.clamp(0.25, 1.0);

    Some(TimeOfDayEntropy { h_norm, weight })
}

pub fn detect_change_events(
    rows: &[MetricsBucketRow],
    interval_seconds: i64,
    z_threshold: f64,
    min_separation_buckets: i64,
    gap_max_buckets: i64,
    polarity: EventPolarityV1,
    max_events: usize,
) -> DetectedEvents {
    let options = EventDetectOptionsV1::fixed_default(
        interval_seconds,
        z_threshold,
        min_separation_buckets,
        gap_max_buckets,
        polarity,
        max_events,
    );
    detect_change_events_with_options(rows, &options)
}

pub fn detect_change_events_with_options(
    rows: &[MetricsBucketRow],
    options: &EventDetectOptionsV1,
) -> DetectedEvents {
    detect_change_events_with_options_and_delta_mode(rows, options, DeltaMode::Linear)
}

pub fn detect_change_events_with_options_and_delta_mode(
    rows: &[MetricsBucketRow],
    options: &EventDetectOptionsV1,
    delta_mode: DeltaMode,
) -> DetectedEvents {
    let mut gap_skipped_deltas: u64 = 0;

    let start_epoch = rows
        .first()
        .map(|r| r.bucket.timestamp())
        .unwrap_or_default();
    let end_epoch = rows
        .last()
        .map(|r| r.bucket.timestamp())
        .unwrap_or_default();

    let mut detected = detect_events_inner(
        rows,
        options,
        start_epoch,
        end_epoch,
        &mut gap_skipped_deltas,
        delta_mode,
    );

    if detected.events.is_empty()
        && options.sparse_point_events_enabled
        && !matches!(options.detector_mode, EventDetectorModeV1::BucketLevels)
    {
        let mut fallback = options.clone();
        fallback.detector_mode = EventDetectorModeV1::BucketLevels;
        detected = detect_events_inner(
            rows,
            &fallback,
            start_epoch,
            end_epoch,
            &mut gap_skipped_deltas,
            delta_mode,
        );
    }

    detected.gap_skipped_deltas = gap_skipped_deltas;
    detected
}

fn detect_events_inner(
    rows: &[MetricsBucketRow],
    options: &EventDetectOptionsV1,
    start_epoch: i64,
    end_epoch: i64,
    gap_skipped_deltas: &mut u64,
    delta_mode: DeltaMode,
) -> DetectedEvents {
    let mut base = DetectedEvents {
        events: Vec::new(),
        gap_skipped_deltas: 0,
        points_total: 0,
        peak_abs_z: None,
        up_events: 0,
        down_events: 0,
        boundary_events: 0,
        z_threshold_used: options.z_threshold.abs(),
    };

    if rows.len() < 3 {
        return base;
    }

    let interval_seconds = options.interval_seconds.max(1);
    let gap_max_buckets = options.gap_max_buckets.max(0);
    let gap_threshold_seconds = if gap_max_buckets > 0 {
        gap_max_buckets.saturating_mul(interval_seconds)
    } else {
        i64::MAX
    };
    let boundary_seconds = interval_seconds; // "within 1 bucket" of window edges

    let threshold_fixed = options.z_threshold.abs();
    if !threshold_fixed.is_finite() || threshold_fixed <= 0.0 {
        return base;
    }

    let adaptive_cfg = options
        .adaptive
        .as_ref()
        .filter(|_| matches!(options.threshold_mode, EventThresholdModeV1::AdaptiveRate));
    let adaptive_min_z = adaptive_cfg
        .and_then(|cfg| cfg.min_z)
        .map(|v| v.abs())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(threshold_fixed);

    let target_min_events = adaptive_cfg
        .and_then(|cfg| cfg.target_min_events)
        .and_then(|v| usize::try_from(v).ok())
        .filter(|v| *v > 0);
    let target_max_events = adaptive_cfg
        .and_then(|cfg| cfg.target_max_events)
        .and_then(|v| usize::try_from(v).ok())
        .filter(|v| *v > 0);

    let desired_events = match (target_min_events, target_max_events) {
        (Some(min), Some(max)) => Some(((min + max) / 2).clamp(min, max).max(1)),
        (Some(min), None) => Some(min.max(1)),
        (None, Some(max)) => Some(max.max(1)),
        (None, None) => None,
    };
    // Default to fixed z when adaptive config doesn't specify any target.
    let mut threshold_used = match options.threshold_mode {
        EventThresholdModeV1::FixedZ => threshold_fixed,
        EventThresholdModeV1::AdaptiveRate => adaptive_min_z,
    };

    let polarity = options.polarity;

    let mut points: Vec<(DateTime<Utc>, f64)> = Vec::new();
    match options.detector_mode {
        EventDetectorModeV1::BucketDeltas => {
            for idx in 1..rows.len() {
                let prev = &rows[idx - 1];
                let curr = &rows[idx];
                if !prev.value.is_finite() || !curr.value.is_finite() {
                    continue;
                }
                let dt_seconds = (curr.bucket - prev.bucket).num_seconds();
                if dt_seconds > gap_threshold_seconds {
                    *gap_skipped_deltas = (*gap_skipped_deltas).saturating_add(1);
                    continue;
                }
                let delta = signal_semantics::delta(prev.value, curr.value, delta_mode);
                if delta.is_finite() {
                    points.push((curr.bucket, delta));
                }
            }
        }
        EventDetectorModeV1::BucketSecondDeltas => {
            for idx in 2..rows.len() {
                let prev2 = &rows[idx - 2];
                let prev = &rows[idx - 1];
                let curr = &rows[idx];
                if !prev2.value.is_finite() || !prev.value.is_finite() || !curr.value.is_finite() {
                    continue;
                }
                let dt1 = (prev.bucket - prev2.bucket).num_seconds();
                let dt2 = (curr.bucket - prev.bucket).num_seconds();
                if dt1 > gap_threshold_seconds || dt2 > gap_threshold_seconds {
                    *gap_skipped_deltas = (*gap_skipped_deltas).saturating_add(1);
                    continue;
                }
                let delta_curr = signal_semantics::delta(prev.value, curr.value, delta_mode);
                let delta_prev = signal_semantics::delta(prev2.value, prev.value, delta_mode);
                let delta2 = delta_curr - delta_prev;
                if delta2.is_finite() {
                    points.push((curr.bucket, delta2));
                }
            }
        }
        EventDetectorModeV1::BucketLevels => {
            for row in rows {
                if row.value.is_finite() {
                    points.push((row.bucket, row.value));
                }
            }
        }
    }

    if points.len() < 3 {
        base.points_total = points.len() as u64;
        return base;
    }

    base.points_total = points.len() as u64;

    let mut values: Vec<f64> = points.iter().map(|(_, v)| *v).collect();
    let Some((center, raw_scale)) = robust::robust_scale(&mut values) else {
        return base;
    };
    let mut scale = if raw_scale.is_finite() && raw_scale > 0.0 {
        raw_scale
    } else {
        1.0
    };

    if matches!(
        options.detector_mode,
        EventDetectorModeV1::BucketDeltas | EventDetectorModeV1::BucketSecondDeltas
    ) {
        let mut abs_nonzero: Vec<f64> = points
            .iter()
            .map(|(_, d)| d.abs())
            .filter(|v| v.is_finite() && *v > 0.0)
            .collect();
        // Quantization-aware scale floor: only apply when we have enough samples for a stable median.
        if abs_nonzero.len() >= 5 {
            if let Some(q_step) = robust::median(&mut abs_nonzero) {
                if q_step.is_finite() && q_step > 0.0 && scale < q_step {
                    scale = q_step;
                }
            }
        }
    }

    if matches!(options.threshold_mode, EventThresholdModeV1::AdaptiveRate) {
        if let Some(desired) = desired_events {
            let mut z_abs: Vec<f64> = points
                .iter()
                .map(|(_, value)| ((*value - center) / scale).abs())
                .filter(|v| v.is_finite())
                .collect();
            z_abs.sort_by(|a, b| b.total_cmp(a));
            if z_abs.len() >= desired {
                let picked = z_abs[desired.saturating_sub(1)];
                if picked.is_finite() && picked > 0.0 {
                    threshold_used = threshold_used.max(picked);
                }
            }
        }
    }

    base.z_threshold_used = threshold_used;

    let mut events: Vec<EventPoint> = Vec::new();
    let mut peak_abs_z: f64 = 0.0;
    let mut has_peak = false;
    for (ts, value) in points {
        let z = (value - center) / scale;
        if !z.is_finite() {
            continue;
        }
        let z_abs = z.abs();
        if z_abs.is_finite() {
            if !has_peak || z_abs > peak_abs_z {
                peak_abs_z = z_abs;
                has_peak = true;
            }
        }
        let pass = z_abs >= threshold_used;
        if !pass {
            continue;
        }

        let direction = if z >= 0.0 {
            EventDirectionV1::Up
        } else {
            EventDirectionV1::Down
        };
        match polarity {
            EventPolarityV1::Both => {}
            EventPolarityV1::Up => {
                if direction != EventDirectionV1::Up {
                    continue;
                }
            }
            EventPolarityV1::Down => {
                if direction != EventDirectionV1::Down {
                    continue;
                }
            }
        }

        let ts_epoch = ts.timestamp();
        let is_boundary = (ts_epoch - start_epoch).abs() <= boundary_seconds
            || (end_epoch - ts_epoch).abs() <= boundary_seconds;
        if options.exclude_boundary_events && is_boundary {
            continue;
        }

        let delta = match options.detector_mode {
            EventDetectorModeV1::BucketLevels => value - center,
            _ => value,
        };

        events.push(EventPoint {
            ts,
            ts_epoch,
            z,
            direction,
            delta,
            is_boundary,
        });
    }

    if events.is_empty() {
        if has_peak {
            base.peak_abs_z = Some(peak_abs_z);
        }
        return base;
    }

    let window_seconds = options
        .min_separation_buckets
        .max(0)
        .saturating_mul(interval_seconds);
    let mut suppressed = match options.suppression_mode {
        EventSuppressionModeV1::GreedyMinSeparation => suppress_events_greedy(events, window_seconds),
        EventSuppressionModeV1::NmsWindow => suppress_events_nms(events, window_seconds),
    };

    if matches!(options.threshold_mode, EventThresholdModeV1::AdaptiveRate) {
        if let Some(max_events) = target_max_events {
            if suppressed.len() > max_events {
                suppressed.sort_by(|a, b| b.z.abs().total_cmp(&a.z.abs()));
                suppressed.truncate(max_events);
                let cutoff = suppressed
                    .iter()
                    .map(|e| e.z.abs())
                    .filter(|v| v.is_finite())
                    .fold(f64::INFINITY, f64::min);
                suppressed.sort_by_key(|e| e.ts_epoch);
                if cutoff.is_finite() {
                    base.z_threshold_used = base.z_threshold_used.max(cutoff);
                }
            }
        }
    }

    if options.max_events > 0 && suppressed.len() > options.max_events {
        suppressed.sort_by(|a, b| b.z.abs().total_cmp(&a.z.abs()));
        suppressed.truncate(options.max_events);
        suppressed.sort_by_key(|e| e.ts_epoch);
    }

    for evt in &suppressed {
        match evt.direction {
            EventDirectionV1::Up => base.up_events = base.up_events.saturating_add(1),
            EventDirectionV1::Down => base.down_events = base.down_events.saturating_add(1),
        }
        if evt.is_boundary {
            base.boundary_events = base.boundary_events.saturating_add(1);
        }
    }

    base.events = suppressed;
    if has_peak {
        base.peak_abs_z = Some(peak_abs_z);
    }
    base
}

fn suppress_events_greedy(mut events: Vec<EventPoint>, min_sep_seconds: i64) -> Vec<EventPoint> {
    if events.is_empty() {
        return events;
    }
    events.sort_by_key(|e| e.ts_epoch);
    if min_sep_seconds <= 0 {
        return events;
    }

    let mut merged: Vec<EventPoint> = Vec::new();
    for evt in events.into_iter() {
        if let Some(last) = merged.last_mut() {
            if evt.ts_epoch - last.ts_epoch <= min_sep_seconds {
                if evt.z.abs() > last.z.abs() {
                    *last = evt;
                }
                continue;
            }
        }
        merged.push(evt);
    }
    merged
}

fn suppress_events_nms(mut events: Vec<EventPoint>, window_seconds: i64) -> Vec<EventPoint> {
    if events.is_empty() {
        return events;
    }
    events.sort_by_key(|e| e.ts_epoch);
    if window_seconds <= 0 {
        return events;
    }

    let mut indices: Vec<usize> = (0..events.len()).collect();
    indices.sort_by(|a, b| {
        events[*b]
            .z
            .abs()
            .total_cmp(&events[*a].z.abs())
            .then_with(|| events[*a].ts_epoch.cmp(&events[*b].ts_epoch))
    });

    let mut suppressed: Vec<bool> = vec![false; events.len()];
    let mut selected: Vec<EventPoint> = Vec::new();

    for idx in indices {
        if suppressed[idx] {
            continue;
        }
        let picked = events[idx].clone();
        selected.push(picked.clone());

        let pick_t = picked.ts_epoch;
        for (j, evt) in events.iter().enumerate() {
            if suppressed[j] {
                continue;
            }
            if (evt.ts_epoch - pick_t).abs() <= window_seconds {
                suppressed[j] = true;
            }
        }
    }

    selected.sort_by_key(|e| e.ts_epoch);
    selected
}

#[cfg(test)]
mod tests {
    use super::{
        detect_change_events, detect_change_events_with_options, detect_change_events_with_options_and_delta_mode,
        EventDetectOptionsV1,
    };
    use super::{hour_of_day_mean_residual_rows, time_of_day_entropy, EventPoint};
    use crate::services::analysis::signal_semantics::DeltaMode;
    use crate::services::analysis::parquet_duckdb::MetricsBucketRow;
    use crate::services::analysis::tsse::types::{
        EventDetectorModeV1, EventPolarityV1, EventSuppressionModeV1, EventThresholdModeV1,
    };
    use chrono::{DateTime, Utc};

    fn row_at(epoch: i64, value: f64) -> MetricsBucketRow {
        MetricsBucketRow {
            sensor_id: "sensor-a".to_string(),
            bucket: DateTime::<Utc>::from_timestamp(epoch, 0).expect("ts"),
            value,
            samples: 1,
        }
    }

    #[test]
    fn gap_max_buckets_skips_large_time_jumps_and_counts_skipped_deltas() {
        let interval_seconds = 60;
        let rows = vec![
            row_at(0, 0.0),
            row_at(60, 0.0),
            row_at(120, 1.0),
            row_at(180, 1.0),
            // Large gap (simulated downtime)
            row_at(3600, 100.0),
            row_at(3660, 100.0),
        ];

        let detected_no_gap = detect_change_events(
            &rows,
            interval_seconds,
            3.0,
            0,
            0,
            EventPolarityV1::Both,
            1000,
        );
        assert_eq!(detected_no_gap.gap_skipped_deltas, 0);
        assert!(
            detected_no_gap.events.iter().any(|e| e.ts_epoch == 3600),
            "expected a gap-driven event when gap suppression is disabled"
        );

        let detected_gap = detect_change_events(
            &rows,
            interval_seconds,
            3.0,
            0,
            5,
            EventPolarityV1::Both,
            1000,
        );
        assert_eq!(detected_gap.gap_skipped_deltas, 1);
        assert!(
            !detected_gap.events.iter().any(|e| e.ts_epoch == 3600),
            "gap-driven delta should be ignored when gap suppression is enabled"
        );
    }

    #[test]
    fn quantization_scale_floor_prevents_inflated_zscores_for_step_like_series() {
        let interval_seconds = 60;

        // Delta series (10 deltas) alternates: 0, 0.1, 0, 0.1, ...
        // Non-zero deltas count = 5 so the q_step median is stable.
        let rows = vec![
            row_at(0, 0.0),
            row_at(60, 0.0),
            row_at(120, 0.1),
            row_at(180, 0.1),
            row_at(240, 0.2),
            row_at(300, 0.2),
            row_at(360, 0.3),
            row_at(420, 0.3),
            row_at(480, 0.4),
            row_at(540, 0.4),
            row_at(600, 0.5),
        ];

        // With the quantization floor, z magnitudes land at |z|=0.5 for this series
        // and should not exceed a 0.6 threshold.
        let detected = detect_change_events(
            &rows,
            interval_seconds,
            0.6,
            0,
            0,
            EventPolarityV1::Both,
            1000,
        );
        assert_eq!(detected.gap_skipped_deltas, 0);
        assert!(
            detected.events.is_empty(),
            "expected no events after applying quantization-aware scale floor"
        );
    }

    #[test]
    fn circular_delta_mode_suppresses_wind_direction_wrap_artifacts() {
        let interval_seconds = 60;
        // Build a series with small, varied deltas plus a large wrap jump (359° -> 1°).
        // The variation ensures MAD-based scaling (not IQR) so the wrap artifact is clearly detected
        // under linear deltas.
        let rows = vec![
            row_at(0, 359.0),
            row_at(60, 1.0),
            row_at(120, 4.0),
            row_at(180, 6.0),
            row_at(240, 9.0),
            row_at(300, 11.0),
        ];

        let options = EventDetectOptionsV1::fixed_default(
            interval_seconds,
            3.0,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        );

        let linear = detect_change_events_with_options(&rows, &options);
        assert!(
            linear.events.iter().any(|e| e.ts_epoch == 60),
            "expected a large wrap-driven event under linear deltas"
        );

        let circular =
            detect_change_events_with_options_and_delta_mode(&rows, &options, DeltaMode::CircularDegrees { period: 360.0 });
        assert!(
            !circular.events.iter().any(|e| e.ts_epoch == 60),
            "expected circular delta mode to suppress wrap-driven events"
        );
    }

    #[test]
    fn non_negative_reset_delta_mode_suppresses_daily_total_reset_artifacts() {
        let interval_seconds = 60;
        // Simulate daily cumulative total that resets to 0.
        let rows = vec![
            row_at(0, 0.0),
            row_at(60, 5.0),
            row_at(120, 7.0),
            row_at(180, 0.0),
            row_at(240, 1.0),
            row_at(300, 2.0),
        ];

        let options = EventDetectOptionsV1::fixed_default(
            interval_seconds,
            3.0,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        );

        let linear = detect_change_events_with_options(&rows, &options);
        assert!(
            linear.events.iter().any(|e| e.ts_epoch == 180),
            "expected a large reset-driven event under linear deltas"
        );

        let clamped =
            detect_change_events_with_options_and_delta_mode(&rows, &options, DeltaMode::NonNegativeReset);
        assert!(
            !clamped.events.iter().any(|e| e.ts_epoch == 180),
            "expected NonNegativeReset delta mode to suppress reset-driven events"
        );
    }

    #[test]
    fn nms_picks_strongest_event_in_cluster_like_greedy() {
        let interval_seconds = 60;
        let rows = vec![
            row_at(0, 0.0),
            row_at(60, 0.0),
            row_at(120, 1.0),
            row_at(180, 1.0),
            row_at(240, 4.0),
            row_at(300, 4.0),
        ];

        let mut greedy = EventDetectOptionsV1::fixed_default(
            interval_seconds,
            0.8,
            3,
            0,
            EventPolarityV1::Both,
            10_000,
        );
        greedy.suppression_mode = EventSuppressionModeV1::GreedyMinSeparation;
        greedy.detector_mode = EventDetectorModeV1::BucketDeltas;
        greedy.threshold_mode = EventThresholdModeV1::FixedZ;

        let mut nms = greedy.clone();
        nms.suppression_mode = EventSuppressionModeV1::NmsWindow;

        let detected_greedy = detect_change_events_with_options(&rows, &greedy);
        let detected_nms = detect_change_events_with_options(&rows, &nms);

        assert_eq!(detected_greedy.events.len(), 1, "expected cluster merge into one event");
        assert_eq!(detected_nms.events.len(), 1, "expected NMS to merge into one event");
        assert_eq!(detected_greedy.events[0].ts_epoch, 240);
        assert_eq!(detected_nms.events[0].ts_epoch, 240);
    }

    #[test]
    fn adaptive_threshold_targets_event_count_on_levels_detector() {
        let interval_seconds = 60;
        let rows = vec![
            row_at(0, -100.0),
            row_at(60, -50.0),
            row_at(120, 0.0),
            row_at(180, 50.0),
            row_at(240, 100.0),
        ];

        let mut options = EventDetectOptionsV1::fixed_default(
            interval_seconds,
            3.0,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        );
        options.detector_mode = EventDetectorModeV1::BucketLevels;
        options.threshold_mode = EventThresholdModeV1::AdaptiveRate;
        options.adaptive = Some(crate::services::analysis::tsse::types::AdaptiveThresholdConfigV1 {
            target_min_events: None,
            target_max_events: Some(2),
            min_z: Some(0.1),
        });

        let detected = detect_change_events_with_options(&rows, &options);
        assert_eq!(detected.events.len(), 2, "expected adaptive to keep ~2 endpoints");
        assert!(
            detected.z_threshold_used.is_finite() && detected.z_threshold_used > 0.5,
            "expected adaptive mode to raise the effective z threshold above the min_z floor"
        );
    }

    #[test]
    fn hour_of_day_mean_residual_removes_perfect_daily_pattern() {
        let interval_seconds = 3600;
        let mut rows: Vec<MetricsBucketRow> = Vec::new();
        for day in 0..3 {
            for hour in 0..24 {
                let t = (day * 24 + hour) as i64 * interval_seconds;
                // Pure diurnal signal repeated exactly each day.
                let phase = (hour as f64) * std::f64::consts::TAU / 24.0;
                let value = phase.sin();
                rows.push(row_at(t, value));
            }
        }

        let detected_raw = detect_change_events(
            &rows,
            interval_seconds,
            0.1,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        );
        assert!(
            !detected_raw.events.is_empty(),
            "expected some events on raw diurnal series at low threshold"
        );

        let residual = hour_of_day_mean_residual_rows(&rows);
        let detected_resid = detect_change_events(
            &residual,
            interval_seconds,
            0.1,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        );
        assert!(
            detected_resid.events.is_empty(),
            "expected deseasoned residual to remove diurnal events"
        );
    }

    #[test]
    fn time_of_day_entropy_penalizes_fixed_hour_events() {
        let mut events: Vec<EventPoint> = Vec::new();
        for day in 0..10_i64 {
            let epoch = day * 86_400 + 12 * 3600;
            let ts = DateTime::<Utc>::from_timestamp(epoch, 0).expect("ts");
            events.push(EventPoint {
                ts,
                ts_epoch: epoch,
                z: 5.0,
                direction: crate::services::analysis::tsse::types::EventDirectionV1::Up,
                delta: 1.0,
                is_boundary: false,
            });
        }

        let entropy = time_of_day_entropy(&events).expect("entropy");
        assert!((entropy.h_norm - 0.0).abs() < 1e-9);
        assert!((entropy.weight - 0.25).abs() < 1e-9);
    }

    #[test]
    fn time_of_day_entropy_is_near_one_for_uniform_hours() {
        let mut events: Vec<EventPoint> = Vec::new();
        for hour in 0..24_i64 {
            let epoch = hour * 3600;
            let ts = DateTime::<Utc>::from_timestamp(epoch, 0).expect("ts");
            events.push(EventPoint {
                ts,
                ts_epoch: epoch,
                z: 5.0,
                direction: crate::services::analysis::tsse::types::EventDirectionV1::Up,
                delta: 1.0,
                is_boundary: false,
            });
        }

        let entropy = time_of_day_entropy(&events).expect("entropy");
        assert!(
            entropy.h_norm > 0.99,
            "expected normalized entropy near 1 for uniform distribution"
        );
        assert!((entropy.weight - 1.0).abs() < 1e-6);
    }
}
