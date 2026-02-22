⏺ Farm Dashboard - Security & Code Quality Audit Report

  Date: January 1, 2026
  Scope: READ-ONLY comprehensive audit
  Repository: farm_dashboard (repo root)

  NOTE (2026-02-12): Some repo paths referenced in this audit (legacy Python core-server, WAN portal scaffold)
  have since been removed as part of a repo-wide pruning pass (ARCH-6). Treat file-path references as historical.

  ---
  Executive Summary

  This audit identified ~80 distinct issues across error handling, configuration, and code organization. No critical security vulnerabilities were found - the .env file with API credentials is properly gitignored and not tracked in version control. The most significant concerns are around error handling robustness (mutex lock poisoning, unguarded panics) and plaintext SSH password handling in the deployment system.

  ---
  Issues Found & Recommendations

  🔴 HIGH Severity

  | Issue                                     | Count | Impact                                          |
  |-------------------------------------------|-------|-------------------------------------------------|
  | Mutex lock poisoning vulnerability        | 8     | One panic permanently breaks deployment service |
  | Unguarded .expect() in production code    | 14    | Panics crash the service on unexpected input    |
  | Plaintext SSH passwords in deployment API | 1     | Passwords logged, no key-based auth option      |
  | Default DB credentials as fallback        | 2     | postgres:postgres used if env var missing       |

  🟠 MEDIUM Severity

  | Issue                                    | Count | Impact                                            |
  |------------------------------------------|-------|---------------------------------------------------|
  | Hardcoded 127.0.0.1 addresses            | 22    | Difficult to deploy to non-localhost environments |
  | Silent data overwrites in config helpers | 9     | Data loss without warning if wrong type passed    |
  | No API rate limiting                     | —     | DoS vulnerability on deployment/config endpoints  |
  | E2E test failure unresolved              | 1     | na53-e2e-installer-stack-smoke failing            |
  | Hardcoded test password                  | 1     | Should use environment variable                   |

  🟡 LOW Severity

  | Issue                         | Count | Impact                                     |
  |-------------------------------|-------|--------------------------------------------|
  | Monolithic files (>600 lines) | 5     | Harder to test and maintain                |
  | .unwrap() in test code        | 21    | Acceptable but could use better assertions |
  | Pydantic v2 migration TODOs   | 50+   | Auto-generated code needs regeneration     |

  ✅ False Positives from Initial Audit

  | Initially Reported          | Actual Status                                  |
  |-----------------------------|------------------------------------------------|
  | Exposed credentials in .env | NOT an issue - file is gitignored, not tracked |
  | Missing Cargo.lock files    | NOT an issue - all 4 Rust crates have locks    |
  | Missing poetry.lock files   | NOT an issue - both Python apps have locks     |

  ---
  Top 10 Recommendations

  1. Replace mutex lock pattern - Use parking_lot::Mutex or implement poison recovery
  2. Convert .expect() to proper error handling - Use ? operator or match with logging
  3. Implement SSH key-based authentication - Remove password auth from deployment API
  4. Remove default DB credentials - Require explicit configuration, fail fast if missing
  5. Add rate limiting middleware - Use tower-governor or similar on public endpoints
  6. Externalize localhost defaults - Make bind addresses configurable without fallbacks
  7. Add logging to ensure_object/ensure_array - Warn when data type coercion occurs
  8. Fix failing E2E tests - Resolve na53-e2e-installer-stack-smoke before merging
  9. Refactor large files - Split deployments.rs (1,017 lines) into focused modules
  10. Move test password to env var - SmokeTest!123 should not be hardcoded

  ---
  Positive Observations

  - ✅ .env properly gitignored - credentials are local-only
  - ✅ All Cargo.lock and poetry.lock files committed for reproducible builds
  - ✅ Good separation of concerns across apps, infra, and tools
  - ✅ Comprehensive documentation and ADRs
  - ✅ Smart pre-commit hook with staged-path test selection
  - ✅ OpenTelemetry instrumentation throughout
  - ✅ Offline-first design well implemented
  - ✅ Active Rust migration with parity validation

  ---
  Supplemental Citation Report

  🔴 Mutex Lock Poisoning (8 occurrences)

  Risk: If any thread panics while holding these locks, all subsequent operations on that mutex will fail permanently until service restart.

  apps/core-server-rs/src/services/deployments.rs (7 occurrences)

  | Line | Code                                                                     |
  |------|--------------------------------------------------------------------------|
  | 166  | let mut store = self.jobs.lock().expect("deployment job lock poisoned"); |
  | 531  | let mut store = self.jobs.lock().expect("deployment job lock poisoned"); |
  | 549  | let mut store = self.jobs.lock().expect("deployment job lock poisoned"); |
  | 586  | let mut store = self.jobs.lock().expect("deployment job lock poisoned"); |
  | 603  | let mut store = self.jobs.lock().expect("deployment job lock poisoned"); |
  | 618  | let mut store = self.jobs.lock().expect("deployment job lock poisoned"); |
  | 630  | let mut store = self.jobs.lock().expect("deployment job lock poisoned"); |

  apps/farmctl/src/launchd.rs (1 occurrence)

  | Line | Code                                                  |
  |------|-------------------------------------------------------|
  | 574  | let _guard = lock.lock().expect("env lock poisoned"); |

  Recommendation: Replace with parking_lot::Mutex (no poisoning) or implement recovery:
  let mut store = match self.jobs.lock() {
      Ok(guard) => guard,
      Err(poisoned) => {
          tracing::warn!("Recovering from poisoned lock");
          poisoned.into_inner()
      }
  };

  ---
  🔴 Unguarded .expect() in Production Code (14 occurrences)

  apps/core-server-rs/src/presets.rs

  | Line | Code                                                | Risk                                    |
  |------|-----------------------------------------------------|-----------------------------------------|
  | 42   | .expect("invalid shared/presets/integrations.json") | Startup crash if preset file malformed  |
  | 68   | .expect("preset file must parse")                   | Same - test code but pattern propagates |

  apps/core-server-rs/src/routes/renogy.rs

  | Line | Code                              | Risk                      |
  |------|-----------------------------------|---------------------------|
  | 99   | .expect("value forced to object") | Logic error causes panic  |
  | 106  | .expect("value forced to array")  | Logic error causes panic  |
  | 256  | .expect("index exists")           | Index out of bounds panic |

  apps/core-server-rs/src/routes/display_profiles.rs

  | Line | Code                              | Risk                     |
  |------|-----------------------------------|--------------------------|
  | 142  | .expect("value forced to object") | Logic error causes panic |

  apps/core-server-rs/src/routes/analytics.rs

  | Line | Code                       | Risk                            |
  |------|----------------------------|---------------------------------|
  | 337  | .expect("expected sensor") | Missing sensor crashes endpoint |

  apps/farmctl/src/native_deps.rs

  | Line | Code                           | Risk                    |
  |------|--------------------------------|-------------------------|
  | 150  | .expect("current_dir")         | Fails if CWD deleted    |
  | 152  | .expect("resolve_output_root") | Path resolution failure |
  | 160  | .expect("current_dir")         | Fails if CWD deleted    |
  | 162  | .expect("resolve_output_root") | Path resolution failure |

  apps/wan-portal/src/state.rs

  | Line | Code                                   | Risk                     |
  |------|----------------------------------------|--------------------------|
  | 21   | .expect("reqwest client should build") | TLS/system config issues |

  ---
  🔴 Plaintext SSH Password Handling

  File: apps/core-server-rs/src/services/deployments.rs

  | Line | Code                                                     | Issue                                    |
  |------|----------------------------------------------------------|------------------------------------------|
  | 86   | pub password: String                                     | Plaintext password in API request struct |
  | 91   | pub mqtt_password: Option<String>                        | MQTT password also plaintext             |
  | 346  | &request.password                                        | Password passed through call chain       |
  | 365  | &request.password                                        | Password passed through call chain       |
  | 375  | &request.password                                        | Password passed through call chain       |
  | 388  | &request.password                                        | Password passed through call chain       |
  | 482  | .userauth_password(&request.username, &request.password) | SSH password auth                        |
  | 515  | password: &str                                           | Function accepts plaintext password      |
  | 520  | run_sudo(session, password, command, timeout)            | Password passed to sudo                  |
  | 933  | fn run_sudo(..., password: &str, ...)                    | Sudo function definition                 |
  | 941  | .write_all(format!("{password}\n").as_bytes())           | Password written to stdin                |

  Recommendation: Implement SSH key-based authentication, remove password option, or at minimum use a secure string type that prevents accidental logging.

  ---
  🔴 Default Database Credentials (2 occurrences)

  | File                              | Line | Value                                                       |
  |-----------------------------------|------|-------------------------------------------------------------|
  | apps/core-server-rs/src/config.rs | 31   | "postgresql+psycopg://postgres:postgres@127.0.0.1:5432/iot" |
  | apps/farmctl/src/config.rs        | 56   | "postgresql+psycopg://postgres:postgres@127.0.0.1:5432/iot" |

  Risk: If CORE_DATABASE_URL env var is unset, production connects with default postgres:postgres credentials.

  Recommendation: Remove default, require explicit configuration:
  let database_url = env::var("CORE_DATABASE_URL")
      .expect("CORE_DATABASE_URL must be set");

  ---
  🟠 Hardcoded 127.0.0.1 Addresses (22 occurrences)

  apps/core-server-rs/src/

  | File                    | Line | Context                                   |
  |-------------------------|------|-------------------------------------------|
  | cli.rs                  | 7    | #[arg(long, default_value = "127.0.0.1")] |
  | config.rs               | 31   | Database URL default                      |
  | config.rs               | 33   | env_string("CORE_MQTT_HOST", "127.0.0.1") |
  | services/deployments.rs | 404  | curl -sf http://127.0.0.1:{}/healthz      |
  | services/deployments.rs | 812  | curl -sf http://127.0.0.1:{port}/healthz  |

  apps/farmctl/src/

  | File         | Line | Context                                              |
  |--------------|------|------------------------------------------------------|
  | cli.rs       | 195  | #[arg(long, default_value = "127.0.0.1")]            |
  | config.rs    | 48   | "127.0.0.1".to_string()                              |
  | config.rs    | 56   | Database URL default                                 |
  | constants.rs | 2    | pub const DEFAULT_SETUP_HOST: &str = "127.0.0.1"     |
  | health.rs    | 26   | http://127.0.0.1:{}/healthz                          |
  | health.rs    | 27   | http://127.0.0.1:{}/                                 |
  | health.rs    | 83   | TcpStream::connect(("127.0.0.1", config.redis_port)) |
  | native.rs    | 42   | bind 127.0.0.1 (Redis config template)               |
  | native.rs    | 49   | listener {} 127.0.0.1 (Mosquitto config)             |
  | utils.rs     | 38   | TcpListener::bind(("127.0.0.1", port))               |
  | utils.rs     | 42   | TcpListener::bind(("127.0.0.1", 0))                  |
  | server.rs    | 62   | HeaderValue::from_static("http://127.0.0.1:3000")    |
  | server.rs    | 63   | HeaderValue::from_static("http://127.0.0.1:3005")    |
  | launchd.rs   | 156  | "127.0.0.1".to_string()                              |

  apps/telemetry-sidecar/src/

  | File      | Line | Context                                     |
  |-----------|------|---------------------------------------------|
  | config.rs | 40   | unwrap_or_else(|_| "127.0.0.1".to_string()) |

  apps/wan-portal/src/

  | File     | Line | Context                          |
  |----------|------|----------------------------------|
  | proxy.rs | 191  | TcpListener::bind("127.0.0.1:0") |
  | proxy.rs | 214  | listen: "127.0.0.1:0".parse()    |

  ---
  🟠 Silent Data Overwrites (9 occurrences)

  These helper functions silently replace data of the wrong type with empty collections, causing silent data loss.

  apps/core-server-rs/src/routes/renogy.rs

  | Line    | Code                        | Issue                         |
  |---------|-----------------------------|-------------------------------|
  | 95-100  | fn ensure_object(...)       | Overwrites non-object with {} |
  | 102-107 | fn ensure_array(...)        | Overwrites non-array with []  |
  | 181     | ensure_object(config)       | Caller                        |
  | 185     | ensure_object(renogy)       | Caller                        |
  | 241     | ensure_object(config)       | Caller                        |
  | 245     | ensure_array(sensors_value) | Caller                        |
  | 257     | ensure_object(sensor)       | Caller                        |
  | 344     | ensure_object(config)       | Caller                        |
  | 348     | ensure_array(sensors_value) | Caller                        |

  apps/core-server-rs/src/routes/display_profiles.rs

  | Line    | Code                       | Issue        |
  |---------|----------------------------|--------------|
  | 138-143 | fn ensure_object(...)      | Same pattern |
  | 227     | ensure_object(&mut config) | Caller       |

  Recommendation: Log a warning when type coercion occurs:
  fn ensure_object(value: &mut JsonValue) -> &mut Map<String, JsonValue> {
      if !value.is_object() {
          tracing::warn!("Coercing non-object value to empty object");
          *value = JsonValue::Object(Map::new());
      }
      value.as_object_mut().expect("value forced to object")
  }

  ---
  🟠 .unwrap() in Test Code (21 occurrences)

  These are in test modules and are acceptable, but noted for completeness.

  apps/wan-portal/src/proxy.rs (tests)

  | Line                         | Code                                            |
  |------------------------------|-------------------------------------------------|
  | 191                          | TcpListener::bind("127.0.0.1:0").await.unwrap() |
  | 192                          | listener.local_addr().unwrap()                  |
  | 196                          | .unwrap()                                       |
  | 214                          | "127.0.0.1:0".parse().unwrap()                  |
  | 226                          | PortalConfig::from_args(args).unwrap()          |
  | 237, 240, 251, 254, 264, 267 | Various .unwrap() in test assertions            |

  apps/farmctl/src/launchd.rs (tests)

  | Line | Code                                  |
  |------|---------------------------------------|
  | 589  | tempfile::tempdir().unwrap()          |
  | 599  | generate_plan(&config).unwrap()       |
  | 604  | plist::Value::from_file(...).unwrap() |
  | 605  | value.as_dictionary().unwrap()        |
  | 621  | tempfile::tempdir().unwrap()          |
  | 629  | generate_plan(&config).unwrap()       |
  | 634  | plist::Value::from_file(...).unwrap() |
  | 635  | value.as_dictionary().unwrap()        |

  apps/core-server-rs/src/routes/analytics.rs (tests)

  | Line | Code                                                |
  |------|-----------------------------------------------------|
  | 343  | Utc.with_ymd_and_hms(2025, 1, 2, 0, 30, 0).unwrap() |
  | 351  | series.last().unwrap()                              |

  apps/farmctl/src/ (other)

  | File       | Line | Code                                    |
  |------------|------|-----------------------------------------|
  | server.rs  | 301  | content_type.parse().unwrap()           |
  | config.rs  | 534  | normalize_config(...).unwrap() (test)   |
  | netboot.rs | 171  | validate_http_path(...).unwrap() (test) |

  ---
  🟠 E2E Test Failure

  | File                                                       | Status |
  |------------------------------------------------------------|--------|
  | reports/na53-e2e-installer-stack-smoke-20260101_155213.log | FAIL   |

  e2e-installer-stack-smoke: FAIL (e2e_setup_smoke failed)
  Artifacts: reports/e2e-installer-stack-smoke/20260101_235213/

  Recommendation: Investigate and fix before committing related changes.

  ---
  🟠 Hardcoded Test Password

  | File                   | Line | Value                      |
  |------------------------|------|----------------------------|
  | tools/e2e_ios_smoke.py | 918  | password = "SmokeTest!123" |

  Recommendation: Use environment variable: os.environ.get("E2E_TEST_PASSWORD", "SmokeTest!123")

  ---
  🟡 Monolithic Files (>600 lines)

  | File                                               | Lines | Recommendation                              |
  |----------------------------------------------------|-------|---------------------------------------------|
  | apps/core-server-rs/src/services/deployments.rs    | 1,017 | Split into ssh.rs, steps.rs, jobs.rs        |
  | apps/core-server-rs/src/routes/weather_stations.rs | 742   | Extract WS-2902 protocol handling           |
  | apps/core-server/app/schemas.py                    | 777   | Split by domain (nodes, sensors, schedules) |
  | apps/farmctl/src/bundle.rs                         | 678   | Separate DMG generation logic               |
  | apps/farmctl/src/launchd.rs                        | 648   | Extract plist generation                    |

  ---
  🟡 Pydantic v2 Migration TODOs (50+ occurrences)

  All in apps/node-agent/app/generated_api/generated_api/docs/*.md - these are auto-generated files.

  Sample files:
  - AdoptionTokenRead.md:22
  - AnalyticsBundleInput.md:18
  - AnalyticsFeedStatusEntry.md:17
  - BackupRetentionPolicy.md:17
  - ConditionGroupInputConditionsInner.md:28
  - ConnectionStatus.md:19
  - DashboardSnapshot.md:28
  - NodeRead.md:26
  - SensorRead.md:24
  - ... and 40+ more

  Recommendation: Regenerate API client with updated OpenAPI generator that supports Pydantic v2.

  ---
  Summary Statistics

  | Category                | Count     | Severity  |
  |-------------------------|-----------|-----------|
  | Mutex lock poisoning    | 8         | 🔴 HIGH   |
  | Production .expect()    | 14        | 🔴 HIGH   |
  | SSH password handling   | 10 refs   | 🔴 HIGH   |
  | Default DB credentials  | 2         | 🔴 HIGH   |
  | Hardcoded localhost     | 22        | 🟠 MEDIUM |
  | Silent data overwrites  | 9         | 🟠 MEDIUM |
  | E2E test failure        | 1         | 🟠 MEDIUM |
  | Hardcoded test password | 1         | 🟠 MEDIUM |
  | Test code .unwrap()     | 21        | 🟡 LOW    |
  | Monolithic files        | 5         | 🟡 LOW    |
  | Pydantic TODOs          | 50+       | 🟡 LOW    |
  | Total Issues            | ~143 refs | —         |

  ---
  End of Audit Report
