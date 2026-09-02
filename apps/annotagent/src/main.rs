mod demo;
mod model_cli;
mod plugin_cli;
mod runner;
mod tui;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use annotagent_application::{AnnotAgentApplication, DatasetCoordinator, LocalApplication};
use annotagent_core::{DatasetExporter, DomainSkill, ProjectSnapshot, SnapshotImage};
use annotagent_export::{
    CocoExporter, LabelMeExporter, NativeExporter, YoloDetectionExporter, YoloSegmentationExporter,
};
use annotagent_skill_robocup::RoboCupSkill;
use annotagent_storage::{HistoryDocument, SqliteStore};
use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(
    name = "annotagent",
    version,
    about = "Composable annotation workflows for vision data"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init {
        project_directory: PathBuf,
        #[arg(long, default_value = "robocup")]
        skill: String,
    },
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    Import {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        images: PathBuf,
    },
    ImportAnnotations {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        format: String,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long = "map", value_name = "SOURCE=TARGET")]
        label_mapping: Vec<String>,
    },
    Run(RunArgs),
    Tui {
        #[arg(long)]
        project: Option<PathBuf>,
    },
    Serve {
        #[arg(long, default_value = "./workspace")]
        workspace: PathBuf,
        #[arg(long)]
        open: bool,
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    Plugin {
        #[arg(long)]
        data_dir: Option<PathBuf>,
        #[command(subcommand)]
        command: PluginCommand,
    },
    Models {
        #[arg(long, default_value = "./workspace")]
        workspace: PathBuf,
        #[command(subcommand)]
        command: ModelsCommand,
    },
    History {
        #[command(subcommand)]
        command: HistoryCommand,
    },
    Export {
        #[arg(long)]
        project: PathBuf,
        #[arg(long)]
        format: String,
        #[arg(long)]
        output: PathBuf,
    },
    Evaluate {
        #[arg(long)]
        ground_truth: PathBuf,
        #[arg(long)]
        predictions: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, default_value_t = 0.5)]
        bbox_iou_threshold: f64,
        #[arg(long)]
        minimum_field_region_iou: Option<f64>,
    },
    Doctor,
    Demo {
        name: String,
    },
}

#[derive(Debug, Clone, Args)]
struct RunArgs {
    #[arg(long)]
    project: PathBuf,
    #[arg(long, default_value = "openai_compatible")]
    provider: String,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Subcommand)]
enum ProjectCommand {
    Validate { project: PathBuf },
}

#[derive(Subcommand)]
enum SkillsCommand {
    List,
    Show { skill_id: String },
}

#[derive(Subcommand)]
enum PluginCommand {
    Dev {
        directory: PathBuf,
    },
    Inspect {
        package: PathBuf,
    },
    Pack {
        directory: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Verify {
        package: PathBuf,
    },
    Install {
        package: PathBuf,
        #[arg(long)]
        accept: bool,
    },
    Update {
        package: PathBuf,
        #[arg(long)]
        accept: bool,
    },
    List,
    Show {
        plugin_id: String,
        #[arg(long)]
        version: Option<String>,
    },
    Versions {
        plugin_id: String,
    },
    Provision {
        plugin_id: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        component: Option<String>,
        #[arg(long)]
        weights: PathBuf,
        #[arg(long)]
        sha256: Option<String>,
    },
    Test {
        plugin_id: String,
        #[arg(long)]
        version: Option<String>,
    },
    Doctor {
        plugin_id: String,
        #[arg(long)]
        version: Option<String>,
    },
    Start {
        plugin_id: String,
        #[arg(long)]
        version: Option<String>,
    },
    Stop {
        plugin_id: String,
        #[arg(long)]
        version: Option<String>,
    },
    Restart {
        plugin_id: String,
        #[arg(long)]
        version: Option<String>,
    },
    Enable {
        plugin_id: String,
        #[arg(long)]
        version: Option<String>,
    },
    Disable {
        plugin_id: String,
        #[arg(long)]
        version: Option<String>,
    },
    Uninstall {
        plugin_id: String,
        #[arg(long)]
        version: String,
    },
    References {
        plugin_id: String,
        #[arg(long)]
        version: String,
    },
}

#[derive(Subcommand)]
enum ModelsCommand {
    Catalog {
        #[command(subcommand)]
        command: Option<ModelCatalogCommand>,
    },
    Search {
        query: String,
    },
    Show {
        bundle_id: String,
    },
    Install {
        bundle: String,
        #[arg(long)]
        accept: bool,
    },
    Import {
        package: PathBuf,
        #[arg(long)]
        accept: bool,
    },
    List,
    Test {
        model_instance_id: String,
    },
    Disable {
        model_instance_id: String,
    },
    Enable {
        model_instance_id: String,
    },
    Remove {
        bundle: String,
    },
    References {
        bundle: String,
    },
    Doctor {
        model_instance_id: String,
    },
    Gc,
    Recipe {
        #[command(subcommand)]
        command: ModelRecipeCommand,
    },
    Bundle {
        #[command(subcommand)]
        command: ModelBundleCommand,
    },
}

#[derive(Subcommand)]
enum ModelCatalogCommand {
    List,
    AddLocal {
        directory: PathBuf,
    },
    Refresh,
    Build {
        directory: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        catalog_id: Option<String>,
    },
    Verify {
        catalog_file: PathBuf,
    },
}

#[derive(Subcommand)]
enum ModelRecipeCommand {
    Audit {
        recipe: PathBuf,
    },
    Fetch {
        recipe: PathBuf,
    },
    Build {
        recipe: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        catalog_entry: Option<PathBuf>,
        #[arg(long)]
        verification_report: Option<PathBuf>,
    },
    Verify {
        recipe: PathBuf,
    },
}

#[derive(Subcommand)]
enum ModelBundleCommand {
    Pack {
        directory: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Inspect {
        package: PathBuf,
    },
    Verify {
        package: PathBuf,
    },
}

#[derive(Subcommand)]
enum HistoryCommand {
    List,
    Show {
        run_id: annotagent_core::RunId,
    },
    Export {
        run_id: annotagent_core::RunId,
        #[arg(long)]
        output: PathBuf,
    },
    Import {
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();
    let cli = Cli::parse();
    match cli.command {
        Command::Init {
            project_directory,
            skill,
        } => init_project(&project_directory, &skill),
        Command::Project {
            command: ProjectCommand::Validate { project },
        } => validate_project(&project),
        Command::Import { project, images } => import_images(&project, &images),
        Command::ImportAnnotations {
            project,
            format,
            source,
            dry_run,
            label_mapping,
        } => import_annotations(&project, &format, &source, dry_run, &label_mapping).await,
        Command::Run(arguments) => run_command(&arguments).await,
        Command::Tui { project } => tui::run(project).await,
        Command::Serve {
            workspace,
            open,
            port,
        } => serve_command(&workspace, port, open).await,
        Command::Skills { command } => skills_command(command),
        Command::Plugin { data_dir, command } => plugin_cli::run(command, data_dir).await,
        Command::Models { workspace, command } => model_cli::run(&workspace, command).await,
        Command::History { command } => history_command(command),
        Command::Export {
            project,
            format,
            output,
        } => export_command(&project, &format, &output).await,
        Command::Evaluate {
            ground_truth,
            predictions,
            output,
            bbox_iou_threshold,
            minimum_field_region_iou,
        } => evaluate_command(
            &ground_truth,
            &predictions,
            output.as_deref(),
            bbox_iou_threshold,
            minimum_field_region_iou,
        ),
        Command::Doctor => doctor(),
        Command::Demo { name } => demo::run(&name).await,
    }
}

async fn import_annotations(
    project_path: &Path,
    format: &str,
    source: &Path,
    dry_run: bool,
    mappings: &[String],
) -> Result<()> {
    let project_path = project_path.canonicalize()?;
    let project_directory = project_path
        .parent()
        .context("project path has no parent directory")?;
    let workspace = project_directory
        .parent()
        .context("project directory has no workspace parent")?;
    let project_id = project_directory
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .context("project directory name is not valid UTF-8")?;
    let mut label_mapping = std::collections::BTreeMap::new();
    for mapping in mappings {
        let (source, target) = mapping.split_once('=').with_context(|| {
            format!("invalid label mapping {mapping:?}; expected SOURCE=TARGET")
        })?;
        if source.is_empty() || target.is_empty() {
            bail!("invalid label mapping {mapping:?}; labels cannot be empty");
        }
        label_mapping.insert(source.to_owned(), target.to_owned());
    }
    let application = LocalApplication::new(workspace)?;
    let report = application
        .import_project_annotations(project_id, format, source, label_mapping, dry_run)
        .await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn evaluate_command(
    ground_truth_path: &Path,
    predictions_path: &Path,
    output: Option<&Path>,
    bbox_iou_threshold: f64,
    minimum_field_region_iou: Option<f64>,
) -> Result<()> {
    let ground_truth: annotagent_skill_robocup::EvaluationGroundTruth = serde_json::from_slice(
        &std::fs::read(ground_truth_path)
            .with_context(|| format!("cannot read {}", ground_truth_path.display()))?,
    )
    .with_context(|| format!("invalid ground truth {}", ground_truth_path.display()))?;
    let predictions: annotagent_skill_robocup::EvaluationPredictions = serde_json::from_slice(
        &std::fs::read(predictions_path)
            .with_context(|| format!("cannot read {}", predictions_path.display()))?,
    )
    .with_context(|| format!("invalid predictions {}", predictions_path.display()))?;
    let report = annotagent_skill_robocup::evaluate_with_thresholds(
        &ground_truth,
        &predictions,
        annotagent_skill_robocup::EvaluationThresholds {
            bbox_iou: bbox_iou_threshold,
            minimum_field_region_mask_iou: minimum_field_region_iou,
        },
    )
    .map_err(|error| anyhow::anyhow!(error))?;
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(path) = output {
        std::fs::write(path, format!("{json}\n"))
            .with_context(|| format!("cannot write {}", path.display()))?;
        println!("wrote evaluation report to {}", path.display());
    } else {
        println!("{json}");
    }
    Ok(())
}

fn init_project(directory: &Path, skill: &str) -> Result<()> {
    if skill != "robocup" {
        bail!("only the production robocup skill is bundled in this release");
    }
    std::fs::create_dir_all(directory.join("images"))
        .with_context(|| format!("cannot create {}", directory.display()))?;
    let project_file = directory.join("project.yaml");
    if project_file.exists() {
        bail!(
            "{} already exists; refusing to overwrite it",
            project_file.display()
        );
    }
    std::fs::write(
        &project_file,
        include_str!("../../../examples/robocup/project.yaml"),
    )?;
    annotagent_image_tools::generate_synthetic_robocup(&directory.join("images/demo.png"))
        .map_err(|error| anyhow::anyhow!(error))?;
    println!("created AnnotAgent project at {}", directory.display());
    println!(
        "next: annotagent project validate {}",
        project_file.display()
    );
    Ok(())
}

fn validate_project(path: &Path) -> Result<()> {
    let (project, skill) = runner::load_project(path)?;
    println!(
        "valid: schema v{}, project {:?}, skill {} v{}, {} tasks",
        project.version,
        project.project.name,
        skill.id(),
        project.project.skill_version,
        project.tasks.len()
    );
    Ok(())
}

fn import_images(project_path: &Path, images: &Path) -> Result<()> {
    let (project, _) = runner::load_project(project_path)?;
    let project_root = project_path.parent().unwrap_or_else(|| Path::new("."));
    let destination = project_root.join(project.dataset.root);
    std::fs::create_dir_all(&destination)?;
    let canonical_source = images
        .canonicalize()
        .with_context(|| format!("cannot access {}", images.display()))?;
    if canonical_source.is_file()
        && canonical_source
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        bail!(
            "ZIP image import is not supported; archives are rejected before extraction to prevent path traversal"
        );
    }
    let mut imported = 0_u64;
    let mut duplicates = 0_u64;
    let mut known_hashes = std::collections::BTreeSet::new();
    for existing in WalkDir::new(&destination)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|path| runner::is_supported_image(path))
    {
        if let Ok(bytes) = std::fs::read(existing) {
            known_hashes.insert(annotagent_image_tools::sha256(&bytes));
        }
    }
    for source in WalkDir::new(&canonical_source)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .filter(|path| runner::is_supported_image(path))
    {
        let bytes = std::fs::read(&source)?;
        if !known_hashes.insert(annotagent_image_tools::sha256(&bytes)) {
            duplicates += 1;
            continue;
        }
        let file_name = source
            .file_name()
            .context("source image has no file name")?;
        let mut target = destination.join(file_name);
        if target.exists() {
            target = destination.join(format!(
                "{}-{}",
                uuid::Uuid::new_v4(),
                file_name.to_string_lossy()
            ));
        }
        std::fs::copy(&source, &target)?;
        imported += 1;
    }
    println!("imported {imported} image(s); skipped {duplicates} duplicate(s)");
    Ok(())
}

async fn run_command(arguments: &RunArgs) -> Result<()> {
    if arguments.limit.is_some_and(|limit| limit == 0) {
        bail!("--limit must be greater than zero");
    }
    let workspace = arguments
        .project
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .canonicalize()?;
    let application = LocalApplication::with_database(workspace, runner::database_path()?)?;
    if arguments.limit == Some(1) {
        let prepared = application.start_run_path(
            &arguments.project,
            &arguments.provider,
            arguments.config.as_deref(),
        )?;
        println!(
            "run {}: image {}",
            prepared.run_id,
            prepared.image_path.display()
        );
        let result = tokio::select! {
            result = application.wait_run(prepared.run_id) => result?,
            signal = tokio::signal::ctrl_c() => {
                signal.context("cannot listen for Ctrl-C")?;
                eprintln!("cancellation requested; waiting for the active model call to stop safely");
                application.cancel_run(prepared.run_id).await?;
                application.wait_run(prepared.run_id).await?
            }
        };
        print_image_result(&result);
        return Ok(());
    }

    let coordinator = DatasetCoordinator::new(&application);
    let results = coordinator
        .run(
            &arguments.project,
            &arguments.provider,
            arguments.config.as_deref(),
            arguments.limit,
        )
        .await?;
    for item in results {
        println!("image {}", item.image_path.display());
        print_image_result(&item.result);
    }
    Ok(())
}

fn print_image_result(result: &annotagent_runtime::ImageRunResult) {
    println!(
        "status={:?} committed={} review={} issues={} tokens={}/{} requests={} cost={}",
        result.status,
        result.committed.len(),
        result.review_queue.len(),
        result.issues.len(),
        result.usage.input_tokens,
        result.usage.output_tokens,
        result.usage.requests,
        result.usage.cost
    );
    for issue in &result.issues {
        println!("issue {}: {}", issue.code, issue.message);
    }
}

fn skills_command(command: SkillsCommand) -> Result<()> {
    let skill = RoboCupSkill::new().map_err(|error| anyhow::anyhow!(error))?;
    match command {
        SkillsCommand::List => println!(
            "{}\t{}\t{}",
            annotagent_core::DomainSkill::id(&skill),
            skill.manifest().display_name,
            skill.manifest().description
        ),
        SkillsCommand::Show { skill_id } => {
            if skill_id != annotagent_core::DomainSkill::id(&skill) {
                bail!("unknown skill {skill_id:?}; installed skill: robocup");
            }
            println!("{}", serde_json::to_string_pretty(skill.manifest())?);
            println!(
                "tools: {}",
                skill
                    .tool_factories()
                    .iter()
                    .map(|tool| tool.definition().name)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(
                "validators: {}",
                skill
                    .validators()
                    .iter()
                    .map(|validator| validator.id())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            println!(
                "refiners: {}",
                skill
                    .refiners()
                    .iter()
                    .map(|refiner| refiner.id())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    Ok(())
}

fn history_command(command: HistoryCommand) -> Result<()> {
    let database = runner::database_path()?;
    let store = SqliteStore::open(&database)
        .with_context(|| format!("cannot open history database {}", database.display()))?;
    match command {
        HistoryCommand::List => {
            for run in store.list_runs()? {
                println!(
                    "{}\t{:?}\t{}\t{}\t{}",
                    run.id, run.status, run.project_name, run.skill_id, run.updated_at
                );
            }
        }
        HistoryCommand::Show { run_id } => {
            println!("{}", serde_json::to_string_pretty(&store.history(run_id)?)?);
        }
        HistoryCommand::Export { run_id, output } => {
            store.export_history(run_id, &output)?;
            println!("exported run {run_id} to {}", output.display());
        }
        HistoryCommand::Import { file } => {
            let document: HistoryDocument = serde_json::from_slice(&std::fs::read(&file)?)?;
            let report = store.import_history(document)?;
            println!(
                "imported run {} (ids_remapped={})",
                report.run_id, report.ids_remapped
            );
            for warning in report.warnings {
                println!("warning: {warning}");
            }
        }
    }
    Ok(())
}

async fn export_command(project_path: &Path, format: &str, output: &Path) -> Result<()> {
    let (project, _) = runner::load_project(project_path)?;
    let database = runner::database_path()?;
    let store = SqliteStore::open(&database)?;
    let run = store
        .list_runs()?
        .into_iter()
        .find(|run| run.project_name == project.project.name)
        .context("no completed run for this project; run annotation first")?;
    let annotations = store.list_annotations(run.id)?;
    let first = annotations
        .first()
        .context("latest run has no annotations")?;
    let project_root = project_path.parent().unwrap_or_else(|| Path::new("."));
    let image_path = WalkDir::new(project_root.join(&project.dataset.root))
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
        .find(|path| runner::is_supported_image(path))
        .context("project has no image file")?;
    let frame = annotagent_image_tools::load_image(&image_path, 40_000_000)
        .map_err(|error| anyhow::anyhow!(error))?;
    let relative_path = image_path
        .strip_prefix(project_root)
        .unwrap_or(&image_path)
        .to_path_buf();
    let snapshot = ProjectSnapshot {
        schema: project,
        images: vec![SnapshotImage {
            id: first.image_id,
            relative_path,
            metadata: frame.metadata,
        }],
        annotations,
        revisions: store.history(run.id)?.revisions,
    };
    let exporter: Arc<dyn DatasetExporter> = match format {
        "native" => Arc::new(NativeExporter),
        "coco" => Arc::new(CocoExporter),
        "yolo" | "yolo_detection" => Arc::new(YoloDetectionExporter),
        "yolo_segmentation" => Arc::new(YoloSegmentationExporter),
        "labelme" => Arc::new(LabelMeExporter),
        other => bail!(
            "unknown export format {other:?}; choose native, coco, yolo, yolo_segmentation, or labelme"
        ),
    };
    let report = exporter
        .export(annotagent_core::ExportRequest {
            project: snapshot,
            output: output.to_path_buf(),
        })
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn doctor() -> Result<()> {
    let settings = runner::load_settings(None)?;
    let database = runner::database_path()?;
    if let Some(parent) = database.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let store = SqliteStore::open(&database)?;
    let skill = RoboCupSkill::new().map_err(|error| anyhow::anyhow!(error))?;
    let project = Path::new("examples/robocup/project.yaml");
    let project_status = if project.exists() {
        runner::load_project(project).map_or("invalid", |_| "ok")
    } else {
        "missing"
    };
    println!("config: ok (model={})", settings.provider.model);
    println!(
        "workspace: writable ({})",
        std::env::current_dir()?.display()
    );
    println!("SQLite: ok ({} tables)", store.schema_tables()?.len());
    println!("migrations: ok");
    println!(
        "skill: {} ({})",
        skill.manifest().id,
        skill.manifest().display_name
    );
    println!(
        "provider key env {}: {}",
        settings.provider.api_key_env,
        if std::env::var_os(&settings.provider.api_key_env).is_some() {
            "set"
        } else {
            "not set (live Provider runs require a configured credential)"
        }
    );
    println!("example project: {project_status}");
    println!(
        "web build: {}",
        if Path::new("web/dist/index.html").exists() {
            "present"
        } else {
            "missing (run npm --prefix web run build)"
        }
    );
    let port_status = match std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 8787)) {
        Ok(listener) => {
            drop(listener);
            "available"
        }
        Err(_) => "in use",
    };
    println!("port 127.0.0.1:8787: {port_status}");
    Ok(())
}

async fn serve_command(workspace: &Path, port: u16, open: bool) -> Result<()> {
    let application = Arc::new(LocalApplication::new(workspace)?);
    let state = annotagent_server::ServerState::new(application).await?;
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let url = format!("http://{address}");
    println!("AnnotAgent GUI: {url}");
    println!("workspace: {}", workspace.display());
    if open {
        webbrowser::open(&url)?;
    }
    annotagent_server::serve(state, address, Some(Path::new("web/dist"))).await
}
