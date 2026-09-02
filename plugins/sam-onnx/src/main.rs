use std::sync::Arc;

use annotagent_plugin_sam_onnx::SamOnnxPlugin;
use annotagent_plugin_sdk::{PluginSdkError, PluginServer};

#[tokio::main]
async fn main() -> Result<(), PluginSdkError> {
    PluginServer::run_from_stdin(Arc::new(SamOnnxPlugin::load()?)).await
}
