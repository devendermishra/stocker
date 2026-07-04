#[cfg(feature = "web")]
use dioxus::prelude::spawn;

/// Open the system print dialog so the user can save the report as PDF.
pub fn export_report_pdf() {
    #[cfg(feature = "desktop")]
    {
        dioxus::desktop::window().print();
        return;
    }

    #[cfg(feature = "web")]
    {
        // Prefer direct window.print from the user-gesture handler; eval as fallback.
        if let Some(window) = web_sys::window() {
            let _ = window.print();
            return;
        }
        spawn(async {
            let _ = dioxus::document::eval("window.print();").await;
        });
    }
}
