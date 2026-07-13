use http::StatusCode;

/// Errors that can be rendered as a page by [`crate::error_template::ErrorTemplate`].
///
/// One variant today; each new variant adds a `status_code` match arm and is
/// rendered by the same template. Route-level 404s are synthesized in the
/// `Routes` fallback (see `crate::App`); future fallible renders surface through
/// an `ErrorBoundary` fed by the same template.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    #[error("Not Found")]
    NotFound,
}

impl AppError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
        }
    }
}
