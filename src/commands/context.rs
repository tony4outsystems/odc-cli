//! Shared per-invocation context threaded through command handlers.
//!
//! `Ctx` replaces the repeated `make_client(args.json, args.color)` call (and the separate
//! `mentor_client` helper) that used to appear at the top of almost every handler. It carries
//! the global flags (`--json`, `--color`, `--no-resolve`) plus lazy constructors for the API
//! client and the Mentor client, both of which load settings and build an `Output` the same
//! way the old helpers did — so error ordering and messages are unchanged.

use crate::client::Client;
use crate::mentor::MentorClient;
use crate::output::{ColorMode, Output};
use anyhow::Result;
use std::sync::Arc;

pub struct Ctx {
    pub output: Arc<Output>,
    pub no_resolve: bool,
}

impl Ctx {
    pub fn new(json: bool, color: ColorMode, no_resolve: bool) -> Self {
        Self {
            output: Arc::new(Output::new(json, color)),
            no_resolve,
        }
    }

    pub fn json(&self) -> bool {
        self.output.json
    }

    /// Load settings and build an API client sharing this context's `Output`.
    pub fn client(&self) -> Result<Client> {
        let settings = crate::settings::load_settings()?;
        Client::new(settings, self.output.clone())
    }

    /// Load settings and build a Mentor MCP client.
    pub fn mentor(&self) -> Result<MentorClient> {
        let settings = crate::settings::load_settings()?;
        MentorClient::new(settings)
    }
}
