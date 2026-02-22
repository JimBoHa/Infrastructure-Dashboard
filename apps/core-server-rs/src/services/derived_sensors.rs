use std::collections::{HashMap, HashSet};

use evalexpr::{
    build_operator_tree, ContextWithMutableFunctions, ContextWithMutableVariables, EvalexprError,
    HashMapContext, Node, Value,
};

pub const SENSOR_CONFIG_SOURCE_DERIVED: &str = "derived";

const MAX_DERIVED_INPUT_LAG_SECONDS: i64 = 86_400; // 24h

#[derive(Debug, Clone)]
pub struct DerivedSensorInput {
    pub sensor_id: String,
    pub var: String,
    pub lag_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct DerivedSensorSpec {
    pub expression: String,
    pub inputs: Vec<DerivedSensorInput>,
}

#[derive(Debug, Clone)]
pub struct DerivedSensorCompiled {
    expression: String,
    inputs: Vec<DerivedSensorInput>,
    tree: Node,
    base_ctx: HashMapContext,
}

fn is_valid_var_name(raw: &str) -> bool {
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn to_float(value: &Value) -> Result<f64, String> {
    match value {
        Value::Float(v) => Ok(*v),
        Value::Int(v) => Ok(*v as f64),
        other => Err(format!("Expected numeric value, got {other:?}")),
    }
}

fn to_bool(value: &Value) -> Result<bool, String> {
    match value {
        Value::Boolean(v) => Ok(*v),
        Value::Float(v) => Ok(*v != 0.0),
        Value::Int(v) => Ok(*v != 0),
        other => Err(format!("Expected boolean/numeric condition, got {other:?}")),
    }
}

fn args_to_floats(args: &Value) -> Result<Vec<f64>, String> {
    match args {
        Value::Tuple(items) => items.iter().map(to_float).collect(),
        other => Ok(vec![to_float(other)?]),
    }
}

fn register_default_functions(ctx: &mut HashMapContext) -> Result<(), EvalexprError> {
    // min(a, b, ...)
    ctx.set_function(
        "min".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.is_empty() {
                return Err(EvalexprError::CustomMessage(
                    "min() requires at least 1 argument".to_string(),
                ));
            }
            let mut out = floats[0];
            for v in floats.iter().skip(1) {
                out = out.min(*v);
            }
            Ok(Value::from(out))
        }),
    )?;

    // max(a, b, ...)
    ctx.set_function(
        "max".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.is_empty() {
                return Err(EvalexprError::CustomMessage(
                    "max() requires at least 1 argument".to_string(),
                ));
            }
            let mut out = floats[0];
            for v in floats.iter().skip(1) {
                out = out.max(*v);
            }
            Ok(Value::from(out))
        }),
    )?;

    // sum(a, b, ...)
    ctx.set_function(
        "sum".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            let out: f64 = floats.into_iter().sum();
            Ok(Value::from(out))
        }),
    )?;

    // avg(a, b, ...)
    ctx.set_function(
        "avg".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.is_empty() {
                return Err(EvalexprError::CustomMessage(
                    "avg() requires at least 1 argument".to_string(),
                ));
            }
            let sum: f64 = floats.iter().sum();
            Ok(Value::from(sum / (floats.len() as f64)))
        }),
    )?;

    // clamp(x, lo, hi)
    ctx.set_function(
        "clamp".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 3 {
                return Err(EvalexprError::CustomMessage(
                    "clamp(x, lo, hi) requires exactly 3 arguments".to_string(),
                ));
            }
            let x = floats[0];
            let lo = floats[1];
            let hi = floats[2];
            Ok(Value::from(x.max(lo).min(hi)))
        }),
    )?;

    // abs(x)
    ctx.set_function(
        "abs".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "abs(x) requires exactly 1 argument".to_string(),
                ));
            }
            Ok(Value::from(floats[0].abs()))
        }),
    )?;

    // round(x, decimals)
    ctx.set_function(
        "round".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 2 {
                return Err(EvalexprError::CustomMessage(
                    "round(x, decimals) requires exactly 2 arguments".to_string(),
                ));
            }
            let x = floats[0];
            let decimals = floats[1].round().clamp(0.0, 12.0) as i32;
            let factor = 10_f64.powi(decimals);
            Ok(Value::from((x * factor).round() / factor))
        }),
    )?;

    // floor(x)
    ctx.set_function(
        "floor".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "floor(x) requires exactly 1 argument".to_string(),
                ));
            }
            Ok(Value::from(floats[0].floor()))
        }),
    )?;

    // ceil(x)
    ctx.set_function(
        "ceil".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "ceil(x) requires exactly 1 argument".to_string(),
                ));
            }
            Ok(Value::from(floats[0].ceil()))
        }),
    )?;

    // sqrt(x)
    ctx.set_function(
        "sqrt".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "sqrt(x) requires exactly 1 argument".to_string(),
                ));
            }
            if floats[0] < 0.0 {
                return Err(EvalexprError::CustomMessage(
                    "sqrt(x) requires x >= 0".to_string(),
                ));
            }
            Ok(Value::from(floats[0].sqrt()))
        }),
    )?;

    // pow(x, y)
    ctx.set_function(
        "pow".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 2 {
                return Err(EvalexprError::CustomMessage(
                    "pow(x, y) requires exactly 2 arguments".to_string(),
                ));
            }
            let out = floats[0].powf(floats[1]);
            if !out.is_finite() {
                return Err(EvalexprError::CustomMessage(
                    "pow(x, y) evaluates to a non-finite number".to_string(),
                ));
            }
            Ok(Value::from(out))
        }),
    )?;

    // ln(x) (natural log)
    ctx.set_function(
        "ln".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "ln(x) requires exactly 1 argument".to_string(),
                ));
            }
            if floats[0] <= 0.0 {
                return Err(EvalexprError::CustomMessage(
                    "ln(x) requires x > 0".to_string(),
                ));
            }
            Ok(Value::from(floats[0].ln()))
        }),
    )?;

    // log10(x)
    ctx.set_function(
        "log10".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "log10(x) requires exactly 1 argument".to_string(),
                ));
            }
            if floats[0] <= 0.0 {
                return Err(EvalexprError::CustomMessage(
                    "log10(x) requires x > 0".to_string(),
                ));
            }
            Ok(Value::from(floats[0].log10()))
        }),
    )?;

    // log(x, base)
    ctx.set_function(
        "log".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 2 {
                return Err(EvalexprError::CustomMessage(
                    "log(x, base) requires exactly 2 arguments".to_string(),
                ));
            }
            let x = floats[0];
            let base = floats[1];
            if x <= 0.0 {
                return Err(EvalexprError::CustomMessage(
                    "log(x, base) requires x > 0".to_string(),
                ));
            }
            if base <= 0.0 || (base - 1.0).abs() < 1e-12 {
                return Err(EvalexprError::CustomMessage(
                    "log(x, base) requires base > 0 and base != 1".to_string(),
                ));
            }
            let out = x.log(base);
            if !out.is_finite() {
                return Err(EvalexprError::CustomMessage(
                    "log(x, base) evaluates to a non-finite number".to_string(),
                ));
            }
            Ok(Value::from(out))
        }),
    )?;

    // exp(x)
    ctx.set_function(
        "exp".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "exp(x) requires exactly 1 argument".to_string(),
                ));
            }
            let out = floats[0].exp();
            if !out.is_finite() {
                return Err(EvalexprError::CustomMessage(
                    "exp(x) evaluates to a non-finite number".to_string(),
                ));
            }
            Ok(Value::from(out))
        }),
    )?;

    // sin(x) (radians)
    ctx.set_function(
        "sin".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "sin(x) requires exactly 1 argument".to_string(),
                ));
            }
            Ok(Value::from(floats[0].sin()))
        }),
    )?;

    // cos(x) (radians)
    ctx.set_function(
        "cos".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "cos(x) requires exactly 1 argument".to_string(),
                ));
            }
            Ok(Value::from(floats[0].cos()))
        }),
    )?;

    // tan(x) (radians)
    ctx.set_function(
        "tan".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "tan(x) requires exactly 1 argument".to_string(),
                ));
            }
            Ok(Value::from(floats[0].tan()))
        }),
    )?;

    // deg2rad(x)
    ctx.set_function(
        "deg2rad".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "deg2rad(x) requires exactly 1 argument".to_string(),
                ));
            }
            Ok(Value::from(floats[0].to_radians()))
        }),
    )?;

    // rad2deg(x)
    ctx.set_function(
        "rad2deg".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "rad2deg(x) requires exactly 1 argument".to_string(),
                ));
            }
            Ok(Value::from(floats[0].to_degrees()))
        }),
    )?;

    // sign(x) -> -1, 0, 1
    ctx.set_function(
        "sign".to_string(),
        evalexpr::Function::new(|args| {
            let floats = args_to_floats(args).map_err(EvalexprError::CustomMessage)?;
            if floats.len() != 1 {
                return Err(EvalexprError::CustomMessage(
                    "sign(x) requires exactly 1 argument".to_string(),
                ));
            }
            let x = floats[0];
            let out = if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            };
            Ok(Value::from(out))
        }),
    )?;

    // if(cond, a, b) -> choose a if cond is true, else b
    ctx.set_function(
        "if".to_string(),
        evalexpr::Function::new(|args| {
            let Value::Tuple(items) = args else {
                return Err(EvalexprError::CustomMessage(
                    "if(cond, a, b) requires exactly 3 arguments".to_string(),
                ));
            };
            if items.len() != 3 {
                return Err(EvalexprError::CustomMessage(
                    "if(cond, a, b) requires exactly 3 arguments".to_string(),
                ));
            }
            let cond = to_bool(&items[0]).map_err(EvalexprError::CustomMessage)?;
            let a = to_float(&items[1]).map_err(EvalexprError::CustomMessage)?;
            let b = to_float(&items[2]).map_err(EvalexprError::CustomMessage)?;
            Ok(Value::from(if cond { a } else { b }))
        }),
    )?;

    Ok(())
}

pub fn parse_derived_sensor_spec(
    config: &serde_json::Value,
) -> Result<Option<DerivedSensorSpec>, String> {
    let source = config
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if source != SENSOR_CONFIG_SOURCE_DERIVED {
        return Ok(None);
    }

    let derived = config
        .get("derived")
        .ok_or("Derived sensor config requires config.derived")?;
    let derived_obj = derived
        .as_object()
        .ok_or("Derived sensor config.derived must be an object")?;
    let expression = derived_obj
        .get("expression")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if expression.is_empty() {
        return Err("Derived sensor requires a non-empty expression".to_string());
    }

    let inputs_value = derived_obj
        .get("inputs")
        .ok_or("Derived sensor config requires derived.inputs")?;
    let inputs_arr = inputs_value
        .as_array()
        .ok_or("Derived sensor derived.inputs must be an array")?;
    if inputs_arr.is_empty() {
        return Err("Derived sensor must include at least one input".to_string());
    }
    if inputs_arr.len() > 10 {
        return Err("Derived sensor supports up to 10 inputs".to_string());
    }

    let mut inputs: Vec<DerivedSensorInput> = Vec::new();
    let mut seen_vars: HashSet<String> = HashSet::new();

    for (idx, raw) in inputs_arr.iter().enumerate() {
        let obj = raw
            .as_object()
            .ok_or_else(|| format!("Derived sensor input #{idx} must be an object"))?;
        let sensor_id = obj
            .get("sensor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if sensor_id.is_empty() {
            return Err(format!("Derived sensor input #{idx} missing sensor_id"));
        }

        let var = obj
            .get("var")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if var.is_empty() {
            return Err(format!("Derived sensor input #{idx} missing var"));
        }
        if !is_valid_var_name(&var) {
            return Err(format!(
                "Derived sensor input var \"{var}\" is invalid (use letters/numbers/underscore; must start with a letter/underscore)"
            ));
        }
        if !seen_vars.insert(var.clone()) {
            return Err(format!(
                "Derived sensor inputs must use unique variable names (duplicate \"{var}\")"
            ));
        }

        let lag_seconds = obj.get("lag_seconds").and_then(|v| v.as_i64()).unwrap_or(0);
        let lag_abs = lag_seconds.checked_abs().unwrap_or(i64::MAX);
        if lag_abs > MAX_DERIVED_INPUT_LAG_SECONDS {
            return Err(format!(
                "Derived sensor input #{idx} lag_seconds out of range (abs max {}s)",
                MAX_DERIVED_INPUT_LAG_SECONDS
            ));
        }

        inputs.push(DerivedSensorInput {
            sensor_id,
            var,
            lag_seconds,
        });
    }

    Ok(Some(DerivedSensorSpec { expression, inputs }))
}

pub fn compile_derived_sensor(spec: &DerivedSensorSpec) -> Result<DerivedSensorCompiled, String> {
    if spec.expression.trim().is_empty() {
        return Err("Derived sensor expression cannot be empty".to_string());
    }
    if spec.expression.len() > 4096 {
        return Err("Derived sensor expression is too long".to_string());
    }

    let mut ctx = HashMapContext::new();
    register_default_functions(&mut ctx)
        .map_err(|err| format!("Derived sensor function registry failed: {err}"))?;
    for input in &spec.inputs {
        ctx.set_value(input.var.clone(), Value::from(1.0))
            .map_err(|err| format!("Derived sensor variable init failed: {err}"))?;
    }

    let tree = build_operator_tree(spec.expression.trim())
        .map_err(|err| format!("Invalid expression: {err}"))?;

    let value = tree
        .eval_with_context(&ctx)
        .map_err(|err| format!("Invalid expression: {err}"))?;
    let numeric = to_float(&value)?;
    if !numeric.is_finite() {
        return Err("Expression evaluates to a non-finite number".to_string());
    }

    Ok(DerivedSensorCompiled {
        expression: spec.expression.clone(),
        inputs: spec.inputs.clone(),
        tree,
        base_ctx: ctx,
    })
}

impl DerivedSensorCompiled {
    pub fn inputs(&self) -> &[DerivedSensorInput] {
        &self.inputs
    }

    pub fn expression(&self) -> &str {
        &self.expression
    }

    pub fn eval_with_vars(&mut self, vars: &HashMap<String, f64>) -> Result<f64, String> {
        for input in &self.inputs {
            let Some(value) = vars.get(&input.var) else {
                return Err(format!("Missing value for variable \"{}\"", input.var));
            };
            self.base_ctx
                .set_value(input.var.clone(), Value::from(*value))
                .map_err(|err| format!("Failed to set variable {}: {err}", input.var))?;
        }

        let value = self
            .tree
            .eval_with_context(&self.base_ctx)
            .map_err(|err| format!("Expression evaluation failed: {err}"))?;
        let numeric = to_float(&value)?;
        if !numeric.is_finite() {
            return Err("Expression evaluates to a non-finite number".to_string());
        }
        Ok(numeric)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_non_derived_returns_none() {
        let config = serde_json::json!({});
        let parsed = parse_derived_sensor_spec(&config).expect("parse");
        assert!(parsed.is_none());
    }

    #[test]
    fn parse_valid_derived_spec() {
        let config = serde_json::json!({
            "source": "derived",
            "derived": {
                "expression": "a + b",
                "inputs": [
                    { "sensor_id": "s1", "var": "a" },
                    { "sensor_id": "s2", "var": "b" }
                ]
            }
        });
        let spec = parse_derived_sensor_spec(&config)
            .expect("parse")
            .expect("spec");
        assert_eq!(spec.inputs.len(), 2);
        assert_eq!(spec.expression, "a + b");
        assert_eq!(spec.inputs[0].lag_seconds, 0);
        assert_eq!(spec.inputs[1].lag_seconds, 0);
    }

    #[test]
    fn parse_derived_spec_allows_lag_seconds() {
        let config = serde_json::json!({
            "source": "derived",
            "derived": {
                "expression": "a + b",
                "inputs": [
                    { "sensor_id": "s1", "var": "a", "lag_seconds": 60 },
                    { "sensor_id": "s2", "var": "b", "lag_seconds": -300 }
                ]
            }
        });
        let spec = parse_derived_sensor_spec(&config)
            .expect("parse")
            .expect("spec");
        assert_eq!(spec.inputs.len(), 2);
        assert_eq!(spec.inputs[0].lag_seconds, 60);
        assert_eq!(spec.inputs[1].lag_seconds, -300);
    }

    #[test]
    fn compile_and_eval_with_functions() {
        let config = serde_json::json!({
            "source": "derived",
            "derived": {
                "expression": "round(clamp(a + b, 0, 10), 2)",
                "inputs": [
                    { "sensor_id": "s1", "var": "a" },
                    { "sensor_id": "s2", "var": "b" }
                ]
            }
        });
        let spec = parse_derived_sensor_spec(&config)
            .expect("parse")
            .expect("spec");
        let mut compiled = compile_derived_sensor(&spec).expect("compile");
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 4.0);
        vars.insert("b".to_string(), 9.0);
        let value = compiled.eval_with_vars(&vars).expect("eval");
        assert!((value - 10.0).abs() < 1e-9);
    }

    #[test]
    fn compile_and_eval_with_extended_functions() {
        let config = serde_json::json!({
            "source": "derived",
            "derived": {
                "expression": "sqrt(pow(a, 2) + pow(b, 2))",
                "inputs": [
                    { "sensor_id": "s1", "var": "a" },
                    { "sensor_id": "s2", "var": "b" }
                ]
            }
        });
        let spec = parse_derived_sensor_spec(&config)
            .expect("parse")
            .expect("spec");
        let mut compiled = compile_derived_sensor(&spec).expect("compile");
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 3.0);
        vars.insert("b".to_string(), 4.0);
        let value = compiled.eval_with_vars(&vars).expect("eval");
        assert!((value - 5.0).abs() < 1e-9);
    }

    #[test]
    fn eval_if_and_log() {
        let config = serde_json::json!({
            "source": "derived",
            "derived": {
                "expression": "round(if(a, log10(100), 0), 4)",
                "inputs": [
                    { "sensor_id": "s1", "var": "a" }
                ]
            }
        });
        let spec = parse_derived_sensor_spec(&config)
            .expect("parse")
            .expect("spec");
        let mut compiled = compile_derived_sensor(&spec).expect("compile");
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 0.0);
        let value = compiled.eval_with_vars(&vars).expect("eval");
        assert!((value - 0.0).abs() < 1e-9);

        vars.insert("a".to_string(), 1.0);
        let value = compiled.eval_with_vars(&vars).expect("eval");
        assert!((value - 2.0).abs() < 1e-9);
    }
}
