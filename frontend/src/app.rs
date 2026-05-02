use dioxus::prelude::*;

use crate::routes::Route;

pub fn app() -> Element {
    rsx! { Router::<Route> {} }
}
