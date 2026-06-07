#[cfg(all(feature = "web", not(feature = "desktop")))]
pub type ResearchReport = crate::web_types::ResearchReport;

#[cfg(all(feature = "desktop", not(feature = "web")))]
pub type ResearchReport = stocker_core::ResearchReport;
