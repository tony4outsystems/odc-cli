//! Authentication and portfolio commands.

use super::shared::*;
use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use std::sync::Arc;

pub async fn cmd_discover(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let discovery = client.discover()?;
    output.print_result(&serde_json::Value::Object(discovery))?;
    Ok(())
}

pub async fn cmd_list_portfolios(options: &Options, positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let mut listing = fetch_listing(
        options,
        |offset, limit| client.list_portfolios_page(offset, limit),
        || client.list_portfolios(),
    )?;
    listing.items = filter_by_substring(
        listing.items,
        positionals.first().map(String::as_str),
        &["name", "key"],
    );
    print_listing(&output, listing, PORTFOLIO_TABLE_COLUMNS)
}
