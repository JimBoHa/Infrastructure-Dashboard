Eliminate SQL Error Information Leakage

  Priority: P1 (Security)
  Status: To Do
  Estimated Effort: Medium (4-8 hours)

  Problem

  150+ instances of internal_error(err.to_string()) expose raw SQL error messages to API clients:

  // Current pattern (INSECURE):
  fn internal_error(err: impl std::fmt::Display) -> (StatusCode, String) {
      (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())  // Leaks: "duplicate key value violates unique constraint..."
  }

  This exposes:
  - Database schema details (table/column names)
  - Constraint violation details
  - Query structure hints

  Solution

  Create a centralized error wrapper that logs full details but returns generic messages:

  // apps/core-server-rs/src/error.rs
  pub fn internal_error(err: impl std::fmt::Display) -> (StatusCode, String) {
      // Log full error for debugging (server-side only)
      tracing::error!("Internal error: {}", err);

      // Return generic message to client
      (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
  }

  // Special handling for common cases:
  pub fn map_db_error(err: sqlx::Error) -> (StatusCode, String) {
      tracing::error!("Database error: {}", err);

      match &err {
          sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, "Resource not found".to_string()),
          sqlx::Error::Database(db) if db.is_unique_violation() => {
              (StatusCode::CONFLICT, "Resource already exists".to_string())
          }
          _ => (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()),
      }
  }

  Acceptance Criteria

  - Create centralized internal_error() in src/error.rs
  - Replace all 150+ route-local internal_error() functions
  - Add tracing::error! logging for full error details
  - Verify no SQL details in API responses (test with invalid data)
  - Add specific handlers for common cases (not found, conflict, etc.)

  Files to Modify

  | File                       | Instances |
  |----------------------------|-----------|
  | routes/sensors.rs          | 22        |
  | routes/outputs.rs          | 21        |
  | routes/weather_stations.rs | 16        |
  | routes/backups.rs          | 11        |
  | routes/nodes.rs            | 8         |
  | routes/users.rs            | 8         |
  | routes/discovery.rs        | 8         |
  | ... and 15 more files      | ~56       |

