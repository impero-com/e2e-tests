#[macro_use]
extern crate rocket;

use common::PayloadCookies;
use rocket::{
    http::{Cookie, CookieJar},
    serde::json::Json,
};

#[get("/")]
fn index(cookies: &CookieJar) -> &'static str {
    cookies.add(Cookie::new("Response", "42"));
    "Hello, world!"
}

#[get("/check-cookies")]
fn get_check_cookies(cookies: &CookieJar) -> &'static str {
    assert_eq!(
        cookies
            .get("Response")
            .expect("[GET /check-cookies] Response is not set, visit / first")
            .value(),
        "42"
    );
    "Hello, world!"
}

#[post("/check-cookies", data = "<payload>")]
fn post_check_cookies(cookies: &CookieJar, payload: Json<PayloadCookies>) -> &'static str {
    assert_eq!(
        cookies
            .get("Response")
            .expect("[POST /check-cookies] Response is not set, visit / first")
            .value(),
        "42"
    );
    let payload = payload.into_inner();
    assert!(payload.message.contains("post"));
    assert!((40..50).contains(&payload.count));

    "Hello, world!"
}

#[put("/check-cookies", data = "<payload>")]
fn put_check_cookies(cookies: &CookieJar, payload: Json<PayloadCookies>) -> &'static str {
    assert_eq!(
        cookies
            .get("Response")
            .expect("[PUT /check-cookies] Response is not set, visit / first")
            .value(),
        "42"
    );
    let payload = payload.into_inner();
    assert!(payload.message.contains("put"));
    assert!((40..50).contains(&payload.count));

    "Hello, world!"
}

#[patch("/check-cookies", data = "<payload>")]
fn patch_check_cookies(cookies: &CookieJar, payload: Json<PayloadCookies>) -> &'static str {
    assert_eq!(
        cookies
            .get("Response")
            .expect("[PATCH /check-cookies] Response is not set, visit / first")
            .value(),
        "42"
    );
    let payload = payload.into_inner();
    assert!(payload.message.contains("patch"));
    assert!((40..50).contains(&payload.count));

    "Hello, world!"
}

#[delete("/check-cookies")]
fn delete_check_cookies(cookies: &CookieJar) -> &'static str {
    assert_eq!(
        cookies
            .get("Response")
            .expect("[DELETE /check-cookies] Response is not set, visit / first")
            .value(),
        "42"
    );
    "Hello, world!"
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount(
        "/",
        routes![
            index,
            get_check_cookies,
            post_check_cookies,
            put_check_cookies,
            patch_check_cookies,
            delete_check_cookies
        ],
    )
}

#[test]
fn foo() {
    assert_eq!(4, 2 + 2);
}

#[test]
fn bar() {
    assert_eq!(5, 2 + 2);
}
