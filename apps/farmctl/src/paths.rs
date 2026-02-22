use std::path::{Path, PathBuf};

use crate::config::SetupConfig;
use crate::constants::STATE_FILE;

pub fn state_path(config: &SetupConfig) -> PathBuf {
    Path::new(&config.install_root).join(STATE_FILE)
}

pub fn native_root(config: &SetupConfig) -> PathBuf {
    Path::new(&config.install_root).join("native")
}

pub fn service_root(config: &SetupConfig) -> PathBuf {
    Path::new(&config.data_root).join("services")
}

pub fn postgres_binary(config: &SetupConfig) -> PathBuf {
    native_root(config).join("postgres/bin/postgres")
}

pub fn postgres_initdb(config: &SetupConfig) -> PathBuf {
    native_root(config).join("postgres/bin/initdb")
}

pub fn redis_binary(config: &SetupConfig) -> PathBuf {
    native_root(config).join("redis/bin/redis-server")
}

pub fn qdrant_binary(config: &SetupConfig) -> PathBuf {
    native_root(config).join("qdrant/bin/qdrant")
}

pub fn mosquitto_binary(config: &SetupConfig) -> PathBuf {
    let bin = native_root(config).join("mosquitto/bin/mosquitto");
    if bin.exists() {
        bin
    } else {
        native_root(config).join("mosquitto/sbin/mosquitto")
    }
}

pub fn postgres_data_dir(config: &SetupConfig) -> PathBuf {
    service_root(config).join("postgres")
}

pub fn redis_data_dir(config: &SetupConfig) -> PathBuf {
    service_root(config).join("redis")
}

pub fn qdrant_data_dir(config: &SetupConfig) -> PathBuf {
    Path::new(&config.data_root).join("storage/qdrant")
}

pub fn qdrant_config_path(config: &SetupConfig) -> PathBuf {
    qdrant_data_dir(config).join("qdrant.yaml")
}

pub fn mosquitto_dir(config: &SetupConfig) -> PathBuf {
    service_root(config).join("mosquitto")
}

pub fn mosquitto_config_path(config: &SetupConfig) -> PathBuf {
    mosquitto_dir(config).join("mosquitto.conf")
}

pub fn redis_config_path(config: &SetupConfig) -> PathBuf {
    redis_data_dir(config).join("redis.conf")
}
