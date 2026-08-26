use std::{collections::BTreeMap, sync::Arc};

use annotagent_core::{AgentTool, TaskId, ToolContext, ToolResult};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolRegistryError {
    #[error("tool {0:?} is already registered")]
    Duplicate(String),
    #[error("unknown tool {0:?}")]
    Unknown(String),
    #[error("tool arguments are invalid at {path}: {message}")]
    InvalidArguments { path: String, message: String },
    #[error("run was cancelled before tool execution")]
    Cancelled,
    #[error("tool {name:?} failed: {message}")]
    Execution { name: String, message: String },
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn AgentTool>>,
}

impl ToolRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn AgentTool>) -> Result<(), ToolRegistryError> {
        let name = tool.definition().name;
        if self.tools.insert(name.clone(), tool).is_some() {
            return Err(ToolRegistryError::Duplicate(name));
        }
        Ok(())
    }

    #[must_use]
    pub fn definitions(&self) -> Vec<annotagent_core::ToolDefinition> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    #[must_use]
    pub fn definitions_for_task(&self, task_id: &TaskId) -> Vec<annotagent_core::ToolDefinition> {
        self.tools
            .values()
            .filter(|tool| {
                let applicable = tool.applicable_tasks();
                applicable.is_empty() || applicable.contains(task_id)
            })
            .map(|tool| tool.definition())
            .collect()
    }

    #[must_use]
    pub fn is_read_only(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .is_some_and(|tool| tool.definition().read_only)
    }

    pub async fn execute(
        &self,
        name: &str,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolRegistryError> {
        if context.cancellation.is_cancelled() {
            return Err(ToolRegistryError::Cancelled);
        }
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolRegistryError::Unknown(name.to_owned()))?;
        validate_schema(&arguments, &tool.definition().parameters, "$")?;
        tool.execute(context, arguments)
            .await
            .map_err(|error| ToolRegistryError::Execution {
                name: name.to_owned(),
                message: error.to_string(),
            })
    }
}

#[must_use]
pub fn normalized_tool_signature(name: &str, arguments: &Value) -> String {
    format!("{name}:{}", canonical_json(arguments))
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let mut entries: Vec<_> = object.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            let body = entries
                .into_iter()
                .map(|(key, value)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_default(),
                        canonical_json(value)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn validate_schema(value: &Value, schema: &Value, path: &str) -> Result<(), ToolRegistryError> {
    if let Some(expected) = schema.get("type").and_then(Value::as_str) {
        let valid = match expected {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => false,
        };
        if !valid {
            return Err(ToolRegistryError::InvalidArguments {
                path: path.to_owned(),
                message: format!("expected {expected}"),
            });
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array)
        && !allowed.contains(value)
    {
        return Err(ToolRegistryError::InvalidArguments {
            path: path.to_owned(),
            message: "value is not in the allowed enum".to_owned(),
        });
    }
    if let Some(number) = value.as_f64() {
        if let Some(minimum) = schema.get("minimum").and_then(Value::as_f64)
            && number < minimum
        {
            return Err(ToolRegistryError::InvalidArguments {
                path: path.to_owned(),
                message: format!("must be at least {minimum}"),
            });
        }
        if let Some(maximum) = schema.get("maximum").and_then(Value::as_f64)
            && number > maximum
        {
            return Err(ToolRegistryError::InvalidArguments {
                path: path.to_owned(),
                message: format!("must be at most {maximum}"),
            });
        }
    }
    if let (Some(object), Some(properties)) = (
        value.as_object(),
        schema.get("properties").and_then(Value::as_object),
    ) {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for name in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(name) {
                    return Err(ToolRegistryError::InvalidArguments {
                        path: format!("{path}.{name}"),
                        message: "required property is missing".to_owned(),
                    });
                }
            }
        }
        if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
            for name in object.keys() {
                if !properties.contains_key(name) {
                    return Err(ToolRegistryError::InvalidArguments {
                        path: format!("{path}.{name}"),
                        message: "unknown property".to_owned(),
                    });
                }
            }
        }
        for (name, child_schema) in properties {
            if let Some(child) = object.get(name) {
                validate_schema(child, child_schema, &format!("{path}.{name}"))?;
            }
        }
    }
    if let (Some(items), Some(item_schema)) = (value.as_array(), schema.get("items")) {
        if let Some(minimum) = schema.get("minItems").and_then(Value::as_u64)
            && items.len() < usize::try_from(minimum).unwrap_or(usize::MAX)
        {
            return Err(ToolRegistryError::InvalidArguments {
                path: path.to_owned(),
                message: format!("must contain at least {minimum} items"),
            });
        }
        if let Some(maximum) = schema.get("maxItems").and_then(Value::as_u64)
            && items.len() > usize::try_from(maximum).unwrap_or(usize::MAX)
        {
            return Err(ToolRegistryError::InvalidArguments {
                path: path.to_owned(),
                message: format!("must contain at most {maximum} items"),
            });
        }
        for (index, item) in items.iter().enumerate() {
            validate_schema(item, item_schema, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn canonical_signature_ignores_object_key_order() {
        assert_eq!(
            normalized_tool_signature("x", &json!({"a": 1, "b": 2})),
            normalized_tool_signature("x", &json!({"b": 2, "a": 1}))
        );
    }

    #[test]
    fn schema_reports_precise_missing_property() {
        let error = validate_schema(
            &json!({}),
            &json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"]}),
            "$",
        )
        .expect_err("missing id");
        assert!(error.to_string().contains("$.id"));
    }

    #[test]
    fn schema_enforces_array_length_and_numeric_range() {
        let schema = json!({
            "type": "array",
            "minItems": 2,
            "maxItems": 2,
            "items": {"type": "number", "minimum": 0, "maximum": 1}
        });
        assert!(validate_schema(&json!([0.2]), &schema, "$").is_err());
        assert!(validate_schema(&json!([0.2, 1.2]), &schema, "$").is_err());
        assert!(validate_schema(&json!([0.2, 0.8]), &schema, "$").is_ok());
    }
}
