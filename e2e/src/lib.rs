#![feature(custom_test_frameworks)]
#![feature(internal_output_capture)]
#![test_runner(e2e_test_runner)]

#[cfg(test)]
mod tests;

use anyhow::Result;
use async_trait::async_trait;
use futures::{stream, FutureExt, StreamExt};
use pin_project::pin_project;
use playwright::{api::Page, Playwright};
use std::{
    any::{type_name, Any},
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display, Formatter},
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    task::Poll,
};
use tokio::runtime::Runtime;

pub mod playwright_ext;

pub fn e2e_test_runner(tests: &[&dyn Testable]) {
    let mut web_server = Command::new("target/debug/web")
        .current_dir("..")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let runtime = Runtime::new().unwrap();
    let results = runtime.block_on(run_tests(tests));

    let exit_code = match results {
        Ok(test_results) => {
            println!("\nSummary:");

            for test_result in &test_results {
                println!("{}", test_result);
            }

            let successes = test_results
                .iter()
                .filter(|test_result| test_result.result.is_ok())
                .count();
            if successes == test_results.len() {
                println!("{} tests ran with success", successes);
                0
            } else {
                println!("{} errors", test_results.len() - successes);
                1
            }
        }
        Err(error) => {
            println!("{:#?}", error);
            1
        }
    };

    web_server.kill().unwrap();

    std::process::exit(exit_code);
}

async fn run_tests(tests: &[&dyn Testable]) -> anyhow::Result<Vec<TestResult>> {
    let playwright = Playwright::initialize().await?;
    playwright.prepare()?; // Install browsers

    let mut browser_map = HashMap::new();
    let mut initialization_errors: Option<ErrorList<FailedToInitialize>> = None;
    {
        match playwright
            .chromium()
            .launcher()
            .headless(true)
            .launch()
            .await
        {
            Ok(browser) => {
                browser_map.insert(BrowserType::Chromium, browser);
            }
            Err(err) => {
                if let Some(errs) = &mut initialization_errors {
                    errs.push(FailedToInitialize(BrowserType::Chromium), err);
                } else {
                    initialization_errors = Some(ErrorList::new(
                        FailedToInitialize(BrowserType::Chromium),
                        err,
                    ));
                }
            }
        }
    }
    {
        match playwright
            .firefox()
            .launcher()
            .headless(true)
            .launch()
            .await
        {
            Ok(browser) => {
                browser_map.insert(BrowserType::Firefox, browser);
            }
            Err(err) => {
                if let Some(errs) = &mut initialization_errors {
                    errs.push(FailedToInitialize(BrowserType::Firefox), err);
                } else {
                    initialization_errors = Some(ErrorList::new(
                        FailedToInitialize(BrowserType::Firefox),
                        err,
                    ));
                }
            }
        }
    }
    {
        match playwright.webkit().launcher().headless(true).launch().await {
            Ok(browser) => {
                browser_map.insert(BrowserType::Webkit, browser);
            }
            Err(err) => {
                if let Some(errs) = &mut initialization_errors {
                    errs.push(FailedToInitialize(BrowserType::Webkit), err);
                } else {
                    initialization_errors =
                        Some(ErrorList::new(FailedToInitialize(BrowserType::Webkit), err));
                }
            }
        }
    }

    if let Some(errors) = initialization_errors {
        return Err(errors.into());
    }

    let (results, error_list) = stream::iter(tests)
        .flat_map(|test| {
            stream::iter(browser_map.iter()).map(move |(&browser_type, browser)| async move {
                let context = browser.context_builder().build().await.map_err(|err| {
                    (
                        FailedToOpenPage {
                            browser_type,
                            test_name: test.name(),
                        },
                        err,
                    )
                })?;
                let page = context.new_page().await.map_err(|err| {
                    (
                        FailedToOpenPage {
                            browser_type,
                            test_name: test.name(),
                        },
                        err,
                    )
                })?;
                let test_name = test.name();
                Ok(test
                    .run(Context { page })
                    .map(|(result, output)| TestResult {
                        test_name,
                        browser_type,
                        result,
                        output,
                    })
                    .inspect(|test_result| println!("{}", test_result))
                    .await)
            })
        })
        .buffer_unordered(tests.len() * browser_map.len())
        .fold(
            (Vec::new(), None),
            |(mut test_results, errors), result| async {
                match (result, errors) {
                    (Ok(test_result), errors) => {
                        test_results.push(test_result);
                        (test_results, errors)
                    }
                    (Err((context, err)), None) => {
                        (test_results, Some(ErrorList::new(context, err)))
                    }
                    (Err((context, err)), Some(mut error_list)) => {
                        error_list.push(context, err);
                        (test_results, Some(error_list))
                    }
                }
            },
        )
        .await;

    if let Some(error_list) = error_list {
        return Err(error_list.into());
    }

    Ok(results)
}

struct TestResult {
    test_name: &'static str,
    browser_type: BrowserType,
    result: anyhow::Result<()>,
    output: Vec<u8>,
}

impl Display for TestResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.result {
            Ok(()) => write!(f, "{} in {}...\t[OK]", self.test_name, self.browser_type)?,
            Err(err) => write!(
                f,
                "{} in {}...\t[FAILED]\n{:#?}",
                self.test_name, self.browser_type, err
            )?,
        }
        if !self.output.is_empty() {
            write!(
                f,
                "\n   ----- TEST STDOUT -----   \n{}\n",
                String::from_utf8_lossy(&self.output)
            )?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, PartialOrd, PartialEq, Eq, Hash)]
enum BrowserType {
    Chromium,
    Firefox,
    Webkit,
}

impl Display for BrowserType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                BrowserType::Chromium => "Chromium",
                BrowserType::Firefox => "Firefox",
                BrowserType::Webkit => "Webkit",
            }
        )
    }
}

pub struct Context {
    pub page: Page,
}

#[async_trait]
pub trait Testable {
    fn name(&self) -> &'static str;
    async fn run(&self, ctx: Context) -> (Result<()>, Vec<u8>);
}

#[async_trait]
impl<F, FF> Testable for F
where
    F: Fn(Context) -> FF + Sync,
    FF: Future<Output = Result<()>> + Send,
{
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }

    async fn run(&self, ctx: Context) -> (Result<()>, Vec<u8>) {
        let (result, output) = CaptureOutputFuture::new(self(ctx)).await;
        match result {
            Ok(test_result) => (test_result, output),
            Err(caught_panic) => (Err(caught_panic.into()), output),
        }
    }
}

#[pin_project]
#[must_use = "futures do nothing unless you `.await` or poll them"]
struct CaptureOutputFuture<Fut> {
    #[pin]
    future: Fut,
    output: Arc<Mutex<Vec<u8>>>,
}

impl<Fut> CaptureOutputFuture<Fut> {
    fn new(future: Fut) -> Self {
        CaptureOutputFuture {
            future,
            output: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl<Fut: Future> Future for CaptureOutputFuture<Fut> {
    type Output = (Result<Fut::Output, CaughtPanic>, Vec<u8>);

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        std::io::set_output_capture(Some(Arc::clone(&self.output)));
        let this = self.project();
        let f = this.future;
        let o = this.output;
        let result = catch_unwind(AssertUnwindSafe(|| f.poll(cx)));
        std::io::set_output_capture(None);

        match result {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(value)) => Poll::Ready((
                Ok(value),
                std::mem::replace(o.lock().unwrap().as_mut(), Vec::new()),
            )),
            Err(err) => Poll::Ready((
                Err(CaughtPanic::new(err)),
                std::mem::replace(o.lock().unwrap().as_mut(), Vec::new()),
            )),
        }
    }
}

struct ErrorList<C> {
    vec: Vec<(C, anyhow::Error)>,
}

impl<C> ErrorList<C> {
    fn new<E: Into<anyhow::Error>>(context: C, error: E) -> Self {
        ErrorList {
            vec: vec![(context, error.into())],
        }
    }

    fn push<E: Into<anyhow::Error>>(&mut self, context: C, error: E) {
        self.vec.push((context, error.into()));
    }
}

impl<C: Display> Debug for ErrorList<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ErrorList:")?;
        for (context, error) in &self.vec {
            writeln!(f, "\t- {}: {:#?}", context, error)?;
        }
        Ok(())
    }
}

impl<C: Display> Display for ErrorList<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "ErrorList:")?;
        for (context, error) in &self.vec {
            writeln!(f, "\t- {}: {}", context, error)?;
        }
        Ok(())
    }
}

impl<C: Display> Error for ErrorList<C> {}

struct FailedToInitialize(BrowserType);

impl Display for FailedToInitialize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to initialize {}", self.0)
    }
}

struct FailedToOpenPage {
    test_name: &'static str,
    browser_type: BrowserType,
}

impl Display for FailedToOpenPage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to open page in {} of {}",
            self.browser_type, self.test_name
        )
    }
}

struct CaughtPanic(Option<Box<str>>);

impl CaughtPanic {
    fn new(err: Box<dyn Any + Send + 'static>) -> Self {
        match err.downcast::<String>() {
            Ok(str) => CaughtPanic(Some(str.into_boxed_str())),
            Err(err) => match err.downcast::<&str>() {
                Ok(str) => CaughtPanic(Some(str.to_string().into_boxed_str())),
                Err(_) => CaughtPanic(None),
            },
        }
    }
}

impl Debug for CaughtPanic {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Display for CaughtPanic {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(str_err) => write!(f, "{}", str_err),
            None => write!(f, "Unknown error"),
        }
    }
}

impl Error for CaughtPanic {}

#[test_case]
async fn sleep(_ctx: Context) -> Result<()> {
    use std::time::Duration;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    Ok(())
}

// #[test_case]
// async fn err(_ctx: Context) -> Result<()> {
//     use std::str::FromStr;
//     let _ = i32::from_str("Not a number")?;
//     Ok(())
// }
//
// #[test_case]
// async fn unimplemented(_ctx: Context) -> Result<()> {
//     unimplemented!()
// }

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
