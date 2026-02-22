use anyhow::{bail, Context, Result};
use postgres::{Client, NoTls};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::postgres_connection_string;
use crate::config::SetupConfig;

pub fn apply_migrations(config: &SetupConfig, migrations_root: &Path) -> Result<()> {
    apply_migrations_url(&config.database_url, migrations_root)
}

pub fn apply_migrations_url(database_url: &str, migrations_root: &Path) -> Result<()> {
    if !migrations_root.exists() {
        bail!(
            "Migrations directory missing at {} (bundle is incomplete)",
            migrations_root.display()
        );
    }

    let mut migrations: Vec<PathBuf> = fs::read_dir(migrations_root)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|v| v.to_str()) == Some("sql"))
        .collect();
    migrations.sort();
    if migrations.is_empty() {
        return Ok(());
    }

    let mut client = Client::connect(&postgres_connection_string(database_url), NoTls)
        .context("Failed to connect for migrations")?;

    for migration in migrations {
        let sql = fs::read_to_string(&migration)
            .with_context(|| format!("Failed to read migration {}", migration.display()))?;
        if sql.trim().is_empty() {
            continue;
        }

        if requires_autocommit(&sql) {
            client
                .batch_execute(&sql)
                .with_context(|| format!("Migration failed: {}", migration.display()))?;
            continue;
        }

        let mut transaction = client
            .transaction()
            .with_context(|| format!("Failed to start transaction for {}", migration.display()))?;
        transaction
            .batch_execute(&sql)
            .with_context(|| format!("Migration failed: {}", migration.display()))?;
        transaction.commit().with_context(|| {
            format!(
                "Failed to commit migration transaction for {}",
                migration.display()
            )
        })?;
    }

    Ok(())
}

fn requires_autocommit(sql: &str) -> bool {
    let lowered = sql.to_ascii_lowercase();
    lowered.contains("create materialized view")
        || lowered.contains("add_continuous_aggregate_policy")
}
