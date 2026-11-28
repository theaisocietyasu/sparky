//! SparkyAI engine: agent, HTTP surface, and store adapters behind the harness traits.

mod core;
mod routes;
mod runtime;
mod stores;
mod wiring;

/// Starts telemetry outside the tokio runtime, serves, then drops the runtime first.
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [name, rest @ ..] = args.as_slice()
        && name == runtime::tools::sandbox::wasm::runner::SUBCOMMAND
    {
        std::process::exit(runtime::tools::sandbox::wasm::runner::main(rest));
    }
    if !args.is_empty() {
        anyhow::bail!("usage: engine");
    }
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
    let done = runtime.block_on(wiring::serve(cfg));
    drop(runtime);
    drop(guard);
    done
}
