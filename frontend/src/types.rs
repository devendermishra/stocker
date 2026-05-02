#[cfg(feature = "web")]
pub type ResearchReport = crate::web_types::ResearchReport;

#[cfg(feature = "desktop")]
pub type ResearchReport = stocker_core::ResearchReport;
