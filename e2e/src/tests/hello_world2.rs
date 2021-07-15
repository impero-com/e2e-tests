use crate::Context;
use anyhow::Result;

#[test_case]
async fn hello_world2(ctx: Context) -> Result<()> {
    ctx.page
        .goto_builder("http://127.0.0.1:8000")
        .goto()
        .await?;
    let body = ctx.page.inner_text("body", None).await?;
    assert_eq!(body, "Hello, world!");

    Ok(())
}
