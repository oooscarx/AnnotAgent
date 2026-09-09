//! Opt-in verification of a ZIP actually downloaded by the HTTP browser test.
//! Uses the independent Rust validator, not the official Ultralytics loader.
use annotagent_export::training_package::{PackageManifest, validate_training_package};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::PathBuf};

#[test]
#[ignore = "requires an explicit browser-downloaded TEST ZIP"]
fn browser_download_is_portable_in_two_unrelated_directories() {
    let archive = PathBuf::from(std::env::var("ANNOTAGENT_DOWNLOADED_TEST_ZIP").unwrap());
    assert!(archive.is_absolute());
    validate_training_package(&archive).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let mut totals = None;
    for name in ["first-location", "另一目录 with spaces"] {
        let destination = temp.path().join(name);
        let mut zip = zip::ZipArchive::new(fs::File::open(&archive).unwrap()).unwrap();
        zip.extract(&destination).unwrap();
        let destination = destination.canonicalize().unwrap();
        let yaml_path = destination.join("data.yaml");
        let yaml: serde_yaml::Value =
            serde_yaml::from_slice(&fs::read(&yaml_path).unwrap()).unwrap();
        assert!(yaml.get("path").is_none());
        let manifest: PackageManifest = serde_json::from_slice(
            &fs::read(destination.join("annotagent/manifest.json")).unwrap(),
        )
        .unwrap();
        assert!(manifest.intent.project_id.len() > 1);
        let mut images = 0;
        let mut objects = 0;
        let mut empty = 0;
        let mut split_paths = BTreeSet::new();
        for split in ["train", "val"] {
            let relative = yaml[split].as_str().unwrap();
            let resolved = yaml_path
                .parent()
                .unwrap()
                .join(relative)
                .canonicalize()
                .unwrap();
            assert!(resolved.starts_with(destination.canonicalize().unwrap()));
            let mut count = 0;
            for entry in fs::read_dir(resolved).unwrap() {
                let path = entry.unwrap().path();
                assert!(split_paths.insert(path.file_name().unwrap().to_owned()));
                let label = destination
                    .join("labels")
                    .join(split)
                    .join(path.file_stem().unwrap())
                    .with_extension("txt");
                let text = fs::read_to_string(label).unwrap();
                images += 1;
                count += 1;
                if text.is_empty() {
                    empty += 1;
                }
                objects += text.lines().count();
                let key = path.strip_prefix(&destination).unwrap().to_str().unwrap();
                let bytes = fs::read(&path).unwrap();
                assert_eq!(
                    format!("{:x}", Sha256::digest(&bytes)),
                    manifest.files[key].sha256
                );
                image::load_from_memory(&bytes).unwrap();
            }
            assert!(count > 0);
        }
        assert!(images > 0 && objects > 0);
        let current = (images, objects, empty, zip.len());
        if let Some(previous) = totals {
            assert_eq!(previous, current);
        }
        totals = Some(current);
        println!(
            "{}: images={images}, objects={objects}, negatives={empty}, entries={}",
            yaml_path.display(),
            zip.len()
        );
    }
}
