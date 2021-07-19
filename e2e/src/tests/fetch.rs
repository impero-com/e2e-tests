use crate::{playwright_ext::PageFetchExt, Context};
use anyhow::Result;
use common::PayloadCookies;
use futures::try_join;

#[test_case]
async fn get_404(ctx: Context) -> Result<()> {
    ctx.page
        .goto_builder("http://127.0.0.1:8000")
        .goto()
        .await?;

    let response_404 = ctx.page.get("/404").await?;

    assert_eq!(response_404.status()?, 404);

    Ok(())
}

#[test_case]
async fn mixed_methods(ctx: Context) -> Result<()> {
    ctx.page
        .goto_builder("http://127.0.0.1:8000")
        .goto()
        .await?;

    let (get, post, put, patch, delete) = try_join!(
        ctx.page.get("/check-cookies"),
        ctx.page.post(
            "/check-cookies",
            PayloadCookies {
                message: "Yummy posted cookies".to_string(),
                count: 42
            }
        ),
        ctx.page.put(
            "/check-cookies",
            PayloadCookies {
                message: "Yummy put cookies".to_string(),
                count: 43
            }
        ),
        ctx.page.patch(
            "/check-cookies",
            PayloadCookies {
                message: "Yummy patched cookies".to_string(),
                count: 44
            }
        ),
        ctx.page.delete("/check-cookies"),
    )?;

    assert_eq!(get.status()?, 200);
    assert_eq!(get.request().method()?, "GET");

    assert_eq!(post.status()?, 200);
    assert_eq!(post.request().method()?, "POST");

    assert_eq!(put.status()?, 200);
    assert_eq!(put.request().method()?, "PUT");

    assert_eq!(patch.status()?, 200);
    assert_eq!(patch.request().method()?, "PATCH");

    assert_eq!(delete.status()?, 200);
    assert_eq!(delete.request().method()?, "DELETE");

    Ok(())
}
