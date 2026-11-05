//! The child process of WasmSandbox: loads the packages and runs one bash command over a folder.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use wasmer_config::package::PackageId;
use wasmer_wasix::PluggableRuntime;
use wasmer_wasix::UnsupportedVirtualNetworking;
use wasmer_wasix::bin_factory::BinaryPackage;
use wasmer_wasix::runners::MappedDirectory;
use wasmer_wasix::runners::wasi::{RuntimeOrEngine, WasiRunner};
use wasmer_wasix::runtime::module_cache::{FileSystemCache, ModuleCache, SharedCache};
use wasmer_wasix::runtime::package_loader::BuiltinPackageLoader;
use wasmer_wasix::runtime::resolver::{InMemorySource, PackageSummary};
use wasmer_wasix::runtime::task_manager::tokio::TokioTaskManager;
use wasmer_wasix::virtual_fs::{
    ArcFile, AsyncReadExt, AsyncSeekExt, BufferFile, NullFile, VirtualFile,
};

/// Subcommand the engine runs itself under to be the child.
pub const SUBCOMMAND: &str = "sandbox-run";

/// Guest path the student folder is mapped to.
pub const GUEST_WORK: &str = "/work";

/// Package that runs the command. Every other package is put on its PATH.
const SHELL: &str = "wasmer/bash";

/// Exit status of a child that could not run the command at all.
pub const SETUP_FAILED: i32 = 125;

/// Why the command did not run.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The packages or the runtime could not be set up.
    #[error("{0}")]
    Setup(String),
}

fn setup(e: impl std::fmt::Display) -> RunError {
    RunError::Setup(e.to_string())
}

/// The registry name and version of a file named namespace__name@version.webc.
pub fn package_id(path: &Path) -> Option<(String, semver::Version)> {
    let stem = path.file_name()?.to_str()?.strip_suffix(".webc")?;
    let (name, version) = stem.split_once('@')?;
    let (namespace, name) = name.split_once("__")?;
    Some((
        format!("{namespace}/{name}"),
        semver::Version::parse(version).ok()?,
    ))
}

/// What the command printed and how it ended.
#[derive(Debug)]
pub struct Ran {
    /// Exit status of the command.
    pub exit_code: i32,
    /// Standard output, cut at the cap.
    pub stdout: Vec<u8>,
    /// Standard error, cut at the cap.
    pub stderr: Vec<u8>,
}

/// Runs the child: args are packages, cache and work folders and an output cap; stdin is the command.
pub fn main(args: &[String]) -> i32 {
    let [packages, cache, work, cap] = args else {
        let _ = writeln!(
            std::io::stderr(),
            "usage: {SUBCOMMAND} PACKAGES CACHE WORK CAP"
        );
        return SETUP_FAILED;
    };
    let mut command = String::new();
    if std::io::stdin().read_to_string(&mut command).is_err() {
        return SETUP_FAILED;
    }
    let cap = cap.parse().unwrap_or(16_384);
    match run(
        Path::new(packages),
        Path::new(cache),
        Path::new(work),
        &command,
        cap,
    ) {
        Ok(ran) => {
            let _ = std::io::stdout().write_all(&ran.stdout);
            let _ = std::io::stderr().write_all(&ran.stderr);
            ran.exit_code
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "sandbox: {e}");
            SETUP_FAILED
        }
    }
}

/// Runs command with bash over work, every package in packages on its PATH and no network.
pub fn run(
    packages: &Path,
    cache: &Path,
    work: &Path,
    command: &str,
    cap: usize,
) -> Result<Ran, RunError> {
    let tokio = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(setup)?;
    tokio.block_on(run_in(packages, cache, work, command, cap))
}

/// The packages in folder by registry name, loaded into the runtime source and loader.
fn load(folder: &Path, rt: &mut PluggableRuntime) -> Result<HashMap<String, PathBuf>, RunError> {
    let mut source = InMemorySource::new();
    let loader = BuiltinPackageLoader::new();
    let mut files = HashMap::new();
    for entry in std::fs::read_dir(folder).map_err(setup)? {
        let path = entry.map_err(setup)?.path();
        let Some((name, version)) = package_id(&path) else {
            continue;
        };
        let mut summary = PackageSummary::from_webc_file(&path).map_err(setup)?;
        summary.pkg.id = PackageId::new_named(name.clone(), version);
        let container = wasmer_package::utils::from_disk(&path).map_err(setup)?;
        loader.insert_cached(summary.dist.webc_sha256, &container);
        source.add(summary);
        files.insert(name, path);
    }
    rt.set_source(source);
    rt.set_package_loader(loader);
    Ok(files)
}

async fn run_in(
    packages: &Path,
    cache: &Path,
    work: &Path,
    command: &str,
    cap: usize,
) -> Result<Ran, RunError> {
    let tasks = Arc::new(TokioTaskManager::new(tokio::runtime::Handle::current()));
    let mut rt = PluggableRuntime::new(Arc::clone(&tasks) as Arc<_>);
    rt.set_engine(
        wasmer::sys::EngineBuilder::new(wasmer::sys::Cranelift::default())
            .engine()
            .into(),
    );
    rt.set_networking_implementation(UnsupportedVirtualNetworking::default());
    rt.set_module_cache(
        SharedCache::default().with_fallback(FileSystemCache::new(cache, tasks.clone())),
    );
    let files = load(packages, &mut rt)?;
    let shell_file = files
        .get(SHELL)
        .ok_or_else(|| RunError::Setup(format!("no {SHELL} package in {}", packages.display())))?;
    let open = |path: &Path| wasmer_package::utils::from_disk(path).map_err(setup);
    let shell = BinaryPackage::from_webc(&open(shell_file)?, &rt)
        .await
        .map_err(setup)?;
    let mut tools = Vec::new();
    for (name, path) in &files {
        if name != SHELL {
            tools.push(
                BinaryPackage::from_webc(&open(path)?, &rt)
                    .await
                    .map_err(setup)?,
            );
        }
    }
    let stdout = ArcFile::new(Box::<BufferFile>::default());
    let stderr = ArcFile::new(Box::<BufferFile>::default());
    let (out, err) = (stdout.clone(), stderr.clone());
    // bash in WASIX does not read the environment it is given, so the script sets it.
    let script =
        format!("export HOME={GUEST_WORK} PYTHON_COLORS=0 NO_COLOR=1 TERM=dumb\n{command}");
    let host = work.to_path_buf();
    let rt = Arc::new(rt);
    let ended = tokio::task::spawn_blocking(move || {
        WasiRunner::new()
            .with_args(["-c", script.as_str()])
            .with_injected_packages(tools)
            .with_mapped_directories([MappedDirectory {
                host,
                guest: GUEST_WORK.into(),
            }])
            .with_current_dir(GUEST_WORK)
            .with_stdin(Box::<NullFile>::default())
            .with_stdout(Box::new(out) as Box<dyn VirtualFile + Send + Sync>)
            .with_stderr(Box::new(err) as Box<dyn VirtualFile + Send + Sync>)
            .run_command("bash", &shell, RuntimeOrEngine::Runtime(rt))
    })
    .await
    .map_err(setup)?;
    let exit_code = match ended {
        Ok(()) => 0,
        Err(e) => exit_code(&e).ok_or_else(|| setup(format!("{e:#}")))?,
    };
    Ok(Ran {
        exit_code,
        stdout: drain(stdout, cap).await,
        stderr: drain(stderr, cap).await,
    })
}

/// The exit status a run that ended with a non-zero code carries.
fn exit_code(error: &anyhow::Error) -> Option<i32> {
    error.chain().find_map(|cause| {
        cause
            .downcast_ref::<wasmer_wasix::WasiError>()
            .and_then(|e| match e {
                wasmer_wasix::WasiError::Exit(code) => Some(code.raw()),
                _ => None,
            })
            .or_else(|| {
                cause
                    .downcast_ref::<wasmer_wasix::WasiRuntimeError>()
                    .and_then(wasmer_wasix::WasiRuntimeError::as_exit_code)
                    .map(|code| code.raw())
            })
    })
}

/// The first cap bytes written to file.
async fn drain(mut file: ArcFile<BufferFile>, cap: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    if file.rewind().await.is_ok() {
        let _ = file.read_to_end(&mut bytes).await;
    }
    bytes.truncate(cap);
    bytes
}
