//! OxyCharacter plugins: separate programs (usually Python, with the SDK in
//! `python/oxycharacter`) that the editor or the `oxy` CLI starts on request
//! and talks to over the child's **stdin/stdout** — no sockets, no ports.
//!
//! * [`protocol`] — JSON-RPC 2.0 messages framed with `Content-Length`
//!   headers (as in LSP and OxyBrush's stdio MCP server). The plugin sends
//!   requests; the host answers each one.
//! * [`api`] — API version 1: the [`api::Host`] trait a host implements and
//!   [`api::dispatch`], which maps method names to it. Hosts never parse
//!   requests themselves, so the editor and the CLI answer identically.
//! * [`runner`] — starting a plugin process, discovering plugins in a folder,
//!   and a blocking run loop for headless hosts.
//!
//! The API is OxyCharacter's own. It is not compatible with, and does not
//! imitate, any other product's scripting interface (see ADR 0011).

pub mod api;
pub mod protocol;
pub mod runner;

pub use api::{dispatch, Host, RpcError, API_VERSION};
