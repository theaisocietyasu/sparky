//! SparkyAI engine: agent, HTTP surface, and store adapters behind the harness traits.

mod core;
mod routes;
mod runtime;
mod stores;
mod wiring;

/// What the engine runs, from its arguments.
enum Command {
    /// Serve HTTP until shutdown.
    Serve,
    /// Copy the platform agents export into the SQLite file; again allows a second run.
    ImportPlatform { again: bool },
}

impl Command {
    /// The command named by args, without the program name.
    fn parse(args: &[String]) -> anyhow::Result<Self> {
        match args {
            [] => Ok(Self::Serve),
            [name, rest @ ..] if name == "import-platform" => match rest {
                [] => Ok(Self::ImportPlatform { again: false }),
                [flag] if flag == "--again" => Ok(Self::ImportPlatform { again: true }),
                _ => anyhow::bail!("usage: engine import-platform [--again]"),
            },
            _ => anyhow::bail!("usage: engine [import-platform [--again]]"),
        }
    }
}

/// Starts telemetry outside the tokio runtime, runs the command, then drops the runtime first.
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [name, rest @ ..] = args.as_slice()
        && name == runtime::tools::sandbox::wasm::runner::SUBCOMMAND
    {
        std::process::exit(runtime::tools::sandbox::wasm::runner::main(rest));
    }
    let command = Command::parse(&args)?;
    if let Err(e) = dotenvy::dotenv()
        && !e.not_found()
    {
        return Err(anyhow::anyhow!(".env: {e}"));
    }
    let cfg = core::config::Config::load()?;
    let guard = core::telemetry::init(&cfg.telemetry, "engine", &cfg.app.env, &cfg.app.log_level)?;
    tracing::info!(env = %cfg.app.env, "engine starting");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let done = match command {
        Command::Serve => runtime.block_on(wiring::serve(cfg)),
        Command::ImportPlatform { again } => runtime.block_on(wiring::import_platform(&cfg, again)),
    };
    drop(runtime);
    drop(guard);
    done
}
