/// Benjamini–Hochberg false-discovery rate correction.
///
/// Input: `(key, p)` pairs where `p` is a raw p-value in `[0, 1]`.
/// Output: a mapping from `key -> q_value` (BH-adjusted p-value) in `[0, 1]`.
///
/// Notes:
/// - This assumes the usual BH conditions (independence or certain positive-dependence
///   structures). In time-series settings this is an approximation, but it is still
///   materially better than treating many pairwise tests as if they were single tests.
pub fn bh_fdr_q_values(pairs: &[(usize, f64)]) -> Vec<(usize, f64)> {
    if pairs.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<(usize, f64)> = pairs
        .iter()
        .copied()
        .filter(|(_, p)| p.is_finite() && *p >= 0.0 && *p <= 1.0)
        .collect();
    if sorted.is_empty() {
        return Vec::new();
    }

    sorted.sort_by(|a, b| a.1.total_cmp(&b.1));
    let m = sorted.len() as f64;

    // First pass: q_i = p_i * m / rank
    let mut q: Vec<(usize, f64)> = Vec::with_capacity(sorted.len());
    for (idx, (key, p)) in sorted.iter().copied().enumerate() {
        let rank = (idx + 1) as f64;
        let raw = (p * m / rank).max(0.0).min(1.0);
        q.push((key, raw));
    }

    // Enforce monotonicity: q_i = min(q_i, q_{i+1}) from the end.
    for idx in (0..q.len().saturating_sub(1)).rev() {
        let next = q[idx + 1].1;
        if q[idx].1 > next {
            q[idx].1 = next;
        }
    }

    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bh_fdr_produces_expected_q_values_for_simple_case() {
        let input = vec![(0, 0.01), (1, 0.02), (2, 0.5)];
        let mut out = bh_fdr_q_values(&input);
        out.sort_by_key(|(k, _)| *k);
        let q0 = out[0].1;
        let q1 = out[1].1;
        let q2 = out[2].1;
        // With m=3:
        // p=0.01 => 0.03
        // p=0.02 => 0.03 (after monotone adjustment)
        // p=0.5  => 0.5
        assert!((q0 - 0.03).abs() < 1e-12);
        assert!((q1 - 0.03).abs() < 1e-12);
        assert!((q2 - 0.5).abs() < 1e-12);
    }

    #[test]
    fn bh_fdr_ignores_invalid_p_values() {
        let input = vec![(0, -0.1), (1, f64::NAN), (2, 1.2), (3, 0.2)];
        let out = bh_fdr_q_values(&input);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 3);
        assert!(out[0].1 >= 0.2);
    }
}
