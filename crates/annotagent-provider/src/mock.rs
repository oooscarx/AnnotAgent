use std::{collections::VecDeque, sync::Mutex};

use annotagent_core::{
    CoreError, CoreResult, ModelCapabilities, ModelRequest, ModelResponse, ModelToolCall,
    TokenUsage, ToolCallId, UsageSource, VisionModelProvider,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockScript {
    pub steps: Vec<MockStep>,
}

impl MockScript {
    pub fn from_yaml(input: &str) -> CoreResult<Self> {
        serde_yaml::from_str(input)
            .map_err(|error| CoreError::Provider(format!("invalid mock script: {error}")))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockStep {
    pub expect_task: Option<String>,
    pub expect_message_contains: Option<String>,
    pub response: MockResponseSpec,
    #[serde(default = "default_mock_usage")]
    pub usage: MockUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MockResponseSpec {
    Content {
        content: String,
    },
    ToolCall {
        name: String,
        arguments: serde_json::Value,
    },
    ToolCalls {
        calls: Vec<MockToolCall>,
        content: Option<String>,
    },
    MalformedArguments {
        name: String,
        arguments: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MockUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

const fn default_mock_usage() -> MockUsage {
    MockUsage {
        input_tokens: 120,
        output_tokens: 40,
    }
}

pub struct MockVisionProvider {
    steps: Mutex<VecDeque<MockStep>>,
}

impl MockVisionProvider {
    #[must_use]
    pub fn new(script: MockScript) -> Self {
        Self {
            steps: Mutex::new(script.steps.into()),
        }
    }

    #[must_use]
    pub fn remaining_steps(&self) -> usize {
        self.steps.lock().map_or(0, |steps| steps.len())
    }
}

#[async_trait]
impl VisionModelProvider for MockVisionProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            vision: true,
            tool_calls: true,
            json_schema: true,
            usage_reporting: true,
            multi_image: true,
        }
    }

    async fn complete(
        &self,
        request: ModelRequest,
        cancellation: CancellationToken,
    ) -> CoreResult<ModelResponse> {
        if cancellation.is_cancelled() {
            return Err(CoreError::Provider("mock request cancelled".to_owned()));
        }
        let step = self
            .steps
            .lock()
            .map_err(|_| CoreError::Provider("mock script lock poisoned".to_owned()))?
            .pop_front()
            .ok_or_else(|| CoreError::Provider("mock script exhausted".to_owned()))?;
        if let Some(expected) = &step.expect_task
            && request.task_id.as_str() != expected
        {
            return Err(CoreError::Provider(format!(
                "mock expected task {expected:?}, received {:?}",
                request.task_id.as_str()
            )));
        }
        if let Some(expected) = &step.expect_message_contains
            && !request
                .messages
                .iter()
                .any(|message| message.content.contains(expected))
        {
            return Err(CoreError::Provider(format!(
                "mock expected context containing {expected:?}"
            )));
        }

        let usage = TokenUsage::known(
            step.usage.input_tokens,
            step.usage.output_tokens,
            UsageSource::Mock,
        );
        let (content, tool_calls) = match step.response {
            MockResponseSpec::Content { content } => (Some(content), Vec::new()),
            MockResponseSpec::ToolCall { name, arguments } => (
                None,
                vec![ModelToolCall {
                    id: ToolCallId::new(format!("mock-{}", uuid::Uuid::new_v4())),
                    name,
                    arguments,
                }],
            ),
            MockResponseSpec::ToolCalls { calls, content } => (
                content,
                calls
                    .into_iter()
                    .map(|call| ModelToolCall {
                        id: ToolCallId::new(format!("mock-{}", uuid::Uuid::new_v4())),
                        name: call.name,
                        arguments: call.arguments,
                    })
                    .collect(),
            ),
            MockResponseSpec::MalformedArguments { name, arguments } => (
                None,
                vec![ModelToolCall {
                    id: ToolCallId::new(format!("mock-{}", uuid::Uuid::new_v4())),
                    name,
                    arguments: serde_json::Value::String(arguments),
                }],
            ),
            MockResponseSpec::Error { message } => return Err(CoreError::Provider(message)),
        };
        Ok(ModelResponse {
            content,
            tool_calls,
            usage,
            request_id: Some(format!("mock-{}", uuid::Uuid::new_v4())),
            provider_metadata: std::collections::BTreeMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use annotagent_core::{ModelRequest, TaskId};

    use super::*;

    #[tokio::test]
    async fn script_checks_task_and_reports_mock_usage() {
        let provider = MockVisionProvider::new(MockScript {
            steps: vec![MockStep {
                expect_task: Some("objects".to_owned()),
                expect_message_contains: None,
                response: MockResponseSpec::Content {
                    content: "done".to_owned(),
                },
                usage: default_mock_usage(),
            }],
        });
        let response = provider
            .complete(
                ModelRequest {
                    model: "mock".to_owned(),
                    task_id: TaskId::from("objects"),
                    messages: Vec::new(),
                    images: Vec::new(),
                    tools: Vec::new(),
                    max_output_tokens: 100,
                    temperature: 0.0,
                    extra: BTreeMap::new(),
                },
                CancellationToken::new(),
            )
            .await
            .expect("scripted response");
        assert_eq!(response.usage.source, UsageSource::Mock);
    }
}
