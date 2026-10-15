//! Shared database tool implementations, extracted from the MCP transport.
//! The typed chat registry and stricter chat policy are the next M3 step.
mod audit;
mod connections;
mod cooldown;
mod discovery;
mod dispatch;
pub(crate) mod output;
pub mod preflight;
mod tools;

pub(crate) use connections::{resolve_db_driver, resolve_default_schema};
pub(crate) use discovery::handle_list_tools;
pub(crate) use dispatch::handle_call_tool;

#[cfg(test)]
mod output_tests;

#[cfg(test)]
pub(crate) use audit::CallAudit;
#[cfg(test)]
pub(crate) use connections::{apply_database_override, database_arg, resolve_driver_for_params};
#[cfg(test)]
pub(crate) use dispatch::dispatch_tool;
#[cfg(test)]
pub(crate) use output::{requested_output_format, ToolOutputFormat};
