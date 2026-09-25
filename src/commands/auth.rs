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

    // NB: --offset/--limit are accepted by clap but, matching pre-refactor behavior, not yet
    // wired up here (list-portfolios always fetches every page). Logged as a known follow-up.
    let listing = client.list_portfolios()?;
    let items = filter_by_substring(listing, args.asset.as_deref(), &["name", "key"]);

    let result = Listing { items, page: None };
    print_listing(&ctx.output, result, PORTFOLIO_TABLE_COLUMNS)
}
