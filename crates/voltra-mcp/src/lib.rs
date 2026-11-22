//! An MCP server for a Voltra project.
//!
//! What an AI client needs to work in a project and check its own work:
//! the components there are and their templates, the assets and scenes on
//! disk, editing scenes, running them headless and looking at a screenshot
//! (docs/MCP.md). It works on the project's files with no editor open, and
//! drives a running editor through `voltra-bridge` with its `editor_*`
//! tools.
//!
//! **Local only.** It speaks over stdio to the client that started it, to an
//! editor only on the loopback interface, and
//! every path it is given is an asset path inside the project's asset root
//! — a path that would leave it, by `..`, an absolute path or a link, is
//! refused.

pub mod project;
pub mod server;
pub mod tools;

pub use project::Project;
pub use server::VoltraMcp;
