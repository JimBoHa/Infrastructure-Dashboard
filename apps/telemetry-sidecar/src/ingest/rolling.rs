use super::types::Sample;
use chrono::{DateTime, Duration as ChronoDuration, TimeZone, Utc};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
struct RollingConfig {
    interval_seconds: i64,
    rolling_avg_seconds: i64,
}

#[derive(Debug)]
pub(in crate::ingest) struct RollingAverager {
    config: RollingConfig,
    buffer: VecDeque<Sample>,
    next_emit: Option<DateTime<Utc>>,
}

impl RollingAverager {
    pub(in crate::ingest) fn new(interval_seconds: i64, rolling_avg_seconds: i64) -> Self {
        Self {
            config: RollingConfig {
                interval_seconds: interval_seconds.max(1),
                rolling_avg_seconds: rolling_avg_seconds.max(1),
            },
            buffer: VecDeque::new(),
            next_emit: None,
        }
    }

    pub(in crate::ingest) fn add_sample(&mut self, sample: Sample) -> Vec<Sample> {
        let mut outputs = Vec::new();
        self.buffer.push_back(sample.clone());

        if self.next_emit.is_none() {
            self.next_emit = Some(align_up(sample.timestamp, self.config.interval_seconds));
        }

        while let Some(next) = self.next_emit {
            if sample.timestamp < next {
                break;
            }
            if let Some(emitted) = self.emit_at(next) {
                outputs.push(emitted);
            }
            self.next_emit = Some(next + ChronoDuration::seconds(self.config.interval_seconds));
        }

        outputs
    }

    fn emit_at(&mut self, emit_ts: DateTime<Utc>) -> Option<Sample> {
        let window_start = emit_ts - ChronoDuration::seconds(self.config.rolling_avg_seconds);
        let mut total_value = 0.0;
        let mut total_quality = 0.0;
        let mut total_samples = 0i64;

        for entry in &self.buffer {
            if entry.timestamp > emit_ts {
                break;
            }
            if entry.timestamp <= window_start {
                continue;
            }
            total_value += entry.value;
            total_quality += entry.quality as f64;
            total_samples += entry.samples;
        }

        if total_samples == 0 {
            return None;
        }

        while let Some(front) = self.buffer.front() {
            if front.timestamp <= window_start {
                self.buffer.pop_front();
            } else {
                break;
            }
        }

        let avg_value = total_value / total_samples as f64;
        let avg_quality = (total_quality / total_samples as f64).round() as i32;

        Some(Sample {
            timestamp: emit_ts,
            value: avg_value,
            quality: avg_quality,
            samples: total_samples,
        })
    }
}

fn align_down(ts: DateTime<Utc>, interval_seconds: i64) -> DateTime<Utc> {
    let interval = interval_seconds.max(1);
    let interval_ms = interval * 1000;
    let ts_ms = ts.timestamp_millis();
    let bucket_ms = ts_ms.div_euclid(interval_ms) * interval_ms;
    Utc.timestamp_millis_opt(bucket_ms).single().unwrap_or(ts)
}

fn align_up(ts: DateTime<Utc>, interval_seconds: i64) -> DateTime<Utc> {
    let interval = interval_seconds.max(1);
    let interval_ms = interval * 1000;
    let ts_ms = ts.timestamp_millis();
    if ts_ms % interval_ms == 0 {
        return align_down(ts, interval_seconds);
    }
    align_down(ts, interval_seconds) + ChronoDuration::milliseconds(interval_ms)
}
