//! Authentication and portfolio commands.

use super::args::*;
use super::shared::*;
use anyhow::Result;

pub async fn cmd_discover(args: DiscoverArgs) -> Result<()> {
    let (output, client) = super::shared::make_client(args.json, args.color)?;

    let discovery = client.discover()?;
    output.print_result(&serde_json::Value::Object(discovery))?;
    Ok(())
}

pub async fn cmd_list_portfolios(args: ListPortfoliosArgs, positionals: &[String]) -> Result<()> {
    let (output, client) = super::shared::make_client(args.json, args.color)?;

    let listing = client.list_portfolios()?;
    let items = filter_by_substring(
        listing,
        args.filter
            .as_deref()
            .or_else(|| positionals.first().map(String::as_str)),
        &["name", "key"],
    );

    let result = Listing { items, page: None };
    print_listing(&output, result, PORTFOLIO_TABLE_COLUMNS)
}
