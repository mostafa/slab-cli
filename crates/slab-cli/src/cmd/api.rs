use super::Context;

/// Run a raw GraphQL query against the configured Slab endpoint.
pub async fn run(ctx: &Context, query: &str, variables: Option<&str>) -> anyhow::Result<()> {
    let client = ctx.client()?;
    let vars = variables
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()
        .map_err(|e| anyhow::anyhow!("invalid --variables JSON: {e}"))?;

    let query_text = if query == "-" {
        std::io::read_to_string(std::io::stdin())?
    } else {
        query.to_string()
    };

    let resp = client.raw_query(&query_text, vars).await?;
    println!("{}", serde_json::to_string_pretty(&resp)?);
    Ok(())
}
