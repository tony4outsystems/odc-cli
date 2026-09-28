//! Authentication and portfolio commands.

use super::args::*;
use super::context::Ctx;
use super::shared::*;
use anyhow::Result;

pub async fn cmd_discover(ctx: &Ctx) -> Result<()> {
    let client = ctx.client()?;

    let discovery = client.discover()?;
    ctx.output
        .print_result(&serde_json::Value::Object(discovery))?;
    Ok(())
}

pub async fn cmd_list_portfolios(ctx: &Ctx, args: &ListPortfoliosArgs) -> Result<()> {
    let client = ctx.client()?;

    let mut listing = fetch_listing(
        args.offset,
        args.limit,
        |offset, limit| client.list_portfolios_page(offset, limit),
        || client.list_portfolios(),
    )?;
    listing.items = filter_by_substring(listing.items, args.asset.as_deref(), &["name", "key"]);

    print_listing(&ctx.output, listing, PORTFOLIO_TABLE_COLUMNS)
}
