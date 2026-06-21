use dioxus::prelude::*;

use crate::routes::Route;

#[cfg(feature = "desktop")]
pub fn app() -> Element {
    rsx! { Router::<Route> {} }
}

#[cfg(not(feature = "desktop"))]
pub fn app() -> Element {
    rsx! { Router::<Route> {} }
}
