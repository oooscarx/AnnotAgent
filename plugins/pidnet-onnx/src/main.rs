use std::sync::Arc;

use annotagent_plugin_pidnet_onnx::PidNetOnnxPlugin;
use annotagent_plugin_sdk::{PluginSdkError, PluginServer};

#[tokio::main]
async fn main() -> Result<(), PluginSdkError> {
    PluginServer::run_from_stdin(Arc::new(PidNetOnnxPlugin::load()?)).await
}
