use std::sync::Arc;

use annotagent_plugin_sdk::{PluginSdkError, PluginServer};
use annotagent_plugin_yolo_onnx::YoloOnnxPlugin;

#[tokio::main]
async fn main() -> Result<(), PluginSdkError> {
    PluginServer::run_from_stdin(Arc::new(YoloOnnxPlugin::load()?)).await
}
