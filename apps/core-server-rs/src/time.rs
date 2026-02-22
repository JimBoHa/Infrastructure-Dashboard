use chrono::{DateTime, Duration, NaiveDateTime, Offset, TimeZone, Utc};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedBlockInterval {
    pub(crate) start_utc: DateTime<Utc>,
    pub(crate) end_utc: DateTime<Utc>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct Candidate {
    utc: DateTime<Utc>,
    warnings: Vec<String>,
}

pub(crate) fn resolve_block_interval<Tz: TimeZone>(
    tz: &Tz,
    start_local: NaiveDateTime,
    end_local: NaiveDateTime,
) -> Result<ResolvedBlockInterval, String>
where
    Tz::Offset: Send + Sync,
{
    if end_local <= start_local {
        return Err("end_local must be after start_local".to_string());
    }

    let expected = end_local - start_local;
    let start_candidates = local_datetime_candidates(tz, start_local)?;
    let end_candidates = local_datetime_candidates(tz, end_local)?;

    let mut best: Option<(Candidate, Candidate, i64)> = None;
    for start in &start_candidates {
        for end in &end_candidates {
            let duration = end.utc - start.utc;
            if duration <= Duration::zero() {
                continue;
            }

            let diff = (duration - expected).num_seconds().abs();
            match &best {
                None => best = Some((start.clone(), end.clone(), diff)),
                Some((best_start, best_end, best_diff)) => {
                    if diff < *best_diff
                        || (diff == *best_diff
                            && (start.utc, end.utc) < (best_start.utc, best_end.utc))
                    {
                        best = Some((start.clone(), end.clone(), diff));
                    }
                }
            }
        }
    }

    let Some((start, end, diff_seconds)) = best else {
        return Err(format!(
            "unable to resolve local interval start={start_local} end={end_local}"
        ));
    };

    let mut warnings: Vec<String> = Vec::new();
    warnings.extend(start.warnings);
    warnings.extend(end.warnings);
    if diff_seconds != 0 {
        let duration = end.utc - start.utc;
        warnings.push(format!(
            "Resolved interval duration differs from wall-clock duration: start={start_local} end={end_local} expected={}s got={}s",
            expected.num_seconds(),
            duration.num_seconds()
        ));
    }

    Ok(ResolvedBlockInterval {
        start_utc: start.utc,
        end_utc: end.utc,
        warnings,
    })
}

fn local_datetime_candidates<Tz: TimeZone>(
    tz: &Tz,
    naive: NaiveDateTime,
) -> Result<Vec<Candidate>, String>
where
    Tz::Offset: Send + Sync,
{
    match tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => Ok(vec![Candidate {
            utc: dt.with_timezone(&Utc),
            warnings: vec![],
        }]),
        chrono::LocalResult::Ambiguous(a, b) => {
            let (earlier, later) = if a.with_timezone(&Utc) <= b.with_timezone(&Utc) {
                (a, b)
            } else {
                (b, a)
            };
            Ok(vec![
                Candidate {
                    utc: earlier.with_timezone(&Utc),
                    warnings: vec![format!(
                        "Ambiguous local datetime {naive} resolved to earlier instance {}",
                        earlier.to_rfc3339()
                    )],
                },
                Candidate {
                    utc: later.with_timezone(&Utc),
                    warnings: vec![format!(
                        "Ambiguous local datetime {naive} resolved to later instance {}",
                        later.to_rfc3339()
                    )],
                },
            ])
        }
        chrono::LocalResult::None => gap_datetime_candidates(tz, naive),
    }
}

fn gap_datetime_candidates<Tz: TimeZone>(
    tz: &Tz,
    naive: NaiveDateTime,
) -> Result<Vec<Candidate>, String>
where
    Tz::Offset: Send + Sync,
{
    const SEARCH_MINUTES: i64 = 180;

    let next_valid = find_next_valid_local(tz, naive, SEARCH_MINUTES)
        .ok_or_else(|| format!("no valid local datetime found after {naive}"))?;
    let prev_valid = find_prev_valid_local(tz, naive, SEARCH_MINUTES)
        .ok_or_else(|| format!("no valid local datetime found before {naive}"))?;

    let next_valid_minutes = (next_valid.naive_local() - naive).num_minutes();
    let next_valid_utc = next_valid.with_timezone(&Utc);

    let prev_offset = prev_valid.offset().fix();
    let prev_assumed = prev_offset
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| "fixed offset unexpectedly returned non-single datetime".to_string())?;
    let prev_assumed_utc = prev_assumed.with_timezone(&Utc);

    let next_offset = next_valid.offset().fix();
    let next_assumed = next_offset
        .from_local_datetime(&naive)
        .single()
        .ok_or_else(|| "fixed offset unexpectedly returned non-single datetime".to_string())?;
    let next_assumed_utc = next_assumed.with_timezone(&Utc);

    let mut candidates: Vec<Candidate> = vec![
        Candidate {
            utc: next_valid_utc,
            warnings: vec![format!(
                "Nonexistent local datetime {naive} resolved to next valid local time {} (shift +{next_valid_minutes}m)",
                next_valid.to_rfc3339()
            )],
        },
        Candidate {
            utc: prev_assumed_utc,
            warnings: vec![format!(
                "Nonexistent local datetime {naive} resolved by assuming pre-transition offset {prev_offset} (utc={}, local={})",
                prev_assumed_utc.to_rfc3339(),
                prev_assumed_utc.with_timezone(tz).to_rfc3339()
            )],
        },
        Candidate {
            utc: next_assumed_utc,
            warnings: vec![format!(
                "Nonexistent local datetime {naive} resolved by assuming post-transition offset {next_offset} (utc={}, local={})",
                next_assumed_utc.to_rfc3339(),
                next_assumed_utc.with_timezone(tz).to_rfc3339()
            )],
        },
    ];

    candidates.sort_by(|a, b| a.utc.cmp(&b.utc));
    candidates.dedup_by(|a, b| a.utc == b.utc);
    Ok(candidates)
}

fn find_next_valid_local<Tz: TimeZone>(
    tz: &Tz,
    naive: NaiveDateTime,
    max_minutes: i64,
) -> Option<DateTime<Tz>>
where
    Tz::Offset: Send + Sync,
{
    for minutes in 0..=max_minutes {
        let candidate = naive + Duration::minutes(minutes);
        match tz.from_local_datetime(&candidate) {
            chrono::LocalResult::Single(dt) => return Some(dt),
            chrono::LocalResult::Ambiguous(a, b) => {
                return Some(if a.with_timezone(&Utc) <= b.with_timezone(&Utc) {
                    a
                } else {
                    b
                })
            }
            chrono::LocalResult::None => continue,
        }
    }
    None
}

fn find_prev_valid_local<Tz: TimeZone>(
    tz: &Tz,
    naive: NaiveDateTime,
    max_minutes: i64,
) -> Option<DateTime<Tz>>
where
    Tz::Offset: Send + Sync,
{
    for minutes in 0..=max_minutes {
        let candidate = naive - Duration::minutes(minutes);
        match tz.from_local_datetime(&candidate) {
            chrono::LocalResult::Single(dt) => return Some(dt),
            chrono::LocalResult::Ambiguous(a, b) => {
                return Some(if a.with_timezone(&Utc) >= b.with_timezone(&Utc) {
                    a
                } else {
                    b
                })
            }
            chrono::LocalResult::None => continue,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn resolves_dst_gap_using_fixed_timezone() {
        let tz = chrono_tz::US::Eastern;
        let date = NaiveDate::from_ymd_opt(2026, 3, 8).expect("date");
        let start = date.and_hms_opt(1, 30, 0).expect("start");
        let end = date.and_hms_opt(2, 30, 0).expect("end");

        let resolved = resolve_block_interval(&tz, start, end).expect("resolve");
        assert_eq!(resolved.end_utc - resolved.start_utc, Duration::hours(1));
        assert!(resolved
            .warnings
            .iter()
            .any(|warning| warning.to_lowercase().contains("nonexistent")));
    }

    #[test]
    fn resolves_dst_ambiguity_using_fixed_timezone() {
        let tz = chrono_tz::US::Eastern;
        let date = NaiveDate::from_ymd_opt(2026, 11, 1).expect("date");
        let start = date.and_hms_opt(1, 30, 0).expect("start");
        let end = date.and_hms_opt(2, 30, 0).expect("end");

        let resolved = resolve_block_interval(&tz, start, end).expect("resolve");
        assert_eq!(resolved.end_utc - resolved.start_utc, Duration::hours(1));

        let expected_start = Utc
            .with_ymd_and_hms(2026, 11, 1, 6, 30, 0)
            .single()
            .expect("utc start");
        assert_eq!(resolved.start_utc, expected_start);
        assert!(resolved
            .warnings
            .iter()
            .any(|warning| warning.to_lowercase().contains("ambiguous")));
    }
}
