use crate::services::analysis::parquet_duckdb::BucketAggregationMode;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeltaMode {
    Linear,
    CircularDegrees { period: f64 },
    /// Treat negative deltas as resets and clamp to 0.
    ///
    /// Intended for monotonically increasing cumulative totals that periodically reset
    /// (e.g., "daily rain total" sensors that reset at midnight).
    NonNegativeReset,
}

impl Default for DeltaMode {
    fn default() -> Self {
        Self::Linear
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SensorAnalysisSemantics {
    pub bucket_mode: BucketAggregationMode,
    pub delta_mode: DeltaMode,
}

fn normalize_sensor_type(sensor_type: Option<&str>) -> String {
    sensor_type
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
}

pub fn infer_semantics(sensor_type: Option<&str>, _source: Option<&str>) -> SensorAnalysisSemantics {
    let normalized = normalize_sensor_type(sensor_type);

    if normalized.is_empty() {
        return SensorAnalysisSemantics {
            bucket_mode: BucketAggregationMode::Avg,
            delta_mode: DeltaMode::Linear,
        };
    }

    // Direction signals are circular; averaging is meaningless and naive deltas wrap badly.
    if normalized.contains("wind_direction")
        || (normalized.contains("wind") && normalized.contains("direction"))
    {
        return SensorAnalysisSemantics {
            bucket_mode: BucketAggregationMode::Last,
            delta_mode: DeltaMode::CircularDegrees { period: 360.0 },
        };
    }

    // Rates should be averaged, not summed.
    if normalized.contains("rain_rate") || normalized.contains("rainrate") {
        return SensorAnalysisSemantics {
            bucket_mode: BucketAggregationMode::Avg,
            delta_mode: DeltaMode::Linear,
        };
    }

    // Explicit rain/precip counters (pulse/tips) should be summed per bucket.
    if (normalized.contains("rain") || normalized.contains("precip"))
        && (normalized.contains("gauge")
            || normalized.contains("tip")
            || normalized.contains("pulse")
            || normalized.contains("counter"))
    {
        return SensorAnalysisSemantics {
            bucket_mode: BucketAggregationMode::Sum,
            delta_mode: DeltaMode::Linear,
        };
    }

    // Daily cumulative totals should use LAST within buckets; their deltas should ignore resets.
    if normalized == "rain"
        || normalized.contains("daily_rain")
        || normalized.contains("rain_daily")
        || normalized.contains("cumulative_rain")
        || normalized.contains("rain_cumulative")
    {
        return SensorAnalysisSemantics {
            bucket_mode: BucketAggregationMode::Last,
            delta_mode: DeltaMode::NonNegativeReset,
        };
    }

    // Generic counter-like types.
    if normalized.contains("pulse") || normalized.contains("flow") || normalized.contains("counter") {
        return SensorAnalysisSemantics {
            bucket_mode: BucketAggregationMode::Sum,
            delta_mode: DeltaMode::Linear,
        };
    }

    // State-like types should use LAST.
    if normalized.contains("state")
        || normalized.contains("status")
        || normalized.contains("bool")
        || normalized.contains("switch")
        || normalized.contains("contact")
        || normalized.contains("mode")
    {
        return SensorAnalysisSemantics {
            bucket_mode: BucketAggregationMode::Last,
            delta_mode: DeltaMode::Linear,
        };
    }

    // Generic precipitation amounts (non-rate, non-daily) behave like counters.
    if normalized.contains("rain") || normalized.contains("precip") {
        return SensorAnalysisSemantics {
            bucket_mode: BucketAggregationMode::Sum,
            delta_mode: DeltaMode::Linear,
        };
    }

    SensorAnalysisSemantics {
        bucket_mode: BucketAggregationMode::Avg,
        delta_mode: DeltaMode::Linear,
    }
}

pub fn auto_bucket_mode(sensor_type: Option<&str>) -> BucketAggregationMode {
    infer_semantics(sensor_type, None).bucket_mode
}

pub fn auto_delta_mode(sensor_type: Option<&str>, source: Option<&str>) -> DeltaMode {
    infer_semantics(sensor_type, source).delta_mode
}

pub fn is_level_like_sensor_type(sensor_type: &str) -> bool {
    let normalized = normalize_sensor_type(Some(sensor_type));
    normalized.contains("water_level") || normalized.contains("reservoir_depth") || normalized.contains("depth")
}

pub fn delta(prev: f64, curr: f64, mode: DeltaMode) -> f64 {
    match mode {
        DeltaMode::Linear => curr - prev,
        DeltaMode::NonNegativeReset => (curr - prev).max(0.0),
        DeltaMode::CircularDegrees { period } => {
            if !period.is_finite() || period <= 0.0 {
                return curr - prev;
            }
            let mut d = curr - prev;
            if !d.is_finite() {
                return d;
            }
            d = d.rem_euclid(period);
            let half = period / 2.0;
            if d > half {
                d -= period;
            }
            d
        }
    }
}

