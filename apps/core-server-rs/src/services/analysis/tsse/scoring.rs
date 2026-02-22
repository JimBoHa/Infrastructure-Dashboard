use super::robust;
use crate::services::analysis::parquet_duckdb::MetricsBucketRow;
use crate::services::analysis::stats::correlation::{
    effective_sample_size_lag1, lag1_autocorr, pearson_confidence_interval_fisher_z,
    pearson_p_value_fisher_z, z_value_for_alpha,
};
use crate::services::analysis::tsse::types::TsseEpisodeV1;
use chrono::{DateTime, TimeZone, Utc};
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct ScoredEpisodes {
    pub best_lag_seconds: i64,
    pub best_lag_r_ci_low: Option<f64>,
    pub best_lag_r_ci_high: Option<f64>,
    pub episodes: Vec<TsseEpisodeV1>,
    pub score: f64,
    pub best_window_sec: Option<i64>,
    pub coverage_pct: Option<f64>,
    pub score_components: BTreeMap<String, f64>,
    pub penalties: Vec<String>,
    pub bonuses: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LagCandidate {
    pub lag_seconds: i64,
    pub score: f64,
    pub n: usize,
}

#[derive(Debug, Clone)]
pub struct LagSearchResult {
    pub best: LagCandidate,
    pub m_lag: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ScoreTimings {
    pub total_ms: u64,
    pub lag_search_ms: u64,
    pub episode_extract_ms: u64,
}

// Scoring tunables: keep defaults conservative; favor repeatable signal over spikes.
// If you tune these, run `tsse_bench` and keep recall/precision evidence in `reports/`.
/// Weight of the aggregated episode score in the final score (stability over spikes).
const PRIMARY_SCORE_WEIGHT: f64 = 0.5;
/// Weight of the best single-episode peak (allows sharp, high-quality bursts to matter).
const PEAK_SCORE_WEIGHT: f64 = 0.5;
/// Bonus when multiple strong episodes corroborate the relationship (gated by quality).
const MULTI_EPISODE_BONUS: f64 = 0.02;
/// Minimum episode peak required for multi_episode bonus (prevents diurnal artifact inflation).
const MULTI_EPISODE_MIN_PEAK: f64 = 0.92;
/// Penalty for tiny overlap coverage to avoid high scores from short coincidences.
const SHORT_COVERAGE_PENALTY: f64 = 0.85;
/// Penalty when aligned points are sparse (mitigates low-overlap false positives).
const LOW_OVERLAP_PENALTY: f64 = 0.9;
/// Minimum overlap multiplier above `min_significant_n` before treating overlap as healthy.
const LOW_OVERLAP_MULTIPLIER: usize = 2;
/// Contribution weight per episode in the union-aggregation; lower keeps saturation gradual.
const EPISODE_CONTRIB_WEIGHT: f64 = 0.10;
/// Cap on contributing episodes to keep compute stable and avoid overweighting long lists.
const MAX_EPISODE_CONTRIB: usize = 8;
/// Minimum episode peak to contribute to union aggregation (filters weak episodes).
const EPISODE_CONTRIB_MIN_PEAK: f64 = 0.80;
/// Coverage floor for applying the short-coverage penalty (2% of horizon).
const SHORT_COVERAGE_FLOOR: f64 = 0.02;
/// Minimum aligned points for avoiding low-overlap penalty (overlap below this is suspect).
const LOW_OVERLAP_MIN_POINTS: usize = 20;
/// CI width threshold - penalize if confidence interval is wider than this.
const CI_WIDTH_PENALTY_THRESHOLD: f64 = 0.08;
/// Penalty multiplier for wide confidence intervals.
const CI_WIDTH_PENALTY: f64 = 0.92;
/// Power for score compression - spreads out high scores to preserve ranking differentiation.
/// score_final = score^COMPRESSION (e.g., 0.95^1.3 ≈ 0.93, 0.85^1.3 ≈ 0.80).
const SCORE_COMPRESSION_POWER: f64 = 1.3;
/// A "diurnal" lag is a near-integer multiple of 24h (common, often non-informative periodicity).
const DIURNAL_LAG_SECONDS: i64 = 86_400;
/// Tolerance for diurnal lag detection (allows some slop around exact 24h multiples).
const DIURNAL_LAG_TOLERANCE_SECONDS: i64 = 30 * 60;
/// Penalize diurnal relationships by default; keep discoverable but down-rank to reduce false positives.
const DIURNAL_LAG_PENALTY_MULTIPLIER: f64 = 0.35;
/// Blend the final score with the global lag correlation signal so tiny |r| cannot score near 1.0.
/// score *= clamp(LAG_SIGNAL_FLOOR + LAG_SIGNAL_WEIGHT * |r|).
const LAG_SIGNAL_FLOOR: f64 = 0.20;
const LAG_SIGNAL_WEIGHT: f64 = 0.80;

fn is_diurnal_lag_seconds(lag_seconds: i64) -> bool {
    if lag_seconds == 0 {
        return false;
    }
    let abs = lag_seconds.abs();
    let nearest_multiple = (abs + DIURNAL_LAG_SECONDS / 2) / DIURNAL_LAG_SECONDS;
    if nearest_multiple <= 0 {
        return false;
    }
    let nearest = nearest_multiple * DIURNAL_LAG_SECONDS;
    (abs - nearest).abs() <= DIURNAL_LAG_TOLERANCE_SECONDS
}

pub fn score_related_series_with_timings(
    params: ScoreParams,
) -> (Option<ScoredEpisodes>, ScoreTimings) {
    let started = Instant::now();
    let mut timings = ScoreTimings::default();

    let interval_seconds = params.interval_seconds.max(1);
    let lag_max_seconds = params.lag_max_seconds.max(0);
    let lag_max_buckets = (lag_max_seconds / interval_seconds).max(0) as i64;

    let focus = match Series::from_buckets(&params.focus) {
        Some(value) => value,
        None => {
            timings.total_ms = started.elapsed().as_millis() as u64;
            return (None, timings);
        }
    };
    let candidate = match Series::from_buckets(&params.candidate) {
        Some(value) => value,
        None => {
            timings.total_ms = started.elapsed().as_millis() as u64;
            return (None, timings);
        }
    };

    let focus_z_points = match focus.robust_z_points(params.z_clip) {
        Some(value) => value,
        None => {
            timings.total_ms = started.elapsed().as_millis() as u64;
            return (None, timings);
        }
    };
    let candidate_z_points = match candidate.robust_z_points(params.z_clip) {
        Some(value) => value,
        None => {
            timings.total_ms = started.elapsed().as_millis() as u64;
            return (None, timings);
        }
    };
    let focus_z: HashMap<i64, f64> = focus_z_points.iter().copied().collect();
    let candidate_z: HashMap<i64, f64> = candidate_z_points.iter().copied().collect();
    let rho1_focus = lag1_autocorr(&focus_z_points.iter().map(|(_, v)| *v).collect::<Vec<_>>());
    let rho1_candidate = lag1_autocorr(
        &candidate_z_points
            .iter()
            .map(|(_, v)| *v)
            .collect::<Vec<_>>(),
    );

    let lag_started = Instant::now();
    let lag_result = if lag_max_buckets == 0 {
        LagSearchResult {
            best: LagCandidate {
                lag_seconds: 0,
                score: pearson_from_maps(&focus_z, &candidate_z, 0).abs(),
                n: aligned_count(&focus_z, &candidate_z, 0),
            },
            m_lag: 1,
        }
    } else {
        let min_overlap = params.min_significant_n.max(3);
        best_lag_search(
            &focus_z,
            &candidate_z,
            interval_seconds,
            lag_max_buckets,
            params.coarse_lag_steps,
            min_overlap,
        )
    };
    timings.lag_search_ms = lag_started.elapsed().as_millis() as u64;

    let min_significant_n = params.min_significant_n.max(3);
    let significance_alpha = params.significance_alpha.clamp(0.000_1, 0.5);
    let min_abs_r = params.min_abs_r.clamp(0.0, 1.0);

    let best_lag = lag_result.best;
    let m_lag = lag_result.m_lag.max(1);

    if best_lag.score < min_abs_r {
        timings.total_ms = started.elapsed().as_millis() as u64;
        return (None, timings);
    }

    let lag_r_signed = pearson_from_maps(&focus_z, &candidate_z, best_lag.lag_seconds);
    let n_eff = effective_sample_size_lag1(best_lag.n, rho1_focus, rho1_candidate);
    let lag_p_raw = pearson_p_value(lag_r_signed.abs(), n_eff);
    let lag_p_lag = lag_p_raw.map(|p| {
        let base = (1.0 - p).clamp(0.0, 1.0);
        let m = (m_lag as f64).max(1.0);
        (1.0 - base.powf(m)).clamp(0.0, 1.0)
    });
    let lag_ci = z_value_for_alpha(significance_alpha)
        .and_then(|z_value| pearson_confidence_interval(lag_r_signed, n_eff, z_value));
    if best_lag.n < min_significant_n {
        timings.total_ms = started.elapsed().as_millis() as u64;
        return (None, timings);
    }
    if lag_p_lag.is_none() || lag_p_lag.unwrap_or(1.0) > significance_alpha {
        timings.total_ms = started.elapsed().as_millis() as u64;
        return (None, timings);
    }

    let windows_seconds = choose_windows(params.horizon_seconds.max(0), interval_seconds);
    let mut episodes: Vec<TsseEpisodeV1> = Vec::new();
    let episode_started = Instant::now();
    for &window_sec in &windows_seconds {
        if let Some(mut eps) = extract_episodes_for_window(
            &focus_z,
            &candidate_z,
            best_lag.lag_seconds,
            interval_seconds,
            window_sec,
            params.horizon_seconds.max(1),
            params.episode_threshold,
        ) {
            episodes.append(&mut eps);
        }
    }
    timings.episode_extract_ms = episode_started.elapsed().as_millis() as u64;

    episodes.sort_by(|a, b| {
        b.score_peak
            .total_cmp(&a.score_peak)
            .then_with(|| a.start_ts.cmp(&b.start_ts))
    });
    if episodes.len() > params.max_episodes {
        episodes.truncate(params.max_episodes);
    }

    let mut score_components = BTreeMap::new();
    let mut penalties = Vec::new();
    let mut bonuses = Vec::new();

    let score_primary = combine_episode_scores(&episodes);
    score_components.insert("score_primary".to_string(), score_primary);
    score_components.insert("lag_score".to_string(), best_lag.score);
    // Back-compat/clarity aliases used by some UIs/clients.
    score_components.insert("lag_r_abs".to_string(), best_lag.score);
    score_components.insert("lag_r_signed".to_string(), lag_r_signed);
    score_components.insert("min_abs_r".to_string(), min_abs_r);
    score_components.insert("aligned_points".to_string(), best_lag.n as f64);
    score_components.insert("n_eff".to_string(), n_eff as f64);
    score_components.insert("m_lag".to_string(), m_lag as f64);
    if let Some(p_value) = lag_p_raw {
        score_components.insert("lag_p_raw".to_string(), p_value);
    }
    if let Some(p_value) = lag_p_lag {
        score_components.insert("lag_p_lag".to_string(), p_value);
        // Back-compat key used by the UI: treat it as the *gating* p-value after lag correction.
        score_components.insert("lag_p_value".to_string(), p_value);
    }
    if let Some((low, high)) = lag_ci {
        score_components.insert("lag_ci_low".to_string(), low);
        score_components.insert("lag_ci_high".to_string(), high);
    }

    let best_episode = episodes.first().cloned();
    let best_window_sec = best_episode.as_ref().map(|ep| ep.window_sec);
    let coverage_pct = best_episode.as_ref().map(|ep| ep.coverage * 100.0);

    let is_diurnal = is_diurnal_lag_seconds(best_lag.lag_seconds);
    score_components.insert("is_diurnal_lag".to_string(), if is_diurnal { 1.0 } else { 0.0 });
    if is_diurnal {
        penalties.push("diurnal_lag".to_string());
    }

    let mut score = score_primary;
    if let Some(best) = &best_episode {
        score_components.insert("best_peak".to_string(), best.score_peak);
        score_components.insert("best_mean".to_string(), best.score_mean);
        score_components.insert("coverage".to_string(), best.coverage);
        score = (score * PRIMARY_SCORE_WEIGHT + best.score_peak * PEAK_SCORE_WEIGHT)
            .max(0.0)
            .min(1.0);

        if best.coverage < SHORT_COVERAGE_FLOOR {
            // Penalize short coverage to avoid high scores from tiny overlapping spans.
            score *= SHORT_COVERAGE_PENALTY;
            penalties.push("short_coverage".to_string());
        }
    }

    // Only apply multi_episode bonus if episodes are genuinely strong (not diurnal artifacts).
    let strong_episode_count = episodes
        .iter()
        .filter(|ep| ep.score_peak >= MULTI_EPISODE_MIN_PEAK)
        .count();
    if !is_diurnal && strong_episode_count >= 2 {
        score = (score + MULTI_EPISODE_BONUS).min(1.0);
        bonuses.push("multi_episode".to_string());
    }

    let low_overlap_min_points =
        (min_significant_n.saturating_mul(LOW_OVERLAP_MULTIPLIER)).max(LOW_OVERLAP_MIN_POINTS);
    if best_lag.n < low_overlap_min_points {
        // Penalize low overlap to reduce false positives from sparse alignment.
        score *= LOW_OVERLAP_PENALTY;
        penalties.push("low_overlap".to_string());
    }

    // Penalize wide confidence intervals - suggests uncertainty in the correlation estimate.
    if let Some((ci_low, ci_high)) = lag_ci {
        let ci_width = (ci_high - ci_low).abs();
        score_components.insert("ci_width".to_string(), ci_width);
        if ci_width > CI_WIDTH_PENALTY_THRESHOLD {
            score *= CI_WIDTH_PENALTY;
            penalties.push("wide_ci".to_string());
        }
    }

    // Base signal bounding: a strong episodic match still needs some global |r| support.
    // This is especially important for periodic/diurnal artifacts and tiny-overlap coincidences.
    let lag_signal_factor = (LAG_SIGNAL_FLOOR + LAG_SIGNAL_WEIGHT * best_lag.score).clamp(0.0, 1.0);
    score_components.insert("lag_signal_factor".to_string(), lag_signal_factor);
    score *= lag_signal_factor;

    if is_diurnal {
        score_components.insert("diurnal_penalty_multiplier".to_string(), DIURNAL_LAG_PENALTY_MULTIPLIER);
        score *= DIURNAL_LAG_PENALTY_MULTIPLIER;
    }

    // Apply score compression to spread out high scores and preserve ranking differentiation.
    // This prevents many sensors from bunching at 1.00.
    let score_uncompressed = score;
    score = score.powf(SCORE_COMPRESSION_POWER).max(0.0).min(1.0);
    score_components.insert("score_uncompressed".to_string(), score_uncompressed);

    let scored = ScoredEpisodes {
        best_lag_seconds: best_lag.lag_seconds,
        best_lag_r_ci_low: lag_ci.map(|v| v.0),
        best_lag_r_ci_high: lag_ci.map(|v| v.1),
        episodes,
        score: score.max(0.0).min(1.0),
        best_window_sec,
        coverage_pct,
        score_components,
        penalties,
        bonuses,
    };
    timings.total_ms = started.elapsed().as_millis() as u64;
    (Some(scored), timings)
}

pub fn score_related_series(params: ScoreParams) -> Option<ScoredEpisodes> {
    score_related_series_with_timings(params).0
}

#[derive(Debug, Clone)]
pub struct ScoreParams {
    pub focus: Vec<MetricsBucketRow>,
    pub candidate: Vec<MetricsBucketRow>,
    pub interval_seconds: i64,
    pub horizon_seconds: i64,
    pub lag_max_seconds: i64,
    pub coarse_lag_steps: i64,
    pub z_clip: f64,
    pub episode_threshold: f64,
    pub max_episodes: usize,
    pub min_significant_n: usize,
    pub significance_alpha: f64,
    pub min_abs_r: f64,
}

#[derive(Debug, Clone)]
pub struct LagInferenceSummary {
    pub best_lag_seconds: i64,
    pub best_lag_abs_r: f64,
    pub aligned_points: usize,
    pub n_eff: usize,
    pub m_lag: usize,
    pub p_raw: Option<f64>,
    pub p_lag: Option<f64>,
    pub r_ci_low: Option<f64>,
    pub r_ci_high: Option<f64>,
}

pub fn infer_lag_inference(params: ScoreParams) -> Option<LagInferenceSummary> {
    let interval_seconds = params.interval_seconds.max(1);
    let lag_max_seconds = params.lag_max_seconds.max(0);
    let lag_max_buckets = (lag_max_seconds / interval_seconds).max(0) as i64;

    let focus = Series::from_buckets(&params.focus)?;
    let candidate = Series::from_buckets(&params.candidate)?;

    let focus_z_points = focus.robust_z_points(params.z_clip)?;
    let candidate_z_points = candidate.robust_z_points(params.z_clip)?;
    let focus_z: HashMap<i64, f64> = focus_z_points.iter().copied().collect();
    let candidate_z: HashMap<i64, f64> = candidate_z_points.iter().copied().collect();

    let rho1_focus = lag1_autocorr(&focus_z_points.iter().map(|(_, v)| *v).collect::<Vec<_>>());
    let rho1_candidate = lag1_autocorr(
        &candidate_z_points
            .iter()
            .map(|(_, v)| *v)
            .collect::<Vec<_>>(),
    );

    let min_overlap = params.min_significant_n.max(3);
    let significance_alpha = params.significance_alpha.clamp(0.000_1, 0.5);

    let lag_result = if lag_max_buckets == 0 {
        LagSearchResult {
            best: LagCandidate {
                lag_seconds: 0,
                score: pearson_from_maps(&focus_z, &candidate_z, 0).abs(),
                n: aligned_count(&focus_z, &candidate_z, 0),
            },
            m_lag: 1,
        }
    } else {
        best_lag_search(
            &focus_z,
            &candidate_z,
            interval_seconds,
            lag_max_buckets,
            params.coarse_lag_steps,
            min_overlap,
        )
    };
    let best_lag = lag_result.best;
    let m_lag = lag_result.m_lag.max(1);

    let lag_r_signed = pearson_from_maps(&focus_z, &candidate_z, best_lag.lag_seconds);
    let n_eff = effective_sample_size_lag1(best_lag.n, rho1_focus, rho1_candidate);

    let (p_raw, p_lag, r_ci_low, r_ci_high) = if best_lag.n >= min_overlap {
        let p_raw = pearson_p_value(lag_r_signed.abs(), n_eff);
        let p_lag = p_raw.map(|p| {
            let base = (1.0 - p).clamp(0.0, 1.0);
            let m = (m_lag as f64).max(1.0);
            (1.0 - base.powf(m)).clamp(0.0, 1.0)
        });
        let lag_ci = z_value_for_alpha(significance_alpha)
            .and_then(|z_value| pearson_confidence_interval(lag_r_signed, n_eff, z_value));
        (p_raw, p_lag, lag_ci.map(|v| v.0), lag_ci.map(|v| v.1))
    } else {
        (None, None, None, None)
    };

    Some(LagInferenceSummary {
        best_lag_seconds: best_lag.lag_seconds,
        best_lag_abs_r: best_lag.score,
        aligned_points: best_lag.n,
        n_eff,
        m_lag,
        p_raw,
        p_lag,
        r_ci_low,
        r_ci_high,
    })
}

impl Default for ScoreParams {
    fn default() -> Self {
        Self {
            focus: Vec::new(),
            candidate: Vec::new(),
            interval_seconds: 60,
            horizon_seconds: 3600,
            lag_max_seconds: 0,
            coarse_lag_steps: 25,
            z_clip: 6.0,
            episode_threshold: 0.6,
            max_episodes: 50,
            min_significant_n: 10,
            significance_alpha: 0.05,
            min_abs_r: 0.2,
        }
    }
}

#[derive(Debug, Clone)]
struct Series {
    points: Vec<(i64, f64)>,
}

impl Series {
    fn from_buckets(rows: &[MetricsBucketRow]) -> Option<Self> {
        let mut points: Vec<(i64, f64)> = rows
            .iter()
            .filter(|r| r.value.is_finite())
            .map(|r| (r.bucket.timestamp(), r.value))
            .collect();
        if points.len() < 3 {
            return None;
        }
        points.sort_by_key(|(ts, _)| *ts);
        points.dedup_by_key(|(ts, _)| *ts);
        Some(Self { points })
    }

    fn values(&self) -> Vec<f64> {
        self.points.iter().map(|(_, v)| *v).collect()
    }

    fn robust_z_points(&self, clip: f64) -> Option<Vec<(i64, f64)>> {
        let values = self.values();
        let z = robust::zscore_robust(&values, clip)?;
        let mut out = Vec::with_capacity(self.points.len());
        for (idx, (ts, _)) in self.points.iter().enumerate() {
            if let Some(v) = z.get(idx) {
                out.push((*ts, *v));
            }
        }
        Some(out)
    }
}

fn aligned_count(
    focus: &HashMap<i64, f64>,
    candidate: &HashMap<i64, f64>,
    lag_seconds: i64,
) -> usize {
    let mut n = 0;
    for (ts, _) in focus.iter() {
        if candidate.contains_key(&(ts + lag_seconds)) {
            n += 1;
        }
    }
    n
}

fn pearson_from_maps(
    focus: &HashMap<i64, f64>,
    candidate: &HashMap<i64, f64>,
    lag_seconds: i64,
) -> f64 {
    let mut x: Vec<f64> = Vec::new();
    let mut y: Vec<f64> = Vec::new();
    x.reserve(focus.len());
    y.reserve(focus.len());
    for (ts, xv) in focus.iter() {
        if let Some(yv) = candidate.get(&(ts + lag_seconds)) {
            if xv.is_finite() && yv.is_finite() {
                x.push(*xv);
                y.push(*yv);
            }
        }
    }
    pearson(&x, &y).unwrap_or(0.0)
}

fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 3 {
        return None;
    }
    let n = x.len() as f64;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_yy = 0.0;
    let mut sum_xy = 0.0;
    for (xv, yv) in x.iter().zip(y.iter()) {
        sum_x += *xv;
        sum_y += *yv;
        sum_xx += xv * xv;
        sum_yy += yv * yv;
        sum_xy += xv * yv;
    }
    let denom_x = n * sum_xx - sum_x * sum_x;
    let denom_y = n * sum_yy - sum_y * sum_y;
    if denom_x <= 0.0 || denom_y <= 0.0 {
        return None;
    }
    let r = (n * sum_xy - sum_x * sum_y) / (denom_x * denom_y).sqrt();
    if !r.is_finite() {
        return None;
    }
    Some(r.max(-1.0).min(1.0))
}

fn pearson_p_value(r: f64, n: usize) -> Option<f64> {
    pearson_p_value_fisher_z(r, n)
}

fn pearson_confidence_interval(r: f64, n: usize, z_value: f64) -> Option<(f64, f64)> {
    pearson_confidence_interval_fisher_z(r, n, z_value)
}

fn best_lag_search(
    focus: &HashMap<i64, f64>,
    candidate: &HashMap<i64, f64>,
    interval_seconds: i64,
    lag_max_buckets: i64,
    coarse_steps: i64,
    min_overlap: usize,
) -> LagSearchResult {
    const TOP_K_CANDIDATES: usize = 6;
    const FULL_SWEEP_OPS_LIMIT: usize = 4_000_000;
    let mut tested: std::collections::HashSet<i64> = std::collections::HashSet::new();

    let max_lag_seconds = lag_max_buckets * interval_seconds;
    if max_lag_seconds <= 0 {
        return LagSearchResult {
            best: LagCandidate {
                lag_seconds: 0,
                score: pearson_from_maps(focus, candidate, 0),
                n: aligned_count(focus, candidate, 0),
            },
            m_lag: 1,
        };
    }

    let total_ops = (2 * lag_max_buckets + 1)
        .max(0)
        .saturating_mul(focus.len() as i64) as usize;
    if total_ops <= FULL_SWEEP_OPS_LIMIT {
        return exact_lag_search(
            focus,
            candidate,
            interval_seconds,
            lag_max_buckets,
            min_overlap,
        );
    }

    let mut best = evaluate_lag(focus, candidate, 0, min_overlap).unwrap_or(LagCandidate {
        lag_seconds: 0,
        score: 0.0,
        n: 0,
    });
    if best.n >= min_overlap {
        tested.insert(best.lag_seconds);
    }
    let mut top = Vec::new();
    if best.n >= min_overlap {
        push_top_k(&mut top, best.clone(), TOP_K_CANDIDATES);
    }

    let coarse_step_buckets =
        std::cmp::max(1, (2 * lag_max_buckets) / std::cmp::max(1, coarse_steps));
    let mut lag_bucket = -lag_max_buckets;
    while lag_bucket <= lag_max_buckets {
        let lag_seconds = lag_bucket * interval_seconds;
        if let Some(candidate_lag) = evaluate_lag(focus, candidate, lag_seconds, min_overlap) {
            tested.insert(candidate_lag.lag_seconds);
            if candidate_lag.score > best.score {
                best = candidate_lag.clone();
            }
            push_top_k(&mut top, candidate_lag, TOP_K_CANDIDATES);
        }
        lag_bucket += coarse_step_buckets;
    }

    let mut step = coarse_step_buckets;
    while step > 1 && !top.is_empty() {
        step = std::cmp::max(1, step / 2);
        let mut next = Vec::new();
        for seed in &top {
            let center_bucket =
                (seed.lag_seconds / interval_seconds).clamp(-lag_max_buckets, lag_max_buckets);
            let start = (center_bucket - step * 2).max(-lag_max_buckets);
            let end = (center_bucket + step * 2).min(lag_max_buckets);
            let mut lag_bucket = start;
            while lag_bucket <= end {
                let lag_seconds = lag_bucket * interval_seconds;
                if let Some(candidate_lag) =
                    evaluate_lag(focus, candidate, lag_seconds, min_overlap)
                {
                    tested.insert(candidate_lag.lag_seconds);
                    if candidate_lag.score > best.score {
                        best = candidate_lag.clone();
                    }
                    push_top_k(&mut next, candidate_lag, TOP_K_CANDIDATES);
                }
                lag_bucket += step;
            }
        }
        top = next;
    }

    let best_bucket =
        (best.lag_seconds / interval_seconds).clamp(-lag_max_buckets, lag_max_buckets);
    let finalize_range = std::cmp::max(1, coarse_step_buckets / 2);
    let start = (best_bucket - finalize_range).max(-lag_max_buckets);
    let end = (best_bucket + finalize_range).min(lag_max_buckets);
    for lag_bucket in start..=end {
        let lag_seconds = lag_bucket * interval_seconds;
        if let Some(candidate_lag) = evaluate_lag(focus, candidate, lag_seconds, min_overlap) {
            tested.insert(candidate_lag.lag_seconds);
            if candidate_lag.score > best.score {
                best = candidate_lag;
            }
        }
    }

    LagSearchResult {
        best,
        m_lag: tested.len().max(1),
    }
}

fn exact_lag_search(
    focus: &HashMap<i64, f64>,
    candidate: &HashMap<i64, f64>,
    interval_seconds: i64,
    lag_max_buckets: i64,
    min_overlap: usize,
) -> LagSearchResult {
    let mut tested: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut best = evaluate_lag(focus, candidate, 0, min_overlap).unwrap_or(LagCandidate {
        lag_seconds: 0,
        score: 0.0,
        n: 0,
    });
    if best.n >= min_overlap {
        tested.insert(best.lag_seconds);
    }
    let mut lag_bucket = -lag_max_buckets;
    while lag_bucket <= lag_max_buckets {
        let lag_seconds = lag_bucket * interval_seconds;
        if let Some(candidate_lag) = evaluate_lag(focus, candidate, lag_seconds, min_overlap) {
            tested.insert(candidate_lag.lag_seconds);
            if candidate_lag.score > best.score {
                best = candidate_lag;
            }
        }
        lag_bucket += 1;
    }
    LagSearchResult {
        best,
        m_lag: tested.len().max(1),
    }
}

fn evaluate_lag(
    focus: &HashMap<i64, f64>,
    candidate: &HashMap<i64, f64>,
    lag_seconds: i64,
    min_overlap: usize,
) -> Option<LagCandidate> {
    let n = aligned_count(focus, candidate, lag_seconds);
    if n < min_overlap {
        return None;
    }
    let score = pearson_from_maps(focus, candidate, lag_seconds).abs();
    Some(LagCandidate {
        lag_seconds,
        score,
        n,
    })
}

fn push_top_k(top: &mut Vec<LagCandidate>, candidate: LagCandidate, limit: usize) {
    if top
        .iter()
        .any(|existing| existing.lag_seconds == candidate.lag_seconds)
    {
        return;
    }
    top.push(candidate);
    top.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| b.n.cmp(&a.n)));
    if top.len() > limit {
        top.truncate(limit);
    }
}

fn choose_windows(horizon_seconds: i64, interval_seconds: i64) -> Vec<i64> {
    let horizon_seconds = horizon_seconds.max(0);
    let candidates = vec![
        300, 900, 3600, 21_600, 86_400, 604_800, 2_592_000, 7_776_000,
    ];
    let mut out = Vec::new();
    for window in candidates {
        if window <= 0 {
            continue;
        }
        if window > horizon_seconds {
            continue;
        }
        let points = window / interval_seconds.max(1);
        if points >= 6 {
            out.push(window);
        }
    }
    if out.is_empty() {
        out.push(std::cmp::max(interval_seconds * 10, 60));
    }
    out
}

fn extract_episodes_for_window(
    focus: &HashMap<i64, f64>,
    candidate: &HashMap<i64, f64>,
    lag_seconds: i64,
    interval_seconds: i64,
    window_seconds: i64,
    horizon_seconds: i64,
    threshold: f64,
) -> Option<Vec<TsseEpisodeV1>> {
    let window_points = (window_seconds / interval_seconds.max(1)).max(3) as usize;

    // Build aligned arrays in increasing timestamp order.
    let mut aligned_ts: Vec<i64> = focus.keys().copied().collect();
    aligned_ts.sort_unstable();
    let mut x: Vec<f64> = Vec::new();
    let mut y: Vec<f64> = Vec::new();
    let mut ts: Vec<i64> = Vec::new();
    for t in aligned_ts {
        let Some(xv) = focus.get(&t) else { continue };
        let Some(yv) = candidate.get(&(t + lag_seconds)) else {
            continue;
        };
        if !xv.is_finite() || !yv.is_finite() {
            continue;
        }
        ts.push(t);
        x.push(*xv);
        y.push(*yv);
    }
    if x.len() < window_points {
        return None;
    }

    let rolling = rolling_pearson(&ts, &x, &y, window_points);
    if rolling.is_empty() {
        return None;
    }

    let stride_points = std::cmp::max(1, window_points / 4);
    let mut episodes: Vec<TsseEpisodeV1> = Vec::new();

    let mut current: Vec<(i64, f64, usize)> = Vec::new();
    for (idx, (window_start_ts, r, n_points)) in rolling.into_iter().enumerate() {
        if idx % stride_points != 0 {
            continue;
        }
        let score = r.abs();
        if score >= threshold && n_points >= 3 {
            current.push((window_start_ts, r, n_points));
            continue;
        }
        if !current.is_empty() {
            episodes.push(build_episode(
                &current,
                window_seconds,
                lag_seconds,
                interval_seconds,
                horizon_seconds,
            ));
            current.clear();
        }
    }
    if !current.is_empty() {
        episodes.push(build_episode(
            &current,
            window_seconds,
            lag_seconds,
            interval_seconds,
            horizon_seconds,
        ));
    }

    if episodes.is_empty() {
        return None;
    }
    Some(episodes)
}

fn rolling_pearson(
    ts: &[i64],
    x: &[f64],
    y: &[f64],
    window_points: usize,
) -> Vec<(i64, f64, usize)> {
    let n = x.len();
    if n != y.len() || n != ts.len() || window_points < 3 || n < window_points {
        return vec![];
    }

    let w = window_points as f64;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_yy = 0.0;
    let mut sum_xy = 0.0;

    for idx in 0..window_points {
        let xv = x[idx];
        let yv = y[idx];
        sum_x += xv;
        sum_y += yv;
        sum_xx += xv * xv;
        sum_yy += yv * yv;
        sum_xy += xv * yv;
    }

    let mut out = Vec::new();
    let mut since_recalc = 0usize;
    for end in (window_points - 1)..n {
        if since_recalc >= 1000 {
            let window_start = end + 1 - window_points;
            sum_x = 0.0;
            sum_y = 0.0;
            sum_xx = 0.0;
            sum_yy = 0.0;
            sum_xy = 0.0;
            for idx in window_start..=end {
                let xv = x[idx];
                let yv = y[idx];
                sum_x += xv;
                sum_y += yv;
                sum_xx += xv * xv;
                sum_yy += yv * yv;
                sum_xy += xv * yv;
            }
            since_recalc = 0;
        }
        let denom_x = w * sum_xx - sum_x * sum_x;
        let denom_y = w * sum_yy - sum_y * sum_y;
        let r = if denom_x > 0.0 && denom_y > 0.0 {
            let num = w * sum_xy - sum_x * sum_y;
            (num / (denom_x * denom_y).sqrt()).max(-1.0).min(1.0)
        } else {
            f64::NAN
        };
        let window_start = end + 1 - window_points;
        let window_start_ts = ts[window_start];
        if r.is_finite() {
            out.push((window_start_ts, r, window_points));
        }

        let next = end + 1;
        if next >= n {
            break;
        }
        let remove_idx = end + 1 - window_points;
        let remove_x = x[remove_idx];
        let remove_y = y[remove_idx];
        sum_x -= remove_x;
        sum_y -= remove_y;
        sum_xx -= remove_x * remove_x;
        sum_yy -= remove_y * remove_y;
        sum_xy -= remove_x * remove_y;

        let add_x = x[next];
        let add_y = y[next];
        sum_x += add_x;
        sum_y += add_y;
        sum_xx += add_x * add_x;
        sum_yy += add_y * add_y;
        sum_xy += add_x * add_y;
        since_recalc += 1;
    }

    out
}

fn build_episode(
    windows: &[(i64, f64, usize)],
    window_seconds: i64,
    lag_seconds: i64,
    _interval_seconds: i64,
    horizon_seconds: i64,
) -> TsseEpisodeV1 {
    let start_ts = windows.first().map(|v| v.0).unwrap_or(0);
    let end_ts = windows
        .last()
        .map(|v| v.0 + window_seconds)
        .unwrap_or(start_ts + window_seconds);

    let scores: Vec<f64> = windows.iter().map(|(_, r, _)| r.abs()).collect();
    let score_peak = scores.iter().copied().fold(0.0_f64, |acc, v| acc.max(v));
    let score_mean = if scores.is_empty() {
        0.0
    } else {
        scores.iter().sum::<f64>() / scores.len() as f64
    };
    let lag_iqr_sec = 0;
    let coverage = if horizon_seconds > 0 {
        ((end_ts - start_ts) as f64 / horizon_seconds as f64)
            .max(0.0)
            .min(1.0)
    } else {
        0.0
    };

    TsseEpisodeV1 {
        start_ts: Utc
            .timestamp_opt(start_ts, 0)
            .single()
            .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
            .to_rfc3339(),
        end_ts: Utc
            .timestamp_opt(end_ts, 0)
            .single()
            .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
            .to_rfc3339(),
        window_sec: window_seconds,
        lag_sec: lag_seconds,
        lag_iqr_sec,
        score_mean,
        score_peak,
        coverage,
        num_points: windows.iter().map(|(_, _, n)| *n as u64).sum(),
    }
}

fn combine_episode_scores(episodes: &[TsseEpisodeV1]) -> f64 {
    let mut p = 1.0_f64;
    let mut count = 0usize;
    for ep in episodes.iter() {
        if count >= MAX_EPISODE_CONTRIB {
            break;
        }
        let score = ep.score_peak.max(0.0).min(1.0);
        // Only count episodes with peaks above the threshold to filter weak/noisy episodes.
        if score < EPISODE_CONTRIB_MIN_PEAK {
            continue;
        }
        // Weighted union of episode peaks (higher = more evidence).
        let contrib = score * EPISODE_CONTRIB_WEIGHT;
        p *= 1.0 - contrib;
        count += 1;
    }
    (1.0 - p).max(0.0).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::analysis::parquet_duckdb::MetricsBucketRow;
    use chrono::{DateTime, Utc};

    fn build_rows(
        start_ts: i64,
        interval: i64,
        values: &[f64],
        sensor_id: &str,
    ) -> Vec<MetricsBucketRow> {
        values
            .iter()
            .enumerate()
            .map(|(idx, value)| MetricsBucketRow {
                sensor_id: sensor_id.to_string(),
                bucket: Utc
                    .timestamp_opt(start_ts + interval * idx as i64, 0)
                    .single()
                    .unwrap(),
                value: *value,
                samples: 1,
            })
            .collect()
    }

    #[test]
    fn scoring_is_deterministic() {
        let interval = 60;
        let start_ts = 1_700_000_000;
        let mut focus_vals = Vec::new();
        let mut candidate_vals = Vec::new();
        for idx in 0usize..200 {
            let base = (idx % 2) as f64;
            focus_vals.push(base);
            let lagged = if idx == 0 {
                base
            } else {
                (idx.saturating_sub(1) % 2) as f64
            };
            candidate_vals.push(lagged);
        }

        let focus = build_rows(start_ts, interval, &focus_vals, "focus");
        let candidate = build_rows(start_ts, interval, &candidate_vals, "candidate");

        let params = ScoreParams {
            focus: focus.clone(),
            candidate: candidate.clone(),
            interval_seconds: interval,
            horizon_seconds: interval * 200,
            lag_max_seconds: interval * 3,
            ..ScoreParams::default()
        };

        let first = score_related_series(params.clone()).expect("score");
        let second = score_related_series(params).expect("score");

        assert_eq!(first.best_lag_seconds, second.best_lag_seconds);
        assert!((first.score - second.score).abs() < 1e-9);
        assert_eq!(first.episodes.len(), second.episodes.len());
    }

    #[test]
    fn episodes_cover_expected_window() {
        let interval = 60;
        let start_ts = 1_700_000_000;
        let mut focus_vals = vec![0.0; 50];
        let mut candidate_vals = vec![0.0; 50];
        for idx in 20..35 {
            focus_vals[idx] = 1.0;
            candidate_vals[idx] = 1.0;
        }
        let focus = build_rows(start_ts, interval, &focus_vals, "focus");
        let candidate = build_rows(start_ts, interval, &candidate_vals, "candidate");

        let scored = score_related_series(ScoreParams {
            focus,
            candidate,
            interval_seconds: interval,
            horizon_seconds: interval * 50,
            lag_max_seconds: 0,
            episode_threshold: 0.3,
            ..ScoreParams::default()
        })
        .expect("score");

        assert!(!scored.episodes.is_empty());
        let best = &scored.episodes[0];
        let best_start = DateTime::parse_from_rfc3339(&best.start_ts)
            .unwrap()
            .timestamp();
        let best_end = DateTime::parse_from_rfc3339(&best.end_ts)
            .unwrap()
            .timestamp();
        let mid_ts = start_ts + interval * 25;
        assert!(best_start <= mid_ts && best_end >= mid_ts);
    }

    #[test]
    fn lag_search_finds_peak_when_coarse_steps_skip() {
        let interval = 60;
        let start_ts = 1_700_000_000;
        let points = 300usize;
        let true_lag_buckets = 37;

        let mut seed: u64 = 0x1234_5678_9abc_def0;
        let mut next_val = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((seed >> 32) as u32) as f64 / (u32::MAX as f64);
            (v * 2.0) - 1.0
        };

        let mut focus_vals = Vec::with_capacity(points);
        for _ in 0..points {
            focus_vals.push(next_val());
        }

        let mut noise_seed: u64 = 0x0fed_cba9_8765_4321;
        let mut next_noise = || {
            noise_seed = noise_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((noise_seed >> 32) as u32) as f64 / (u32::MAX as f64);
            (v * 2.0) - 1.0
        };

        let mut candidate_vals = Vec::with_capacity(points);
        for idx in 0..points {
            if idx >= true_lag_buckets {
                candidate_vals.push(focus_vals[idx - true_lag_buckets]);
            } else {
                candidate_vals.push(next_noise());
            }
        }

        let focus = build_rows(start_ts, interval, &focus_vals, "focus");
        let candidate = build_rows(start_ts, interval, &candidate_vals, "candidate");
        let params = ScoreParams {
            focus,
            candidate,
            interval_seconds: interval,
            horizon_seconds: interval * points as i64,
            lag_max_seconds: interval * 60,
            ..ScoreParams::default()
        };

        let scored = score_related_series(params).expect("score");
        assert_eq!(scored.best_lag_seconds, interval * true_lag_buckets as i64);
    }

    #[test]
    fn lag_search_min_overlap_is_param_driven() {
        let interval_seconds = 60;

        // 3 aligned points only (at lag +60 seconds). This should be considered by lag search
        // when the caller sets `min_overlap=3` (instead of a hidden constant like 10).
        let focus: HashMap<i64, f64> = HashMap::from([(0, 1.0), (60, 2.0), (120, 3.0)]);
        let candidate: HashMap<i64, f64> = HashMap::from([(60, 1.0), (120, 2.0), (180, 3.0)]);

        let result = best_lag_search(&focus, &candidate, interval_seconds, 2, 10, 3);
        assert_eq!(result.best.lag_seconds, 60);
        assert_eq!(result.best.n, 3);
        assert!(result.best.score > 0.99);
        assert!(result.m_lag >= 1);
    }

    #[test]
    fn lag_selection_correction_can_flip_significance() {
        let interval = 60;
        let start_ts = 1_700_000_000;
        let points = 100usize;

        let mut focus_seed: u64 = 0x1111_2222_3333_4444;
        let mut next_focus = || {
            focus_seed = focus_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((focus_seed >> 32) as u32) as f64 / (u32::MAX as f64);
            (v * 2.0) - 1.0
        };

        let mut noise_seed: u64 = 0xaaaa_bbbb_cccc_dddd;
        let mut next_noise = || {
            noise_seed = noise_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((noise_seed >> 32) as u32) as f64 / (u32::MAX as f64);
            // Uniform noise in [-4, 4] to yield a modest but real correlation at lag=0.
            ((v * 2.0) - 1.0) * 4.0
        };

        let mut focus_vals = Vec::with_capacity(points);
        let mut candidate_vals = Vec::with_capacity(points);
        for _ in 0..points {
            let f = next_focus();
            focus_vals.push(f);
            candidate_vals.push(f + next_noise());
        }

        let focus = build_rows(start_ts, interval, &focus_vals, "focus");
        let candidate = build_rows(start_ts, interval, &candidate_vals, "candidate");

        let base = ScoreParams {
            focus: focus.clone(),
            candidate: candidate.clone(),
            interval_seconds: interval,
            horizon_seconds: interval * points as i64,
            min_significant_n: 30,
            significance_alpha: 0.05,
            ..ScoreParams::default()
        };

        let no_search = score_related_series(ScoreParams {
            lag_max_seconds: 0,
            ..base.clone()
        });
        assert!(
            no_search.is_some(),
            "expected lag=0 test to pass without lag search"
        );

        let with_search = score_related_series(ScoreParams {
            lag_max_seconds: interval * 5,
            ..base
        });
        assert!(
            with_search.is_none(),
            "expected lag-selection correction to reject when searching multiple lags"
        );
    }

    #[test]
    fn min_abs_r_gate_can_reject_even_when_p_is_significant() {
        let interval = 60;
        let start_ts = 1_700_000_000;
        let points = 600usize;

        let mut focus_seed: u64 = 0x2468_ace0_1357_9bdf;
        let mut noise_seed: u64 = 0x1234_abcd_5678_ef90;
        let mut next_focus = || {
            focus_seed = focus_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((focus_seed >> 32) as u32) as f64 / (u32::MAX as f64);
            (v * 2.0) - 1.0
        };
        let mut next_noise = || {
            noise_seed = noise_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((noise_seed >> 32) as u32) as f64 / (u32::MAX as f64);
            (v * 2.0) - 1.0
        };

        let mut focus_vals = Vec::with_capacity(points);
        let mut candidate_vals = Vec::with_capacity(points);
        for _ in 0..points {
            let f = next_focus();
            let noise = next_noise();
            focus_vals.push(f);
            // Moderate/strong but not near-1.0 correlation.
            candidate_vals.push(0.8 * f + 0.6 * noise);
        }

        let focus = build_rows(start_ts, interval, &focus_vals, "focus");
        let candidate = build_rows(start_ts, interval, &candidate_vals, "candidate");

        let permissive = score_related_series(ScoreParams {
            focus: focus.clone(),
            candidate: candidate.clone(),
            interval_seconds: interval,
            horizon_seconds: interval * points as i64,
            lag_max_seconds: 0,
            min_significant_n: 30,
            significance_alpha: 0.05,
            min_abs_r: 0.1,
            ..ScoreParams::default()
        });
        assert!(
            permissive.is_some(),
            "expected permissive min_abs_r to pass"
        );

        let strict = score_related_series(ScoreParams {
            focus,
            candidate,
            interval_seconds: interval,
            horizon_seconds: interval * points as i64,
            lag_max_seconds: 0,
            min_significant_n: 30,
            significance_alpha: 0.05,
            min_abs_r: 0.95,
            ..ScoreParams::default()
        });
        assert!(strict.is_none(), "expected strict min_abs_r to reject");
    }

    #[test]
    fn diurnal_lag_detection_flags_near_24h_multiples() {
        assert!(!is_diurnal_lag_seconds(0));
        assert!(is_diurnal_lag_seconds(DIURNAL_LAG_SECONDS));
        assert!(is_diurnal_lag_seconds(-DIURNAL_LAG_SECONDS));
        assert!(is_diurnal_lag_seconds(DIURNAL_LAG_SECONDS + DIURNAL_LAG_TOLERANCE_SECONDS));
        assert!(is_diurnal_lag_seconds(DIURNAL_LAG_SECONDS - DIURNAL_LAG_TOLERANCE_SECONDS));
        assert!(!is_diurnal_lag_seconds(
            DIURNAL_LAG_SECONDS + DIURNAL_LAG_TOLERANCE_SECONDS + 1
        ));
        assert!(!is_diurnal_lag_seconds(21_600));
    }

    fn build_segmented_series(points: usize, lag_buckets: usize) -> (Vec<f64>, Vec<f64>) {
        let mut focus_seed: u64 = 0x1357_9bdf_2468_ace0;
        let mut cand_seed: u64 = 0x2468_ace0_1357_9bdf;

        let next_val = |seed: &mut u64| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let v = ((*seed >> 32) as u32) as f64 / (u32::MAX as f64);
            (v * 2.0) - 1.0
        };

        let mut focus = Vec::with_capacity(points);
        let mut candidate = Vec::with_capacity(points);
        for _ in 0..points {
            focus.push(next_val(&mut focus_seed));
            candidate.push(next_val(&mut cand_seed));
        }

        // Two disjoint "episodes" in the overlap region; candidate matches focus at the given lag.
        let segments: &[(usize, usize)] = &[(100, 140), (400, 440)];
        for (start, end) in segments {
            for idx in *start..*end {
                let shifted = idx.saturating_add(lag_buckets);
                if shifted < points {
                    candidate[shifted] = focus[idx];
                }
            }
        }

        (focus, candidate)
    }

    #[test]
    fn diurnal_lag_penalty_blocks_multi_episode_bonus_and_bounds_score() {
        let interval = 3600;
        let start_ts = 1_700_000_000;
        let points = 720usize; // 30 days of hourly data
        let lag_buckets = 24usize; // 24h

        let (focus_vals, candidate_vals) = build_segmented_series(points, lag_buckets);
        let focus = build_rows(start_ts, interval, &focus_vals, "focus");
        let candidate = build_rows(start_ts, interval, &candidate_vals, "candidate");

        let scored = score_related_series(ScoreParams {
            focus,
            candidate,
            interval_seconds: interval,
            horizon_seconds: interval * points as i64,
            lag_max_seconds: interval * lag_buckets as i64,
            episode_threshold: 0.6,
            min_significant_n: 50,
            significance_alpha: 0.5,
            min_abs_r: 0.0,
            ..ScoreParams::default()
        })
        .expect("score");

        assert_eq!(scored.best_lag_seconds, interval * lag_buckets as i64);
        assert!(scored.penalties.iter().any(|p| p == "diurnal_lag"));
        assert!(
            scored.score_components.get("is_diurnal_lag").copied().unwrap_or(0.0) >= 1.0
        );

        let strong_episode_count = scored
            .episodes
            .iter()
            .filter(|ep| ep.score_peak >= MULTI_EPISODE_MIN_PEAK)
            .count();
        assert!(
            strong_episode_count >= 2,
            "expected >=2 strong episodes; got {strong_episode_count}"
        );
        assert!(
            !scored.bonuses.iter().any(|b| b == "multi_episode"),
            "expected multi_episode bonus to be gated for diurnal lag"
        );

        assert!(
            scored.score_components.contains_key("lag_signal_factor"),
            "expected explainability score component"
        );
        assert!(
            scored.score_components
                .contains_key("diurnal_penalty_multiplier"),
            "expected explainability score component"
        );

        // Previously, diurnal + episodic matches could climb near 1.0; ensure we down-rank.
        assert!(scored.score < 0.35, "expected bounded score; got {}", scored.score);
    }

    #[test]
    fn non_diurnal_can_receive_multi_episode_bonus() {
        let interval = 3600;
        let start_ts = 1_700_000_000;
        let points = 720usize;
        let lag_buckets = 6usize; // 6h

        let (focus_vals, candidate_vals) = build_segmented_series(points, lag_buckets);
        let focus = build_rows(start_ts, interval, &focus_vals, "focus");
        let candidate = build_rows(start_ts, interval, &candidate_vals, "candidate");

        let scored = score_related_series(ScoreParams {
            focus,
            candidate,
            interval_seconds: interval,
            horizon_seconds: interval * points as i64,
            lag_max_seconds: interval * lag_buckets as i64,
            episode_threshold: 0.6,
            min_significant_n: 50,
            significance_alpha: 0.5,
            min_abs_r: 0.0,
            ..ScoreParams::default()
        })
        .expect("score");

        assert_eq!(scored.best_lag_seconds, interval * lag_buckets as i64);
        assert!(!scored.penalties.iter().any(|p| p == "diurnal_lag"));
        assert!(
            scored.score_components.get("is_diurnal_lag").copied().unwrap_or(0.0) == 0.0
        );

        let strong_episode_count = scored
            .episodes
            .iter()
            .filter(|ep| ep.score_peak >= MULTI_EPISODE_MIN_PEAK)
            .count();
        assert!(
            strong_episode_count >= 2,
            "expected >=2 strong episodes; got {strong_episode_count}"
        );
        assert!(
            scored.bonuses.iter().any(|b| b == "multi_episode"),
            "expected multi_episode bonus for non-diurnal lag"
        );
    }
}
