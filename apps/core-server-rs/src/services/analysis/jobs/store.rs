use chrono::Utc;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use uuid::Uuid;

fn job_key_hash_hex(job_key: &str) -> String {
    use sha2::Digest;
    use std::fmt::Write;
    let digest = sha2::Sha256::digest(job_key.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}

use super::types::{
    AnalysisJobCreateRequest, AnalysisJobError, AnalysisJobEventPublic, AnalysisJobProgress,
    AnalysisJobRow,
};

pub const JOB_STATUS_PENDING: &str = "pending";
pub const JOB_STATUS_RUNNING: &str = "running";
pub const JOB_STATUS_COMPLETED: &str = "completed";
pub const JOB_STATUS_FAILED: &str = "failed";
pub const JOB_STATUS_CANCELED: &str = "canceled";

pub async fn create_job(
    db: &PgPool,
    request: &AnalysisJobCreateRequest,
    created_by: Option<Uuid>,
) -> Result<(AnalysisJobRow, bool), sqlx::Error> {
    let dedupe_key = request
        .job_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|_| request.dedupe);

    let dedupe_hash = dedupe_key.map(job_key_hash_hex);
    if let Some(job_key_hash) = dedupe_hash.as_deref() {
        let existing: Option<AnalysisJobRow> = sqlx::query_as(
            r#"
            SELECT
                id, job_type, status, job_key, created_by, params, progress, error,
                created_at, updated_at, started_at, completed_at, cancel_requested_at, canceled_at, expires_at
            FROM analysis_jobs
            WHERE job_type = $1 AND job_key_hash = $2
            LIMIT 1
            "#,
        )
        .bind(request.job_type.trim())
        .bind(job_key_hash)
        .fetch_optional(db)
        .await?;
        if let Some(existing) = existing {
            // Dedupe is advisory (best-effort) unless a DB unique constraint exists.
            // If a matching job exists, return it instead of creating a new row.
            return Ok((existing, false));
        }
    }

    let job_id = Uuid::new_v4();
    let params = request.params.clone();
    let progress = AnalysisJobProgress::default();

    let inserted: Result<AnalysisJobRow, sqlx::Error> = sqlx::query_as(
        r#"
        INSERT INTO analysis_jobs (
            id, job_type, status, job_key, job_key_hash, created_by, params, progress, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now(), now())
        RETURNING
            id, job_type, status, job_key, created_by, params, progress, error,
            created_at, updated_at, started_at, completed_at, cancel_requested_at, canceled_at, expires_at
        "#,
    )
    .bind(job_id)
    .bind(request.job_type.trim())
    .bind(JOB_STATUS_PENDING)
    .bind(dedupe_key)
    .bind(dedupe_hash.as_deref())
    .bind(created_by)
    .bind(SqlJson(params))
    .bind(SqlJson(progress))
    .fetch_one(db)
    .await;

    match inserted {
        Ok(row) => {
            append_event(
                db,
                row.id,
                "created",
                serde_json::json!({
                    "job_type": row.job_type,
                    "job_key": row.job_key,
                }),
            )
            .await?;
            Ok((row, true))
        }
        Err(err) => {
            if let (Some(job_key_hash), sqlx::Error::Database(db_err)) =
                (dedupe_hash.as_deref(), &err)
            {
                if db_err.code().as_deref() == Some("23505") {
                    let existing: AnalysisJobRow = sqlx::query_as(
                        r#"
                        SELECT
                            id, job_type, status, job_key, created_by, params, progress, error,
                            created_at, updated_at, started_at, completed_at, cancel_requested_at, canceled_at, expires_at
                        FROM analysis_jobs
                        WHERE job_type = $1 AND job_key_hash = $2
                        LIMIT 1
                        "#,
                    )
                    .bind(request.job_type.trim())
                    .bind(job_key_hash)
                    .fetch_one(db)
                    .await?;
                    return Ok((existing, false));
                }
            }
            Err(err)
        }
    }
}

pub async fn get_job(db: &PgPool, job_id: Uuid) -> Result<Option<AnalysisJobRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            id, job_type, status, job_key, created_by, params, progress, error,
            created_at, updated_at, started_at, completed_at, cancel_requested_at, canceled_at, expires_at
        FROM analysis_jobs
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(job_id)
    .fetch_optional(db)
    .await
}

pub async fn get_job_by_key(
    db: &PgPool,
    job_type: &str,
    job_key: &str,
) -> Result<Option<AnalysisJobRow>, sqlx::Error> {
    let job_key_hash = job_key_hash_hex(job_key.trim());
    sqlx::query_as(
        r#"
        SELECT
            id, job_type, status, job_key, created_by, params, progress, error,
            created_at, updated_at, started_at, completed_at, cancel_requested_at, canceled_at, expires_at
        FROM analysis_jobs
        WHERE job_type = $1 AND job_key_hash = $2
        LIMIT 1
        "#,
    )
    .bind(job_type.trim())
    .bind(job_key_hash)
    .fetch_optional(db)
    .await
}

pub async fn count_active_jobs_for_user(db: &PgPool, user_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM analysis_jobs
        WHERE created_by = $1
          AND status = ANY($2)
        "#,
    )
    .bind(user_id)
    .bind(vec![JOB_STATUS_PENDING, JOB_STATUS_RUNNING])
    .fetch_one(db)
    .await?;
    Ok(row.0)
}

pub async fn append_event(
    db: &PgPool,
    job_id: Uuid,
    kind: &str,
    payload: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO analysis_job_events (job_id, kind, payload, created_at)
        VALUES ($1, $2, $3, now())
        "#,
    )
    .bind(job_id)
    .bind(kind)
    .bind(SqlJson(payload))
    .execute(db)
    .await?;
    Ok(())
}

pub async fn list_events(
    db: &PgPool,
    job_id: Uuid,
    after: i64,
    limit: i64,
) -> Result<Vec<AnalysisJobEventPublic>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        kind: String,
        payload: SqlJson<serde_json::Value>,
        created_at: chrono::DateTime<Utc>,
    }

    let limit = limit.clamp(1, 500);
    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT id, kind, payload, created_at
        FROM analysis_job_events
        WHERE job_id = $1 AND id > $2
        ORDER BY id ASC
        LIMIT $3
        "#,
    )
    .bind(job_id)
    .bind(after)
    .bind(limit)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| AnalysisJobEventPublic {
            id: row.id,
            kind: row.kind,
            payload: row.payload.0,
            created_at: row.created_at.to_rfc3339(),
        })
        .collect())
}

pub async fn claim_next_pending(db: &PgPool) -> Result<Option<AnalysisJobRow>, sqlx::Error> {
    let mut tx = db.begin().await?;
    let claimed: Option<AnalysisJobRow> = sqlx::query_as(
        r#"
        WITH next AS (
            SELECT id
            FROM analysis_jobs
            WHERE status = $1
            ORDER BY created_at ASC
            LIMIT 1
            FOR UPDATE SKIP LOCKED
        )
        UPDATE analysis_jobs
        SET status = $2,
            started_at = now(),
            updated_at = now()
        WHERE id IN (SELECT id FROM next)
        RETURNING
            id, job_type, status, job_key, created_by, params, progress, error,
            created_at, updated_at, started_at, completed_at, cancel_requested_at, canceled_at, expires_at
        "#,
    )
    .bind(JOB_STATUS_PENDING)
    .bind(JOB_STATUS_RUNNING)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(job) = &claimed {
        sqlx::query(
            r#"
            INSERT INTO analysis_job_events (job_id, kind, payload, created_at)
            VALUES ($1, 'started', $2, now())
            "#,
        )
        .bind(job.id)
        .bind(SqlJson(serde_json::json!({})))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(claimed)
}

pub async fn update_progress(
    db: &PgPool,
    job_id: Uuid,
    progress: &AnalysisJobProgress,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE analysis_jobs
        SET progress = $2,
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(SqlJson(progress))
    .execute(db)
    .await?;
    append_event(
        db,
        job_id,
        "progress",
        serde_json::json!({
            "phase": progress.phase,
            "completed": progress.completed,
            "total": progress.total,
            "message": progress.message,
        }),
    )
    .await?;
    Ok(())
}

pub async fn mark_completed(
    db: &PgPool,
    job_id: Uuid,
    result: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO analysis_job_results (job_id, result, created_at)
        VALUES ($1, $2, now())
        ON CONFLICT (job_id)
        DO UPDATE SET result = EXCLUDED.result, created_at = now()
        "#,
    )
    .bind(job_id)
    .bind(SqlJson(result))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE analysis_jobs
        SET status = $2,
            completed_at = now(),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(JOB_STATUS_COMPLETED)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO analysis_job_events (job_id, kind, payload, created_at)
        VALUES ($1, 'completed', $2, now())
        "#,
    )
    .bind(job_id)
    .bind(SqlJson(serde_json::json!({})))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn mark_failed(
    db: &PgPool,
    job_id: Uuid,
    error: AnalysisJobError,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;

    sqlx::query(
        r#"
        UPDATE analysis_jobs
        SET status = $2,
            error = $3,
            completed_at = now(),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(JOB_STATUS_FAILED)
    .bind(SqlJson(error))
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO analysis_job_events (job_id, kind, payload, created_at)
        VALUES ($1, 'failed', $2, now())
        "#,
    )
    .bind(job_id)
    .bind(SqlJson(serde_json::json!({})))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn request_cancel(
    db: &PgPool,
    job_id: Uuid,
) -> Result<Option<AnalysisJobRow>, sqlx::Error> {
    let mut tx = db.begin().await?;

    let canceled_pending: Option<AnalysisJobRow> = sqlx::query_as(
        r#"
        UPDATE analysis_jobs
        SET status = $2,
            cancel_requested_at = now(),
            canceled_at = now(),
            updated_at = now(),
            progress = $3
        WHERE id = $1 AND status = $4
        RETURNING
            id, job_type, status, job_key, created_by, params, progress, error,
            created_at, updated_at, started_at, completed_at, cancel_requested_at, canceled_at, expires_at
        "#,
    )
    .bind(job_id)
    .bind(JOB_STATUS_CANCELED)
    .bind(SqlJson(AnalysisJobProgress {
        phase: "canceled".to_string(),
        completed: 0,
        total: None,
        message: Some("Canceled before start".to_string()),
    }))
    .bind(JOB_STATUS_PENDING)
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(job) = canceled_pending {
        sqlx::query(
            r#"
            INSERT INTO analysis_job_events (job_id, kind, payload, created_at)
            VALUES ($1, 'canceled', $2, now())
            "#,
        )
        .bind(job_id)
        .bind(SqlJson(serde_json::json!({ "before_start": true })))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        return Ok(Some(job));
    }

    let running_updated: Option<AnalysisJobRow> = sqlx::query_as(
        r#"
        UPDATE analysis_jobs
        SET cancel_requested_at = now(),
            updated_at = now()
        WHERE id = $1 AND status = $2
        RETURNING
            id, job_type, status, job_key, created_by, params, progress, error,
            created_at, updated_at, started_at, completed_at, cancel_requested_at, canceled_at, expires_at
        "#,
    )
    .bind(job_id)
    .bind(JOB_STATUS_RUNNING)
    .fetch_optional(&mut *tx)
    .await?;

    if running_updated.is_some() {
        sqlx::query(
            r#"
            INSERT INTO analysis_job_events (job_id, kind, payload, created_at)
            VALUES ($1, 'cancel_requested', $2, now())
            "#,
        )
        .bind(job_id)
        .bind(SqlJson(serde_json::json!({})))
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(running_updated)
}

pub async fn mark_canceled(db: &PgPool, job_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;

    sqlx::query(
        r#"
        UPDATE analysis_jobs
        SET status = $2,
            canceled_at = now(),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(JOB_STATUS_CANCELED)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO analysis_job_events (job_id, kind, payload, created_at)
        VALUES ($1, 'canceled', $2, now())
        "#,
    )
    .bind(job_id)
    .bind(SqlJson(serde_json::json!({ "before_start": false })))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn get_result(
    db: &PgPool,
    job_id: Uuid,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        result: SqlJson<serde_json::Value>,
    }
    let row: Option<Row> = sqlx::query_as(
        r#"
        SELECT result
        FROM analysis_job_results
        WHERE job_id = $1
        LIMIT 1
        "#,
    )
    .bind(job_id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| r.result.0))
}
