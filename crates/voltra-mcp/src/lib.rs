//! An MCP server for a Voltra project.
//!
//! What an AI client needs to work in a project and check its own work:
//! the components there are and their templates, the assets and scenes on
//! disk, and — in later stages — editing scenes, running them headless and
//! looking at a screenshot (`docs/plan-mcp.md`, M2). It works on the
//! project's files, with no editor open; M3 adds a live editor behind the
//! same tools.
//!
//! **Local only.** It speaks over stdio to the client that started it, and
//! every path it is given is an asset path inside the project's asset root
//! — a path that would leave it, by `..`, an absolute path or a link, is
//! refused.

pub mod project;
pub mod server;
pub mod tools;

pub use project::Project;
pub use server::VoltraMcp;
