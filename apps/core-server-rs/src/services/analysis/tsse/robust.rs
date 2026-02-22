pub fn median(values: &mut [f64]) -> Option<f64> {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return None;
    }
    let mut sorted = finite;
    sorted.sort_by(|a, b| a.total_cmp(b));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Some(sorted[mid])
    } else {
        Some((sorted[mid - 1] + sorted[mid]) / 2.0)
    }
}

pub fn quantile(values: &mut [f64], q: f64) -> Option<f64> {
    if !(0.0..=1.0).contains(&q) {
        return None;
    }
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return None;
    }
    let mut sorted = finite;
    sorted.sort_by(|a, b| a.total_cmp(b));
    if sorted.len() == 1 {
        return Some(sorted[0]);
    }
    let pos = q * (sorted.len() as f64 - 1.0);
    let idx = pos.floor() as usize;
    let frac = pos - idx as f64;
    let a = sorted[idx];
    let b = sorted[(idx + 1).min(sorted.len() - 1)];
    Some(a + (b - a) * frac)
}

pub fn mad(values: &mut [f64], center: f64) -> Option<f64> {
    let mut deviations: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| (v - center).abs())
        .collect();
    if deviations.is_empty() {
        return None;
    }
    deviations.sort_by(|a, b| a.total_cmp(b));
    let mid = deviations.len() / 2;
    let med = if deviations.len() % 2 == 1 {
        deviations[mid]
    } else {
        (deviations[mid - 1] + deviations[mid]) / 2.0
    };
    Some(med)
}

pub fn robust_scale(values: &mut [f64]) -> Option<(f64, f64)> {
    if values.len() < 3 {
        return None;
    }
    let center = median(values)?;
    let mad = mad(values, center)?;
    let epsilon = 1e-9;
    if mad > epsilon {
        // MAD -> std-like scale under Normal
        return Some((center, mad * 1.4826));
    }

    // Degenerate case: fall back to IQR/1.349.
    let p25 = quantile(values, 0.25)?;
    let p75 = quantile(values, 0.75)?;
    let iqr = (p75 - p25).abs();
    if iqr > epsilon {
        return Some((center, iqr / 1.349));
    }
    Some((center, 1.0))
}

pub fn winsorize_in_place(values: &mut [f64], clip: f64) {
    let clip = clip.abs();
    if !clip.is_finite() || clip == 0.0 {
        return;
    }
    for v in values.iter_mut() {
        if !v.is_finite() {
            continue;
        }
        if *v > clip {
            *v = clip;
        } else if *v < -clip {
            *v = -clip;
        }
    }
}

pub fn zscore_robust(values: &[f64], clip: f64) -> Option<Vec<f64>> {
    let mut scratch = values.to_vec();
    let (center, scale) = robust_scale(&mut scratch)?;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let mut out = Vec::with_capacity(values.len());
    for v in values {
        if !v.is_finite() {
            out.push(f64::NAN);
            continue;
        }
        let z = (*v - center) / scale;
        let mut z = if z.is_finite() { z } else { f64::NAN };
        if clip.is_finite() && clip > 0.0 && z.is_finite() {
            if z > clip {
                z = clip;
            } else if z < -clip {
                z = -clip;
            }
        }
        out.push(z);
    }
    Some(out)
}
