use std::sync::Arc;

use annotagent_plugin_efficientsam_onnx::EfficientSamOnnxPlugin;
use annotagent_plugin_sdk::{PluginSdkError, PluginServer};

#[tokio::main]
async fn main() -> Result<(), PluginSdkError> {
    PluginServer::run_from_stdin(Arc::new(EfficientSamOnnxPlugin::load()?)).await
}
