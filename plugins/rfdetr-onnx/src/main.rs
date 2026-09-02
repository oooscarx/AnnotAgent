#![forbid(unsafe_code)]

use std::sync::Arc;

use annotagent_plugin_rfdetr_onnx::RfDetrOnnxPlugin;
use annotagent_plugin_sdk::{PluginSdkError, PluginServer};

#[tokio::main]
async fn main() -> Result<(), PluginSdkError> {
    PluginServer::run_from_stdin(Arc::new(RfDetrOnnxPlugin::load()?)).await
}
