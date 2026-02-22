use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

pub const RULE_VERSION: u8 = 1;
const MAX_DEPTH: usize = 6;
const MAX_NODES: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEnvelope {
    pub version: u8,
    pub target_selector: TargetSelector,
    pub condition: ConditionNode,
    #[serde(default)]
    pub timing: TimingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetSelector {
    Sensor {
        sensor_id: String,
    },
    SensorSet {
        sensor_ids: Vec<String>,
        #[serde(default)]
        r#match: MatchMode,
    },
    NodeSensors {
        node_id: Uuid,
        #[serde(default)]
        types: Vec<String>,
        #[serde(default)]
        r#match: MatchMode,
    },
    Filter {
        provider: Option<String>,
        metric: Option<String>,
        sensor_type: Option<String>,
        #[serde(default)]
        r#match: MatchMode,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    #[default]
    PerSensor,
    Any,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConditionNode {
    Threshold {
        op: CompareOp,
        value: f64,
    },
    Range {
        mode: RangeMode,
        low: f64,
        high: f64,
    },
    Offline {
        missing_for_seconds: i64,
    },
    RollingWindow {
        window_seconds: i64,
        aggregate: AggregateOp,
        op: CompareOp,
        value: f64,
    },
    Deviation {
        window_seconds: i64,
        baseline: BaselineOp,
        mode: DeviationMode,
        value: f64,
    },
    ConsecutivePeriods {
        period: ConsecutivePeriod,
        count: u32,
        child: Box<ConditionNode>,
    },
    All {
        children: Vec<ConditionNode>,
    },
    Any {
        children: Vec<ConditionNode>,
    },
    Not {
        child: Box<ConditionNode>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineOp {
    Mean,
    Median,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviationMode {
    Percent,
    Absolute,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateOp {
    Avg,
    Min,
    Max,
    Stddev,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeMode {
    Inside,
    Outside,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsecutivePeriod {
    Eval,
    Hour,
    Day,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp {
    Lt,
    Lte,
    Gt,
    Gte,
    Eq,
    Neq,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimingConfig {
    #[serde(default)]
    pub debounce_seconds: i64,
    #[serde(default)]
    pub clear_hysteresis_seconds: i64,
    #[serde(default)]
    pub eval_interval_seconds: i64,
}

pub fn parse_rule_envelope(
    target_selector: &JsonValue,
    condition_ast: &JsonValue,
    timing: &JsonValue,
) -> Result<RuleEnvelope, String> {
    let target_selector: TargetSelector = serde_json::from_value(target_selector.clone())
        .map_err(|err| format!("invalid target_selector: {err}"))?;
    let condition: ConditionNode = serde_json::from_value(condition_ast.clone())
        .map_err(|err| format!("invalid condition_ast: {err}"))?;
    let timing: TimingConfig = if timing.is_null() {
        TimingConfig::default()
    } else {
        serde_json::from_value(timing.clone()).map_err(|err| format!("invalid timing: {err}"))?
    };

    let envelope = RuleEnvelope {
        version: RULE_VERSION,
        target_selector,
        condition,
        timing,
    };
    validate_rule_envelope(&envelope)?;
    Ok(envelope)
}

pub fn validate_rule_envelope(envelope: &RuleEnvelope) -> Result<(), String> {
    if envelope.version != RULE_VERSION {
        return Err(format!(
            "unsupported rule version {}; expected {}",
            envelope.version, RULE_VERSION
        ));
    }

    match &envelope.target_selector {
        TargetSelector::Sensor { sensor_id } => {
            if sensor_id.trim().is_empty() {
                return Err("sensor selector requires sensor_id".to_string());
            }
        }
        TargetSelector::SensorSet { sensor_ids, .. } => {
            if sensor_ids.is_empty() {
                return Err("sensor_set selector requires at least one sensor_id".to_string());
            }
            if sensor_ids.iter().any(|sensor_id| sensor_id.trim().is_empty()) {
                return Err("sensor_set selector has blank sensor_id".to_string());
            }
        }
        TargetSelector::NodeSensors { .. } => {}
        TargetSelector::Filter {
            provider,
            metric,
            sensor_type,
            ..
        } => {
            let has_any = provider
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
                || metric
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
                || sensor_type
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty());
            if !has_any {
                return Err("filter selector requires provider, metric, or sensor_type".to_string());
            }
        }
    }

    if envelope.timing.debounce_seconds < 0 {
        return Err("timing.debounce_seconds must be >= 0".to_string());
    }
    if envelope.timing.clear_hysteresis_seconds < 0 {
        return Err("timing.clear_hysteresis_seconds must be >= 0".to_string());
    }

    let mut node_count = 0usize;
    validate_condition_recursive(&envelope.condition, 1, &mut node_count)?;
    Ok(())
}

fn validate_condition_recursive(
    node: &ConditionNode,
    depth: usize,
    node_count: &mut usize,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("condition depth exceeds max {MAX_DEPTH}"));
    }
    *node_count += 1;
    if *node_count > MAX_NODES {
        return Err(format!("condition node count exceeds max {MAX_NODES}"));
    }

    match node {
        ConditionNode::Threshold { value, .. } => {
            if !value.is_finite() {
                return Err("threshold.value must be finite".to_string());
            }
        }
        ConditionNode::Range { low, high, .. } => {
            if !low.is_finite() || !high.is_finite() {
                return Err("range bounds must be finite".to_string());
            }
            if low >= high {
                return Err("range.low must be < range.high".to_string());
            }
        }
        ConditionNode::Offline {
            missing_for_seconds,
        } => {
            if *missing_for_seconds < 1 {
                return Err("offline.missing_for_seconds must be >= 1".to_string());
            }
        }
        ConditionNode::RollingWindow {
            window_seconds,
            value,
            ..
        } => {
            if *window_seconds < 1 {
                return Err("rolling_window.window_seconds must be >= 1".to_string());
            }
            if !value.is_finite() {
                return Err("rolling_window.value must be finite".to_string());
            }
        }
        ConditionNode::Deviation {
            window_seconds,
            value,
            ..
        } => {
            if *window_seconds < 1 {
                return Err("deviation.window_seconds must be >= 1".to_string());
            }
            if !value.is_finite() {
                return Err("deviation.value must be finite".to_string());
            }
            if *value < 0.0 {
                return Err("deviation.value must be >= 0".to_string());
            }
        }
        ConditionNode::ConsecutivePeriods { count, child, .. } => {
            if *count < 1 {
                return Err("consecutive_periods.count must be >= 1".to_string());
            }
            validate_condition_recursive(child, depth + 1, node_count)?;
        }
        ConditionNode::All { children } | ConditionNode::Any { children } => {
            if children.is_empty() {
                return Err("all/any requires at least one child".to_string());
            }
            for child in children {
                validate_condition_recursive(child, depth + 1, node_count)?;
            }
        }
        ConditionNode::Not { child } => {
            validate_condition_recursive(child, depth + 1, node_count)?;
        }
    }

    Ok(())
}

pub fn compare(value: f64, op: CompareOp, threshold: f64) -> bool {
    match op {
        CompareOp::Lt => value < threshold,
        CompareOp::Lte => value <= threshold,
        CompareOp::Gt => value > threshold,
        CompareOp::Gte => value >= threshold,
        CompareOp::Eq => (value - threshold).abs() <= f64::EPSILON,
        CompareOp::Neq => (value - threshold).abs() > f64::EPSILON,
    }
}
