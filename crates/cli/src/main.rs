use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

mod commands;

#[derive(Parser)]
#[command(name = "mango", version, about = "Mango - 漫剧创作工作站 CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Database file path. Defaults to the same location used by the desktop
    /// app (`<data_dir>/com.mango.app/mango.db`).
    #[arg(long, global = true)]
    db: Option<PathBuf>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Project management
    Project(commands::project::ProjectArgs),
    /// Character management
    Character(commands::character::CharacterArgs),
    /// API account management (keyring-backed)
    Account(commands::account::AccountArgs),
    /// AI generation tasks (submit / status / list / cancel)
    Task(commands::task::TaskArgs),
}

fn main() {
    if let Err(e) = run() {
        // Print only the Display chain (not the Debug backtrace) so user-facing
        // errors stay clean even when RUST_BACKTRACE is set.
        eprintln!("Error: {e}");
        for cause in e.chain().skip(1) {
            eprintln!("  caused by: {cause}");
        }
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let app_data_dir = default_app_data_dir()?;
    let db_path = match cli.db {
        Some(path) => path,
        None => app_data_dir.join("mango.db"),
    };

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = mango_core::db::open_sync(&db_path)?;
    mango_core::startup::initialize(&conn, &app_data_dir)?;

    match cli.command {
        Commands::Project(args) => commands::project::execute(&conn, &app_data_dir, args)?,
        Commands::Character(args) => commands::character::execute(&conn, args)?,
        Commands::Account(args) => commands::account::execute(&conn, args)?,
        Commands::Task(args) => commands::task::execute(&conn, &app_data_dir, args)?,
    }

    Ok(())
}

/// Resolves the default app data directory. Must stay aligned with the Tauri
/// app's `app.path().app_data_dir()` so GUI and CLI share the same SQLite file
/// and convention project paths.
///
/// - macOS:   ~/Library/Application Support/com.mango.app
/// - Linux:   ~/.local/share/com.mango.app
/// - Windows: %APPDATA%\com.mango.app
fn default_app_data_dir() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine system data directory"))?;
    Ok(data_dir.join("com.mango.app"))
}
