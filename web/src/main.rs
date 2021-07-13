#[macro_use]
extern crate rocket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index])
}

#[test]
fn foo() {
    assert_eq!(4, 2 + 2);
}

#[test]
fn bar() {
    assert_eq!(5, 2 + 2);
}
