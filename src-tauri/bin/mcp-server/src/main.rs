mod server;
mod tools;

use std::path::PathBuf;

use anyhow::{bail, Context};
use rmcp::ServiceExt;

use server::TraceMcpServer;

#[derive(Debug)]
struct CliOptions {
    db: PathBuf,
    app_support_dir: PathBuf,
}

impl CliOptions {
    fn parse() -> anyhow::Result<Self> {
        let mut args = std::env::args().skip(1);
        let mut db = None;
        let mut app_support_dir = None;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--db" => db = args.next().map(PathBuf::from),
                "--app-support-dir" => app_support_dir = args.next().map(PathBuf::from),
                "--help" | "-h" => {
                    bail!(
                        "usage: mcp-server --db <absolute path> --app-support-dir <absolute path>"
                    );
                }
                other => bail!("unknown argument: {other}"),
            }
        }

        let db = db.context("--db is required")?;
        let app_support_dir = app_support_dir.context("--app-support-dir is required")?;
        if !db.is_absolute() {
            bail!("--db must be an absolute path");
        }
        if !app_support_dir.is_absolute() {
            bail!("--app-support-dir must be an absolute path");
        }

        Ok(Self {
            db,
            app_support_dir,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let options = CliOptions::parse()?;
    let pool = project_manager_shared::db::connect_path(&options.db)
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    let server = TraceMcpServer::new(pool, options.app_support_dir);

    server
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|error| anyhow::anyhow!(error))?
        .waiting()
        .await
        .map_err(|error| anyhow::anyhow!(error))?;

    Ok(())
}
