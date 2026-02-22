use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static CATALOG: OnceLock<DeviceCatalog> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DeviceCatalog {
    pub version: u32,
    pub vendors: Vec<DeviceVendor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DeviceVendor {
    pub id: String,
    pub name: String,
    pub models: Vec<DeviceModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DeviceModel {
    pub id: String,
    pub name: String,
    pub since_year: Option<u32>,
    pub protocols: Vec<String>,
    pub points: Vec<DevicePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DevicePoint {
    pub name: String,
    pub metric: String,
    pub sensor_type: String,
    pub unit: String,
    pub protocol: String,
    #[serde(default)]
    pub register: Option<u32>,
    #[serde(default)]
    pub data_type: Option<String>,
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub oid: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub json_pointer: Option<String>,
    #[serde(default)]
    pub bacnet_object: Option<String>,
}

pub fn catalog() -> &'static DeviceCatalog {
    CATALOG.get_or_init(|| {
        let raw = include_str!("../../../shared/device_profiles/catalog.json");
        serde_json::from_str(raw).unwrap_or(DeviceCatalog {
            version: 1,
            vendors: Vec::new(),
        })
    })
}

pub fn find_model(vendor_id: &str, model_id: &str) -> Option<DeviceModel> {
    catalog()
        .vendors
        .iter()
        .find(|vendor| vendor.id == vendor_id)
        .and_then(|vendor| vendor.models.iter().find(|model| model.id == model_id))
        .cloned()
}
