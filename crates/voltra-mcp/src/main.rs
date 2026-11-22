//! `voltra-mcp`: the MCP server for a Voltra project, over stdio.
//!
//! `voltra-mcp --project <dir> [--assets <dir>]` — the asset root is
//! `<project>/assets` unless `--assets` names another. The log goes to
//! stderr: stdout is the protocol's.

use std::path::PathBuf;

use rmcp::ServiceExt;
use voltra_mcp::{Project, VoltraMcp};

fn main() {
    voltra_core::logging::init(false);
    let Some(project) = parse(std::env::args().skip(1)) else {
        eprintln!("usage: voltra-mcp --project <dir> [--assets <dir>]");
        std::process::exit(2);
    };
    log::info!(
        "voltra-mcp serving {} (assets {})",
        project.root().display(),
        project.assets().display()
    );
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            log::error!("could not start the async runtime: {e}");
            std::process::exit(1);
        }
    };
    let served = runtime.block_on(async move {
        let service = VoltraMcp::new(project)
            .serve(rmcp::transport::stdio())
            .await?;
        service.waiting().await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    });
    if let Err(e) = served {
        log::error!("the MCP session ended with an error: {e}");
        std::process::exit(1);
    }
}

/// The project the arguments name, or `None` for arguments that do not
/// make sense.
fn parse(mut args: impl Iterator<Item = String>) -> Option<Project> {
    let mut root: Option<PathBuf> = None;
    let mut assets: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => root = Some(args.next()?.into()),
            "--assets" => assets = Some(args.next()?.into()),
            _ => return None,
        }
    }
    let root = root.or_else(|| std::env::current_dir().ok())?;
    Some(Project::new(root, assets))
}
