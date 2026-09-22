//! Authentication and portfolio commands.

use super::args::*;
use super::shared::*;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use std::sync::Arc;

pub async fn cmd_discover(args: DiscoverArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let discovery = client.discover()?;
    output.print_result(&serde_json::Value::Object(discovery))?;
    Ok(())
}

pub async fn cmd_list_portfolios(args: ListPortfoliosArgs, positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let listing = client.list_portfolios()?;
    let items = filter_by_substring(
        listing,
        args.filter.as_deref().or_else(|| positionals.first().map(String::as_str)),
        &["name", "key"],
    );

    let result = Listing { items, page: None };
    print_listing(&output, result, PORTFOLIO_TABLE_COLUMNS)
}
