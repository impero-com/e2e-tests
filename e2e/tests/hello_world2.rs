#![feature(custom_test_frameworks)]
#![test_runner(e2e::e2e_test_runner)]

use anyhow::Result;
use e2e::Context;

#[test_case]
async fn hello_world(ctx: Context) -> Result<()> {
    ctx.page
        .goto_builder("http://127.0.0.1:8000")
        .goto()
        .await?;
    let body = ctx.page.inner_text("body", None).await?;
    assert_eq!(body, "Hello, world!");

    Ok(())
}
