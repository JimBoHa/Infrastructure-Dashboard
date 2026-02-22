use anyhow::{bail, Context, Result};
use aws_cognito_srp::{SrpClient, User};
use aws_config::BehaviorVersion;
use aws_sdk_cognitoidentityprovider::{
    types::{AuthFlowType, ChallengeNameType},
    Client as CognitoClient,
};
use aws_types::region::Region;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::warn;

const EMPORIA_USER_POOL_ID: &str = "us-east-2_ghlOXVLi1";
const EMPORIA_CLIENT_ID: &str = "4qte47jbstod8apnfic0bunmrq";
const EMPORIA_API_BASE: &str = "https://api.emporiaenergy.com";
const DEFAULT_SCALE: &str = "1S";
const ENERGY_UNIT_KWH: &str = "KilowattHours";
const ENERGY_UNIT_VOLTAGE: &str = "Voltage";
const ENERGY_UNIT_AMP_HOURS: &str = "AmpHours";
const DEVICE_LIST_USAGE_MAX_ATTEMPTS: usize = 3;
const DEVICE_LIST_USAGE_RETRY_DELAY_MS: u64 = 250;

#[derive(Debug, Clone)]
pub struct EmporiaTokens {
    pub id_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EmporiaDeviceInfo {
    pub device_gid: String,
    pub name: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmporiaDeviceReading {
    pub device_gid: String,
    pub main_power_w: f64,
    pub mains_voltage_v: Option<f64>,
    pub mains_current_a: Option<f64>,
    pub channels: Vec<EmporiaChannelReading>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmporiaChannelReading {
    pub channel_num: String,
    pub raw_channel_num: String,
    pub nested_device_gid: Option<String>,
    pub name: Option<String>,
    pub power_w: Option<f64>,
    pub energy_kwh: Option<f64>,
    pub voltage_v: Option<f64>,
    pub current_a: Option<f64>,
    pub is_mains: bool,
    pub percentage: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct EmporiaUsageAggregate {
    pub timestamp: DateTime<Utc>,
    pub total_kw: f64,
    pub solar_kw: f64,
    pub consumption_kwh: f64,
    pub devices: Vec<EmporiaDeviceReading>,
}

#[derive(Deserialize)]
struct DevicesEnvelope {
    devices: Option<Vec<JsonValue>>,
}

#[derive(Deserialize)]
struct DeviceListUsagesEnvelope {
    #[serde(rename = "deviceListUsages")]
    device_list_usages: DeviceListUsages,
}

#[derive(Deserialize)]
struct DeviceListUsages {
    instant: String,
    #[serde(default)]
    devices: Vec<UsageDevice>,
}

#[derive(Deserialize)]
struct UsageDevice {
    #[serde(rename = "deviceGid")]
    device_gid: JsonValue,
    #[serde(rename = "channelUsages", default)]
    channel_usages: Vec<ChannelUsage>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct ChannelUsage {
    #[serde(rename = "channelNum")]
    channel_num: String,
    usage: Option<f64>,
    name: Option<String>,
    percentage: Option<f64>,
    #[serde(rename = "nestedDevices", default)]
    nested_devices: Vec<UsageDevice>,
}

#[derive(Debug, Clone)]
struct FlatChannelUsage {
    channel_key: String,
    raw_channel_num: String,
    nested_device_gid: Option<String>,
    usage: Option<f64>,
    name: Option<String>,
    percentage: Option<f64>,
}

fn collect_flat_channels(
    channels: &[ChannelUsage],
    prefix: Option<String>,
    nested_device_gid: Option<String>,
    out: &mut Vec<FlatChannelUsage>,
) {
    for channel in channels {
        let raw_channel_num = normalize_channel_num(&channel.channel_num);
        if raw_channel_num.is_empty() {
            continue;
        }
        let channel_key = match prefix.as_deref() {
            Some(prefix) if !prefix.is_empty() => format!("{prefix}{raw_channel_num}"),
            _ => raw_channel_num.clone(),
        };
        let name = channel
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());

        out.push(FlatChannelUsage {
            channel_key,
            raw_channel_num: raw_channel_num.clone(),
            nested_device_gid: nested_device_gid.clone(),
            usage: channel.usage,
            name,
            percentage: channel.percentage,
        });

        for nested in &channel.nested_devices {
            let nested_gid =
                parse_device_gid(&nested.device_gid).unwrap_or_else(|| "unknown".to_string());
            let next_prefix = Some(format!(
                "{}{}:",
                prefix.as_deref().unwrap_or(""),
                nested_gid.trim()
            ));
            collect_flat_channels(&nested.channel_usages, next_prefix, Some(nested_gid), out);
        }
    }
}

fn normalize_channel_num(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    if !trimmed.contains(',') {
        return trimmed.to_string();
    }

    let parts: Vec<&str> = trimmed
        .split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect();

    if parts.len() <= 1 {
        return trimmed.to_string();
    }

    let mut numeric_parts = Vec::with_capacity(parts.len());
    for part in &parts {
        match part.parse::<u16>() {
            Ok(num) => numeric_parts.push(num),
            Err(_) => return parts.join(","),
        }
    }

    numeric_parts.sort_unstable();
    numeric_parts.dedup();
    numeric_parts
        .into_iter()
        .map(|num| num.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub struct EmporiaService {
    http: Client,
}

impl EmporiaService {
    pub fn new(http: Client) -> Self {
        Self { http }
    }

    async fn cognito_client() -> Result<CognitoClient> {
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new("us-east-2"))
            .load()
            .await;
        Ok(CognitoClient::new(&config))
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<EmporiaTokens> {
        let client = Self::cognito_client().await?;
        let srp_user = User::new(EMPORIA_USER_POOL_ID, username, password);
        let srp_client = SrpClient::new(srp_user, EMPORIA_CLIENT_ID, None);
        let auth_parameters = srp_client.get_auth_parameters();

        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("USERNAME".into(), auth_parameters.username.clone());
        params.insert("SRP_A".into(), auth_parameters.a.clone());

        let challenge = client
            .initiate_auth()
            .auth_flow(AuthFlowType::UserSrpAuth)
            .client_id(EMPORIA_CLIENT_ID)
            .set_auth_parameters(Some(params))
            .send()
            .await?;

        let challenge_params = challenge
            .challenge_parameters()
            .cloned()
            .unwrap_or_default();
        let secret_block = challenge_params
            .get("SECRET_BLOCK")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Emporia login missing SECRET_BLOCK"))?;
        let user_id = challenge_params
            .get("USERNAME")
            .or_else(|| challenge_params.get("USER_ID_FOR_SRP"))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Emporia login missing USERNAME for SRP"))?;
        let salt = challenge_params
            .get("SALT")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Emporia login missing SALT"))?;
        let srp_b = challenge_params
            .get("SRP_B")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Emporia login missing SRP_B"))?;

        let verification = srp_client.verify(&secret_block, &user_id, &salt, &srp_b)?;

        let mut challenge_responses: HashMap<String, String> = HashMap::new();
        challenge_responses.insert(
            "PASSWORD_CLAIM_SECRET_BLOCK".into(),
            verification.password_claim_secret_block,
        );
        challenge_responses.insert(
            "PASSWORD_CLAIM_SIGNATURE".into(),
            verification.password_claim_signature,
        );
        challenge_responses.insert("TIMESTAMP".into(), verification.timestamp);
        challenge_responses.insert("USERNAME".into(), user_id.clone());

        let auth = client
            .respond_to_auth_challenge()
            .challenge_name(ChallengeNameType::PasswordVerifier)
            .client_id(EMPORIA_CLIENT_ID)
            .set_challenge_responses(Some(challenge_responses))
            .send()
            .await?;

        let result = auth
            .authentication_result()
            .ok_or_else(|| anyhow::anyhow!("Emporia login missing authentication result"))?;
        let id_token = result.id_token().unwrap_or_default().trim().to_string();
        let refresh_token = result.refresh_token().map(|t| t.trim().to_string());

        if id_token.is_empty() {
            bail!("Emporia login did not return an id_token");
        }

        Ok(EmporiaTokens {
            id_token,
            refresh_token,
        })
    }

    pub async fn refresh_with_token(&self, refresh_token: &str) -> Result<EmporiaTokens> {
        let client = Self::cognito_client().await?;
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("REFRESH_TOKEN".into(), refresh_token.to_string());

        let auth = client
            .initiate_auth()
            .auth_flow(AuthFlowType::RefreshTokenAuth)
            .client_id(EMPORIA_CLIENT_ID)
            .set_auth_parameters(Some(params))
            .send()
            .await?;

        let result = auth
            .authentication_result()
            .ok_or_else(|| anyhow::anyhow!("Emporia refresh missing authentication result"))?;
        let id_token = result.id_token().unwrap_or_default().trim().to_string();

        if id_token.is_empty() {
            bail!("Emporia refresh did not return an id_token");
        }

        Ok(EmporiaTokens {
            id_token,
            refresh_token: Some(refresh_token.to_string()),
        })
    }

    pub async fn fetch_devices(&self, id_token: &str) -> Result<Vec<EmporiaDeviceInfo>> {
        let url = format!("{EMPORIA_API_BASE}/customers/devices");
        let payload: DevicesEnvelope = self
            .http
            .get(url)
            .header("authtoken", id_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("Failed to decode Emporia devices response")?;

        let mut devices = Vec::new();
        if let Some(entries) = payload.devices {
            for entry in entries {
                devices.extend(flatten_devices(&entry));
            }
        }

        Ok(devices)
    }

    pub async fn fetch_usage(
        &self,
        id_token: &str,
        device_gids: &[String],
    ) -> Result<EmporiaUsageAggregate> {
        let device_gids: Vec<String> = device_gids
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .collect();
        if device_gids.is_empty() {
            bail!("Emporia feed has no site IDs configured");
        }
        // Emporia expects a comma-separated list of device GIDs.
        let joined = device_gids.join(",");

        let now = Utc::now();

        let usage_kwh = self
            .fetch_device_list_usages(id_token, joined.as_str(), &now, ENERGY_UNIT_KWH)
            .await?;

        let usage_voltage = match self
            .fetch_device_list_usages(id_token, joined.as_str(), &now, ENERGY_UNIT_VOLTAGE)
            .await
        {
            Ok(payload) => Some(payload),
            Err(err) => {
                warn!("Emporia usage Voltage poll failed; continuing without voltage readbacks: {err:#}");
                None
            }
        };

        let usage_amp_hours = match self
            .fetch_device_list_usages(id_token, joined.as_str(), &now, ENERGY_UNIT_AMP_HOURS)
            .await
        {
            Ok(payload) => Some(payload),
            Err(err) => {
                warn!("Emporia usage AmpHours poll failed; continuing without current readbacks: {err:#}");
                None
            }
        };

        let timestamp = parse_timestamp(&usage_kwh.instant).unwrap_or(now);
        let mut total_kw = 0.0_f64;
        let mut consumption_kwh = 0.0_f64;
        let mut devices: HashMap<String, EmporiaDeviceReading> = HashMap::new();
        let mut channels_by_device: HashMap<String, HashMap<String, EmporiaChannelReading>> =
            HashMap::new();

        for device in usage_kwh.devices {
            let device_gid =
                parse_device_gid(&device.device_gid).unwrap_or_else(|| "unknown".to_string());

            let mut flat_channels = Vec::new();
            collect_flat_channels(&device.channel_usages, None, None, &mut flat_channels);
            let top_level_total = device.channel_usages.len();

            let mut main_power_w = 0.0_f64;
            let mut main_channels = 0usize;
            let mut fallback_power_w = 0.0_f64;

            let channel_map = channels_by_device.entry(device_gid.clone()).or_default();

            for channel in flat_channels {
                if let Some(usage_kwh) = channel.usage {
                    let power_w = usage_to_average_power_w(usage_kwh);
                    consumption_kwh += usage_kwh.max(0.0);

                    let entry = channel_map
                        .entry(channel.channel_key.clone())
                        .or_insert_with(|| EmporiaChannelReading {
                            channel_num: channel.channel_key.clone(),
                            raw_channel_num: channel.raw_channel_num.clone(),
                            nested_device_gid: channel.nested_device_gid.clone(),
                            name: channel.name.clone(),
                            power_w: None,
                            energy_kwh: None,
                            voltage_v: None,
                            current_a: None,
                            is_mains: false,
                            percentage: channel.percentage,
                        });

                    if entry.name.is_none() {
                        entry.name = channel.name.clone();
                    }
                    entry.power_w = Some(power_w);
                    entry.energy_kwh = Some(usage_kwh);
                    entry.percentage = channel.percentage.or(entry.percentage);

                    if channel.nested_device_gid.is_none() {
                        fallback_power_w += power_w;
                        let is_mains = is_main_channel(
                            &channel.raw_channel_num,
                            entry.name.as_deref(),
                            top_level_total,
                        );
                        if is_mains {
                            entry.is_mains = true;
                            main_power_w += power_w;
                            main_channels += 1;
                        }
                    }
                } else {
                    let entry = channel_map
                        .entry(channel.channel_key.clone())
                        .or_insert_with(|| EmporiaChannelReading {
                            channel_num: channel.channel_key.clone(),
                            raw_channel_num: channel.raw_channel_num.clone(),
                            nested_device_gid: channel.nested_device_gid.clone(),
                            name: channel.name.clone(),
                            power_w: None,
                            energy_kwh: None,
                            voltage_v: None,
                            current_a: None,
                            is_mains: false,
                            percentage: channel.percentage,
                        });

                    if channel.nested_device_gid.is_none()
                        && is_main_channel(
                            &channel.raw_channel_num,
                            entry.name.as_deref(),
                            top_level_total,
                        )
                    {
                        entry.is_mains = true;
                    }
                }
            }

            if main_channels == 0 {
                main_power_w = fallback_power_w;
            }
            if main_power_w > 0.0 {
                total_kw += main_power_w / 1000.0;
            }
            devices.insert(
                device_gid.clone(),
                EmporiaDeviceReading {
                    device_gid,
                    main_power_w,
                    mains_voltage_v: None,
                    mains_current_a: None,
                    channels: Vec::new(),
                },
            );
        }

        if let Some(voltage) = usage_voltage {
            for device in voltage.devices {
                let device_gid =
                    parse_device_gid(&device.device_gid).unwrap_or_else(|| "unknown".to_string());
                let Some(device_entry) = devices.get_mut(&device_gid) else {
                    continue;
                };

                let mut flat_channels = Vec::new();
                collect_flat_channels(&device.channel_usages, None, None, &mut flat_channels);
                let top_level_total = device.channel_usages.len();

                let channel_map = channels_by_device.entry(device_gid.clone()).or_default();
                let mut mains_sum = 0.0_f64;
                let mut mains_count = 0usize;

                for channel in flat_channels {
                    if let Some(usage_v) = channel.usage {
                        let entry = channel_map
                            .entry(channel.channel_key.clone())
                            .or_insert_with(|| EmporiaChannelReading {
                                channel_num: channel.channel_key.clone(),
                                raw_channel_num: channel.raw_channel_num.clone(),
                                nested_device_gid: channel.nested_device_gid.clone(),
                                name: channel.name.clone(),
                                power_w: None,
                                energy_kwh: None,
                                voltage_v: None,
                                current_a: None,
                                is_mains: false,
                                percentage: channel.percentage,
                            });
                        if entry.name.is_none() {
                            entry.name = channel.name.clone();
                        }
                        entry.voltage_v = Some(usage_v);

                        if channel.nested_device_gid.is_none() {
                            let is_mains = is_main_channel(
                                &channel.raw_channel_num,
                                entry.name.as_deref(),
                                top_level_total,
                            );
                            if is_mains {
                                entry.is_mains = true;
                                mains_sum += usage_v;
                                mains_count += 1;
                            }
                        }
                    } else {
                        let entry = channel_map
                            .entry(channel.channel_key.clone())
                            .or_insert_with(|| EmporiaChannelReading {
                                channel_num: channel.channel_key.clone(),
                                raw_channel_num: channel.raw_channel_num.clone(),
                                nested_device_gid: channel.nested_device_gid.clone(),
                                name: channel.name.clone(),
                                power_w: None,
                                energy_kwh: None,
                                voltage_v: None,
                                current_a: None,
                                is_mains: false,
                                percentage: channel.percentage,
                            });

                        if channel.nested_device_gid.is_none()
                            && is_main_channel(
                                &channel.raw_channel_num,
                                entry.name.as_deref(),
                                top_level_total,
                            )
                        {
                            entry.is_mains = true;
                        }
                    }
                }

                if mains_count > 0 {
                    device_entry.mains_voltage_v = Some(mains_sum / (mains_count as f64));
                }
            }
        }

        if let Some(amp_hours) = usage_amp_hours {
            for device in amp_hours.devices {
                let device_gid =
                    parse_device_gid(&device.device_gid).unwrap_or_else(|| "unknown".to_string());
                let Some(device_entry) = devices.get_mut(&device_gid) else {
                    continue;
                };

                let mut flat_channels = Vec::new();
                collect_flat_channels(&device.channel_usages, None, None, &mut flat_channels);
                let top_level_total = device.channel_usages.len();

                let channel_map = channels_by_device.entry(device_gid.clone()).or_default();
                let mut main_current_a = 0.0_f64;
                let mut main_channels = 0usize;
                let mut fallback_current_a = 0.0_f64;

                for channel in flat_channels {
                    if let Some(usage_ah) = channel.usage {
                        let current_a = usage_to_average_current_a(usage_ah);
                        let entry = channel_map
                            .entry(channel.channel_key.clone())
                            .or_insert_with(|| EmporiaChannelReading {
                                channel_num: channel.channel_key.clone(),
                                raw_channel_num: channel.raw_channel_num.clone(),
                                nested_device_gid: channel.nested_device_gid.clone(),
                                name: channel.name.clone(),
                                power_w: None,
                                energy_kwh: None,
                                voltage_v: None,
                                current_a: None,
                                is_mains: false,
                                percentage: channel.percentage,
                            });
                        if entry.name.is_none() {
                            entry.name = channel.name.clone();
                        }
                        entry.current_a = Some(current_a);

                        if channel.nested_device_gid.is_none() {
                            fallback_current_a += current_a;
                            let is_mains = is_main_channel(
                                &channel.raw_channel_num,
                                entry.name.as_deref(),
                                top_level_total,
                            );
                            if is_mains {
                                entry.is_mains = true;
                                main_current_a += current_a;
                                main_channels += 1;
                            }
                        }
                    } else {
                        let entry = channel_map
                            .entry(channel.channel_key.clone())
                            .or_insert_with(|| EmporiaChannelReading {
                                channel_num: channel.channel_key.clone(),
                                raw_channel_num: channel.raw_channel_num.clone(),
                                nested_device_gid: channel.nested_device_gid.clone(),
                                name: channel.name.clone(),
                                power_w: None,
                                energy_kwh: None,
                                voltage_v: None,
                                current_a: None,
                                is_mains: false,
                                percentage: channel.percentage,
                            });

                        if channel.nested_device_gid.is_none()
                            && is_main_channel(
                                &channel.raw_channel_num,
                                entry.name.as_deref(),
                                top_level_total,
                            )
                        {
                            entry.is_mains = true;
                        }
                    }
                }

                if main_channels == 0 {
                    main_current_a = fallback_current_a;
                }
                if main_channels > 0 || fallback_current_a != 0.0 {
                    device_entry.mains_current_a = Some(main_current_a);
                }
            }
        }

        // Finalize channels vec for each device.
        for (device_gid, device_entry) in devices.iter_mut() {
            let mut channels: Vec<EmporiaChannelReading> = channels_by_device
                .remove(device_gid)
                .unwrap_or_default()
                .into_values()
                .collect();
            channels.sort_by(|a, b| a.channel_num.cmp(&b.channel_num));
            device_entry.channels = channels;
        }

        let mut devices: Vec<EmporiaDeviceReading> = devices.into_values().collect();
        devices.sort_by(|a, b| a.device_gid.cmp(&b.device_gid));

        Ok(EmporiaUsageAggregate {
            timestamp,
            total_kw,
            solar_kw: 0.0,
            consumption_kwh,
            devices,
        })
    }

    async fn fetch_device_list_usages(
        &self,
        id_token: &str,
        joined_device_gids: &str,
        instant: &DateTime<Utc>,
        energy_unit: &str,
    ) -> Result<DeviceListUsages> {
        let mut attempt = 0usize;
        let mut merged: Option<DeviceListUsages> = None;
        while attempt < DEVICE_LIST_USAGE_MAX_ATTEMPTS {
            if attempt > 0 {
                sleep(Duration::from_millis(
                    DEVICE_LIST_USAGE_RETRY_DELAY_MS.saturating_mul(attempt as u64),
                ))
                .await;
            }
            attempt += 1;

            let payload = self
                .fetch_device_list_usages_once(id_token, joined_device_gids, instant, energy_unit)
                .await?;

            let incomplete = has_missing_usage(&payload.devices);
            if let Some(existing) = merged.as_mut() {
                merge_device_list_usages(existing, payload);
            } else {
                merged = Some(payload);
            }

            let Some(current) = merged.as_ref() else {
                continue;
            };
            if !incomplete && !has_missing_usage(&current.devices) {
                break;
            }
        }

        Ok(merged.unwrap_or(DeviceListUsages {
            instant: instant.to_rfc3339(),
            devices: Vec::new(),
        }))
    }

    async fn fetch_device_list_usages_once(
        &self,
        id_token: &str,
        joined_device_gids: &str,
        instant: &DateTime<Utc>,
        energy_unit: &str,
    ) -> Result<DeviceListUsages> {
        let url = format!("{EMPORIA_API_BASE}/AppAPI");
        let payload: DeviceListUsagesEnvelope = self
            .http
            .get(url)
            .query(&[
                ("apiMethod", "getDeviceListUsages"),
                ("deviceGids", joined_device_gids),
                ("instant", &instant.to_rfc3339()),
                ("scale", DEFAULT_SCALE),
                ("energyUnit", energy_unit),
            ])
            .header("authtoken", id_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .with_context(|| {
                format!("Failed to decode Emporia usage response (energyUnit={energy_unit})")
            })?;

        Ok(payload.device_list_usages)
    }
}

fn has_missing_usage(devices: &[UsageDevice]) -> bool {
    for device in devices {
        for channel in &device.channel_usages {
            if channel.usage.is_none() {
                return true;
            }
            if has_missing_usage(&channel.nested_devices) {
                return true;
            }
        }
    }
    false
}

fn merge_device_list_usages(base: &mut DeviceListUsages, next: DeviceListUsages) {
    if base.instant.is_empty() {
        base.instant = next.instant;
    }

    let mut device_index = HashMap::new();
    for (idx, device) in base.devices.iter().enumerate() {
        device_index.insert(device_key(device), idx);
    }

    for device in next.devices {
        let key = device_key(&device);
        if let Some(&idx) = device_index.get(&key) {
            merge_usage_device(&mut base.devices[idx], device);
        } else {
            base.devices.push(device);
            device_index.insert(key, base.devices.len() - 1);
        }
    }
}

fn merge_usage_device(base: &mut UsageDevice, next: UsageDevice) {
    merge_channel_usages(&mut base.channel_usages, next.channel_usages);
}

fn merge_channel_usages(base: &mut Vec<ChannelUsage>, next: Vec<ChannelUsage>) {
    let mut channel_index = HashMap::new();
    for (idx, channel) in base.iter().enumerate() {
        channel_index.insert(channel.channel_num.trim().to_string(), idx);
    }

    for channel in next {
        let key = channel.channel_num.trim().to_string();
        if let Some(&idx) = channel_index.get(&key) {
            merge_channel_usage(&mut base[idx], channel);
        } else {
            base.push(channel);
            channel_index.insert(key, base.len() - 1);
        }
    }
}

fn merge_channel_usage(base: &mut ChannelUsage, next: ChannelUsage) {
    if base.usage.is_none() {
        base.usage = next.usage;
    }
    if base.name.is_none() {
        base.name = next.name;
    }
    if base.percentage.is_none() {
        base.percentage = next.percentage;
    }

    merge_usage_devices(&mut base.nested_devices, next.nested_devices);
}

fn merge_usage_devices(base: &mut Vec<UsageDevice>, next: Vec<UsageDevice>) {
    let mut device_index = HashMap::new();
    for (idx, device) in base.iter().enumerate() {
        device_index.insert(device_key(device), idx);
    }

    for device in next {
        let key = device_key(&device);
        if let Some(&idx) = device_index.get(&key) {
            merge_usage_device(&mut base[idx], device);
        } else {
            base.push(device);
            device_index.insert(key, base.len() - 1);
        }
    }
}

fn device_key(device: &UsageDevice) -> String {
    parse_device_gid(&device.device_gid).unwrap_or_else(|| "unknown".to_string())
}

fn usage_to_average_power_w(usage_kwh: f64) -> f64 {
    // With scale=1S, usage is kWh for a one second interval.
    // Convert kWh -> kW by scaling to hours, then to watts.
    usage_kwh * 3_600_000.0
}

fn usage_to_average_current_a(usage_ah: f64) -> f64 {
    // With scale=1S, usage is amp-hours for a one second interval.
    // Convert Ah -> A by scaling to hours (Ah / (1/3600 h) = Ah * 3600).
    usage_ah * 3_600.0
}

fn is_main_channel(channel_num: &str, _name: Option<&str>, total_channels: usize) -> bool {
    let channel_num = channel_num.trim();
    if channel_num.eq_ignore_ascii_case("main") {
        return true;
    }

    // Emporia's voltage/current readbacks often expose mains legs as explicit channels like
    // `Mains_A` / `Mains_B`. Treat any `mains*` channel as mains, but avoid broad substring
    // matches (e.g. "main panel") that can misclassify regular circuits as mains.
    if channel_num.to_lowercase().starts_with("mains") {
        return true;
    }

    if channel_num.is_empty() {
        return total_channels <= 1;
    }

    let mut saw_part = false;
    let mut part_count = 0usize;
    for part in channel_num.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        saw_part = true;
        part_count += 1;
        match part.parse::<u8>() {
            Ok(num) if (1..=3).contains(&num) => {}
            _ => return false,
        }
    }

    // Treat multi-part "1,2,3" style channels as mains, but do not treat single circuit
    // numbers (e.g. "1") as mains when a panel has many channels.
    saw_part && part_count >= 2
}

fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_device_gid(value: &JsonValue) -> Option<String> {
    if let Some(num) = value.as_i64() {
        return Some(num.to_string());
    }
    if let Some(num) = value.as_u64() {
        return Some(num.to_string());
    }
    value.as_str().map(|s| s.to_string())
}

fn flatten_devices(value: &JsonValue) -> Vec<EmporiaDeviceInfo> {
    let mut entries = Vec::new();
    if let Some(device_gid) = value.get("deviceGid").and_then(parse_device_gid) {
        let name = value
            .get("locationProperties")
            .and_then(|props| props.get("deviceName").or_else(|| props.get("displayName")))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string());
        let address = value
            .get("locationProperties")
            .and_then(extract_street_address)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let model = value
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let firmware = value
            .get("firmware")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        entries.push(EmporiaDeviceInfo {
            device_gid,
            name,
            model,
            firmware,
            address,
        });
    }

    if let Some(children) = value.get("devices").and_then(|v| v.as_array()) {
        for child in children {
            entries.extend(flatten_devices(child));
        }
    }

    entries
}

fn extract_street_address(location_properties: &JsonValue) -> Option<&str> {
    for key in [
        "streetAddress",
        "address",
        "address1",
        "street",
        "street1",
        "locationAddress",
    ] {
        if let Some(value) = location_properties.get(key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::is_main_channel;

    #[test]
    fn main_channel_detection_prefers_explicit_mains_labels() {
        assert!(is_main_channel("Main", None, 10));
        assert!(is_main_channel("Mains_A", None, 10));
        assert!(is_main_channel("mains_b", None, 10));
    }

    #[test]
    fn main_channel_detection_does_not_treat_single_circuits_as_mains() {
        assert!(!is_main_channel("1", None, 20));
        assert!(!is_main_channel("2", None, 20));
        assert!(!is_main_channel("3", None, 20));
    }

    #[test]
    fn main_channel_detection_allows_multi_part_mains_patterns() {
        assert!(is_main_channel("1,2,3", None, 20));
        assert!(is_main_channel("1, 2", None, 20));
        assert!(!is_main_channel("4,5", None, 20));
    }
}
