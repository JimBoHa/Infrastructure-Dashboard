use statrs::distribution::{ContinuousCDF, Normal, StudentsT};

/// Lag-1 autocorrelation estimate for a time-ordered sequence.
///
/// This treats the input as evenly spaced and uses a simple Pearson correlation between
/// `x[t-1]` and `x[t]`. For TSSE purposes this is an approximation, but it is cheap and
/// provides a useful "effective sample size" correction when series are autocorrelated.
pub fn lag1_autocorr(values: &[f64]) -> Option<f64> {
    if values.len() < 3 {
        return None;
    }
    let mut x_prev: Vec<f64> = Vec::with_capacity(values.len().saturating_sub(1));
    let mut x_curr: Vec<f64> = Vec::with_capacity(values.len().saturating_sub(1));
    for window in values.windows(2) {
        let a = window[0];
        let b = window[1];
        if !a.is_finite() || !b.is_finite() {
            continue;
        }
        x_prev.push(a);
        x_curr.push(b);
    }
    if x_prev.len() < 3 {
        return None;
    }

    let n = x_prev.len() as f64;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_yy = 0.0;
    let mut sum_xy = 0.0;
    for (xv, yv) in x_prev.iter().zip(x_curr.iter()) {
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
    Some(r.max(-0.999_999_9).min(0.999_999_9))
}

/// Effective sample size adjustment for autocorrelation.
///
/// Decision (tracker Phase 3): apply a conservative lag-1 correction:
///   `n_eff = n / (1 + 2 * rho1_x * rho1_y)`, bounded to `[3, n]`.
///
/// To avoid "over-crediting" negative autocorrelation interactions (which could make
/// `n_eff > n`), this implementation clamps the denominator to at least 1.
pub fn effective_sample_size_lag1(n: usize, rho1_x: Option<f64>, rho1_y: Option<f64>) -> usize {
    if n < 3 {
        return n;
    }
    let (Some(rx), Some(ry)) = (rho1_x, rho1_y) else {
        return n;
    };
    if !rx.is_finite() || !ry.is_finite() {
        return n;
    }
    let denom = (1.0 + 2.0 * rx * ry).max(1.0);
    if !denom.is_finite() || denom <= 0.0 {
        return n;
    }
    let neff = ((n as f64) / denom).floor() as usize;
    neff.clamp(3, n)
}

pub fn pearson_p_value_fisher_z(r: f64, n: usize) -> Option<f64> {
    if n < 4 {
        return None;
    }
    if !r.is_finite() {
        return None;
    }
    let r = r.max(-0.999_999_9).min(0.999_999_9);
    if r.abs() >= 0.999_999 {
        return Some(f64::MIN_POSITIVE); // Avoid exact 0.0 for display clarity
    }

    // Fisher z transform with normal approximation.
    let z = 0.5 * ((1.0 + r) / (1.0 - r)).ln();
    let se = 1.0 / ((n as f64) - 3.0).sqrt();
    if !se.is_finite() || se <= 0.0 {
        return None;
    }

    let normal = Normal::new(0.0, 1.0).ok()?;
    let z_score = (z / se).abs();
    // Use sf() (survival function) for numerical stability with large z-scores.
    // For z > ~8, 1.0 - cdf(z) underflows to 0.0 in f64, but sf() handles this.
    let p = 2.0 * normal.sf(z_score);
    // Clamp to MIN_POSITIVE to avoid displaying "0.000" for astronomically small p-values
    Some(p.max(f64::MIN_POSITIVE).min(1.0))
}

pub fn pearson_confidence_interval_fisher_z(r: f64, n: usize, z_value: f64) -> Option<(f64, f64)> {
    if n < 4 || !r.is_finite() || !z_value.is_finite() {
        return None;
    }
    let r = r.max(-0.999_999_9).min(0.999_999_9);
    let z = 0.5 * ((1.0 + r) / (1.0 - r)).ln();
    let se = 1.0 / ((n as f64) - 3.0).sqrt();
    if !se.is_finite() || se <= 0.0 {
        return None;
    }

    let low_z = z - z_value * se;
    let high_z = z + z_value * se;
    let low = low_z.tanh();
    let high = high_z.tanh();
    Some((low.max(-1.0).min(1.0), high.max(-1.0).min(1.0)))
}

pub fn z_value_for_alpha(alpha: f64) -> Option<f64> {
    if !(0.0 < alpha && alpha < 1.0) {
        return None;
    }
    let normal = Normal::new(0.0, 1.0).ok()?;
    let target = (1.0 - alpha / 2.0).min(1.0).max(0.0);
    Some(normal.inverse_cdf(target))
}

pub fn spearman_p_value_t_approx(r: f64, n: usize) -> Option<f64> {
    if n < 4 {
        return None;
    }
    if !r.is_finite() {
        return None;
    }
    let r = r.max(-0.999_999_9).min(0.999_999_9);
    let df = (n as f64) - 2.0;
    if df <= 0.0 {
        return None;
    }
    let denom = (1.0 - r * r).max(1e-12);
    let t = r * (df / denom).sqrt();
    let dist = StudentsT::new(0.0, 1.0, df).ok()?;
    let p = 2.0 * (1.0 - dist.cdf(t.abs()));
    Some(p.max(0.0).min(1.0))
}

pub fn spearman_confidence_interval_fisher_z_approx(
    r: f64,
    n: usize,
    z_value: f64,
) -> Option<(f64, f64)> {
    // Approximate via Fisher-z transform on Spearman's rho.
    pearson_confidence_interval_fisher_z(r, n, z_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_sample_size_lag1_is_bounded_and_non_increasing() {
        let n = 100;
        let neff = effective_sample_size_lag1(n, Some(0.9), Some(0.9));
        assert!(neff >= 3);
        assert!(neff <= n);

        let neff_none = effective_sample_size_lag1(n, None, Some(0.9));
        assert_eq!(neff_none, n);
    }

    #[test]
    fn neff_makes_p_values_less_extreme_for_same_r() {
        let n = 90;
        let rho = 0.95;
        let neff = effective_sample_size_lag1(n, Some(rho), Some(rho));
        assert!(neff < n);

        let r = 0.2;
        let p_n = pearson_p_value_fisher_z(r, n).unwrap();
        let p_neff = pearson_p_value_fisher_z(r, neff).unwrap();
        assert!(p_neff > p_n);
    }

    #[test]
    fn lag1_autocorr_returns_none_for_too_short_series() {
        assert_eq!(lag1_autocorr(&[1.0, 2.0]), None);
    }
}
