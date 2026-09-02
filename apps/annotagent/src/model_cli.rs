use annotagent_model_bundle::{pack_model_bundle, verify_model_bundle};
use anyhow::Result;

use crate::{ModelBundleCommand, ModelsCommand};

pub fn run(command: ModelsCommand) -> Result<()> {
    match command {
        ModelsCommand::Bundle { command } => match command {
            ModelBundleCommand::Pack { directory, output } => {
                let digest = pack_model_bundle(&directory, &output)?;
                println!("packed {}", output.display());
                println!("bundle sha256: {digest}");
            }
            ModelBundleCommand::Inspect { package } => {
                let verified = verify_model_bundle(&package)?;
                println!("{}", verified.manifest.to_toml()?);
                println!("bundle sha256: {}", verified.bundle_digest);
                println!("signature: {:?}", verified.signature);
                println!("files: {}", verified.files.len());
            }
            ModelBundleCommand::Verify { package } => {
                let verified = verify_model_bundle(&package)?;
                println!(
                    "verified {}@{} ({})",
                    verified.manifest.id, verified.manifest.version, verified.bundle_digest
                );
                println!("signature: {:?}", verified.signature);
            }
        },
    }
    Ok(())
}
