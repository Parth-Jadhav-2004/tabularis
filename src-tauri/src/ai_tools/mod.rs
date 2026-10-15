//! Shared database tool implementations, extracted from the MCP transport.
//! MCP compatibility handlers and a connection-scoped, typed chat registry.
pub mod approval;
mod audit;
mod connections;
mod cooldown;
mod database;
mod discovery;
mod dispatch;
pub(crate) mod output;
pub mod preflight;
mod registry;
pub mod safety;
mod tools;
pub mod truncate;
mod types;

pub use registry::ToolRegistry;
pub use types::{RowsPreview, Tool, ToolContext, ToolError, ToolMode, ToolOrigin, ToolOutput};

pub(crate) use connections::{resolve_db_driver, resolve_default_schema};
pub(crate) use discovery::handle_list_tools;
pub(crate) use dispatch::handle_call_tool;

#[cfg(test)]
mod output_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use audit::CallAudit;
#[cfg(test)]
pub(crate) use connections::{apply_database_override, database_arg, resolve_driver_for_params};
#[cfg(test)]
pub(crate) use dispatch::dispatch_tool;
#[cfg(test)]
pub(crate) use output::{requested_output_format, ToolOutputFormat};
