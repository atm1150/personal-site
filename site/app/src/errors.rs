use http::StatusCode;
use leptos::prelude::RwSignal;

/// Raised when the page `ErrorBoundary` shows its fallback; never lowered -
/// every exit from an errored page is a full load, which resets the app.
#[derive(Clone, Copy)]
pub struct PageErrored(pub RwSignal<bool>);

/// Errors that can be rendered as a page by [`crate::error_template::ErrorTemplate`].
///
/// Each new variant adds a `status_code` match arm and is rendered by the same
/// template. Route-level 404s are synthesized in the `Routes` fallback (see
/// `crate::App`); fallible renders surface through the `ErrorBoundary` there,
/// which maps non-`AppError` errors to [`AppError::Internal`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    #[error("Not Found")]
    NotFound,
    /// Stand-in for any error that is not an AppError; its display is all a visitor sees.
    #[error("Something went wrong on our end.")]
    Internal,
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
