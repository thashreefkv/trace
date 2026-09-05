//! Standalone eval runner — runs all enabled fixtures against a Tauri-app
//! DB and exits non-zero on regression. Designed for CI integration:
//! `pnpm eval -- --json | jq` produces machine-readable output.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use project_manager_shared::{
    brain::BRAIN_FILE_NAME,
    db::{connect_path, DB_FILE_NAME},
    eval,
    runtime,
};
use serde_json::json;

const TAURI_IDENTIFIER: &str = "com.thash.project-manager";

#[derive(Parser, Debug)]
#[command(
    about = "Run all enabled Trace eval fixtures and exit non-zero on regression."
)]
struct Args {
    /// SQLite DB path. Defaults to the standard Tauri app data location.
    #[arg(long)]
    db: Option<PathBuf>,
    /// Brain kuzu path. Defaults relative to db.
    #[arg(long)]
    brain: Option<PathBuf>,
    /// Regression threshold (delta below baseline that fails the run).
    /// Default -0.05 = a 5-point precision drop fails CI.
    #[arg(long, default_value_t = -0.05)]
    threshold_delta: f64,
    /// Emit machine-readable JSON to stdout instead of human-readable text.
    #[arg(long)]
    json: bool,
    /// Gemini API key for Ask/judge evals. Falls back to env GEMINI_API_KEY.
    #[arg(long)]
    api_key: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    let (db_path, brain_path) = match resolve_paths(&args) {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("[eval-runner] {error}");
            return ExitCode::from(2);
        }
    };

    if !db_path.exists() {
        eprintln!(
            "[eval-runner] DB not found at {} — run the app at least once to create it.",
            db_path.display()
        );
        return ExitCode::from(2);
    }

    let api_key = args
        .api_key
        .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        .filter(|s| !s.trim().is_empty());
    runtime::set_gemini_api_key(api_key);

    let pool = match connect_path(&db_path).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("[eval-runner] connect: {error}");
            return ExitCode::from(2);
        }
    };

    let runs = match eval::run_all(&pool, &brain_path).await {
        Ok(runs) => runs,
        Err(error) => {
            eprintln!("[eval-runner] run_all: {error}");
            return ExitCode::from(2);
        }
    };

    let mut regressions = Vec::new();
    let mut passed = 0_usize;
    let mut failed = 0_usize;
    for run in &runs {
        let delta_bad = run
            .delta
            .map(|d| d < args.threshold_delta)
            .unwrap_or(false);
        let regressed = !run.passed || delta_bad;
        if regressed {
            failed += 1;
            regressions.push(json!({
                "fixture_id": run.fixture_id,
                "score": run.score,
                "metric": run.metric,
                "baseline": run.baseline_score,
                "delta": run.delta,
                "passed": run.passed,
            }));
        } else {
            passed += 1;
        }
    }

    if args.json {
        let payload = json!({
            "total": runs.len(),
            "passed": passed,
            "failed": failed,
            "threshold_delta": args.threshold_delta,
            "regressions": regressions,
            "runs": runs,
        });
        println!(
            "{}",
            serde_json::to_string(&payload).unwrap_or_else(|_| "{}".into())
        );
    } else {
        print_human(&runs, args.threshold_delta);
        eprintln!(
            "\n{} passed, {} failed (threshold {:+.2})",
            passed, failed, args.threshold_delta
        );
    }

    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

fn resolve_paths(args: &Args) -> Result<(PathBuf, PathBuf), String> {
    let db_path = match &args.db {
        Some(path) => path.clone(),
        None => default_db_path()?,
    };
    let brain_path = match &args.brain {
        Some(path) => path.clone(),
        None => {
            let parent = db_path
                .parent()
                .ok_or_else(|| "DB path has no parent directory".to_string())?;
            parent.join(BRAIN_FILE_NAME)
        }
    };
    Ok((db_path, brain_path))
}

fn default_db_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir().ok_or_else(|| {
        "could not resolve user config dir (pass --db explicitly)".to_string()
    })?;
    Ok(base.join(TAURI_IDENTIFIER).join(DB_FILE_NAME))
}

fn print_human(runs: &[project_manager_shared::eval::EvalRun], threshold: f64) {
    if runs.is_empty() {
        println!("No fixtures to run.");
        return;
    }
    for run in runs {
        let mark = if run.passed { "✓" } else { "✗" };
        let delta_part = match run.delta {
            Some(d) if d.abs() >= 0.001 => format!("  Δ {:+.2}", d),
            _ => String::new(),
        };
        let regressed = run
            .delta
            .map(|d| d < threshold)
            .unwrap_or(false)
            || !run.passed;
        let suffix = if regressed { "  REGRESSION" } else { "" };
        println!(
            "{} {:<32} {:>6.2}  {}{}{}",
            mark, run.fixture_id, run.score, run.metric, delta_part, suffix
        );
    }
}
