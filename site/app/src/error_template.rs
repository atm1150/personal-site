use crate::errors::AppError;
use crate::shell::is_current;
use http::StatusCode;
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_axum::ResponseOptions;
use leptos_meta::Title;
use leptos_router::hooks::use_location;

/// Build the `ErrorBoundary` fallback view: the shared template, fed the
/// errors the boundary collected from a failed render
pub fn error_boundary_fallback(errors: ArcRwSignal<Errors>) -> impl IntoView {
    if let Some(flag) = use_context::<crate::errors::PageErrored>() {
        flag.0.set(true);
    }
    view! { <ErrorTemplate errors/> }
}

/// Tab title for the error page.
fn page_title(status: Option<StatusCode>) -> &'static str {
    if status == Some(StatusCode::NOT_FOUND) {
        "Page not found"
    } else {
        "Something went wrong"
    }
}

/// A browser-caught error has no HTTP status to claim, so only the ssr
/// compilation names the 500.
fn page_heading(status: StatusCode) -> &'static str {
    if status == StatusCode::NOT_FOUND {
        "404 Not Found"
    } else if cfg!(feature = "ssr") {
        "500 Internal Server Error"
    } else {
        "Something went wrong"
    }
}

/// Renders [`AppError`]s as a page, and (during SSR) sets the HTTP response
/// status from the first error so an unmatched route returns a real 404 rather
/// than a 200 with not-found content (a soft-404).
///
/// Fed by the `Routes` fallback for unmatched routes, and by the `ErrorBoundary`
/// in [`crate::App`] (via [`error_boundary_fallback`]) when a component renders `Err`.
#[component]
pub fn ErrorTemplate(#[prop(into)] errors: Signal<Errors>) -> impl IntoView {
    let pathname = use_location().pathname;
    // Downcast the type-erased errors back to AppError; anything else renders as
    // the generic Internal variant, so foreign error detail never reaches the page.
    let errors = Memo::new(move |_| {
        errors
            .get_untracked()
            .into_iter()
            .map(|(_, v)| {
                v.downcast_ref::<AppError>()
                    .cloned()
                    .unwrap_or(AppError::Internal)
            })
            .collect::<Vec<_>>()
    });

    // Only the first error drives status and title; sufficient while a page renders one error.
    let first_status = errors.read_untracked().first().map(AppError::status_code);

    #[cfg(feature = "ssr")]
    {
        if let Some(response) = use_context::<ResponseOptions>()
            && let Some(status) = first_status
        {
            response.set_status(status);
        }
    }

    view! {
        <Title text=page_title(first_status)/>
        <div class="not-found-page prose">
            {move || {
                errors
                    .get()
                    .into_iter()
                    .map(|error| {
                        view! {
                            <h1>{page_heading(error.status_code())}</h1>
                            <p>{error.to_string()}</p>
                        }
                    })
                    .collect_view()
            }}
            // A router nav to the page we're already on is a no-op; only that
            // case needs the full load.
            <p>
                <a href="/" target=move || is_current(&pathname.get(), "/").then_some("_self")>
                    "Back to home"
                </a>
            </p>
        </div>
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use leptos_router::components::Router;
    use leptos_router::location::RequestUrl;

    const LEAK_MARKER: &str = "LEAK_MARKER_INTERNAL_DETAIL";

    #[derive(Debug, Clone, thiserror::Error)]
    #[error("LEAK_MARKER_INTERNAL_DETAIL: postgres://user:secret@db")]
    struct FixtureError;

    #[component]
    fn AlwaysFails() -> impl IntoView {
        Err::<(), FixtureError>(FixtureError)
    }

    /// Render the boundary as `App` wires it, as if serving `path`.
    fn render_failing_boundary(path: &'static str) -> (String, ResponseOptions) {
        let response = ResponseOptions::default();
        let context = response.clone();
        let owner = Owner::new();
        let html = owner.with(move || {
            provide_context(RequestUrl::new(path));
            provide_context(context);
            leptos_meta::provide_meta_context();
            view! {
                <Router>
                    <ErrorBoundary fallback=error_boundary_fallback>
                        <AlwaysFails/>
                    </ErrorBoundary>
                </Router>
            }
            .to_html()
        });
        (html, response)
    }

    fn status_of(response: &ResponseOptions) -> Option<http::StatusCode> {
        response
            .0
            .read()
            .expect("response parts lock poisoned")
            .status
    }

    #[test]
    fn failing_component_renders_a_500_page_during_ssr() {
        let (html, response) = render_failing_boundary("/");

        assert_eq!(
            status_of(&response),
            Some(http::StatusCode::INTERNAL_SERVER_ERROR)
        );
        assert!(html.contains("500"), "got body:\n{html}");
        assert!(html.contains("Internal Server Error"), "got body:\n{html}");
        assert!(
            html.contains("Something went wrong on our end."),
            "got body:\n{html}"
        );
        assert!(html.contains("Back to home"), "got body:\n{html}");
    }

    #[test]
    fn page_heading_names_the_status_on_ssr() {
        assert_eq!(
            page_heading(http::StatusCode::INTERNAL_SERVER_ERROR),
            "500 Internal Server Error"
        );
        assert_eq!(page_heading(http::StatusCode::NOT_FOUND), "404 Not Found");
    }

    #[test]
    fn back_to_home_escapes_the_router_when_erroring_on_home() {
        let (html, _) = render_failing_boundary("/");

        assert!(html.contains(r#"target="_self""#), "got body:\n{html}");
    }

    #[test]
    fn back_to_home_stays_a_router_link_off_home() {
        let (html, _) = render_failing_boundary("/resume");

        assert!(!html.contains(r#"target="_self""#), "got body:\n{html}");
    }

    #[test]
    fn failing_component_detail_never_reaches_html() {
        let (html, _) = render_failing_boundary("/");

        assert!(!html.contains(LEAK_MARKER), "got body:\n{html}");
        assert!(!html.contains("FixtureError"), "got body:\n{html}");
    }

    #[test]
    fn fallback_sets_page_errored_while_shown() {
        let owner = Owner::new();
        owner.with(move || {
            let flag = crate::errors::PageErrored(RwSignal::new(false));
            provide_context(RequestUrl::new("/"));
            provide_context(flag);
            provide_context(ResponseOptions::default());
            leptos_meta::provide_meta_context();
            let _html = view! {
                <Router>
                    <ErrorBoundary fallback=error_boundary_fallback>
                        <AlwaysFails/>
                    </ErrorBoundary>
                </Router>
            }
            .to_html();

            assert!(
                flag.0.get_untracked(),
                "rendering the fallback should raise PageErrored"
            );
        });
    }

    #[test]
    fn not_found_still_renders_404_with_status() {
        let response = ResponseOptions::default();
        let context = response.clone();
        let owner = Owner::new();
        let html = owner.with(move || {
            provide_context(RequestUrl::new("/no-such-page"));
            provide_context(context);
            leptos_meta::provide_meta_context();
            let mut errors = Errors::default();
            errors.insert_with_default_key(AppError::NotFound);
            let errors = RwSignal::new(errors);
            view! {
                <Router>
                    <ErrorTemplate errors/>
                </Router>
            }
            .to_html()
        });

        assert_eq!(status_of(&response), Some(http::StatusCode::NOT_FOUND));
        assert!(html.contains("404"), "got body:\n{html}");
        assert!(html.contains("Not Found"), "got body:\n{html}");
        // The app is healthy on a 404; going home stays a router navigation.
        assert!(!html.contains(r#"target="_self""#), "got body:\n{html}");
    }

    #[test]
    fn page_title_names_not_found_for_404() {
        assert_eq!(
            page_title(Some(http::StatusCode::NOT_FOUND)),
            "Page not found"
        );
    }

    #[test]
    fn page_title_is_generic_for_other_statuses() {
        assert_eq!(
            page_title(Some(http::StatusCode::INTERNAL_SERVER_ERROR)),
            "Something went wrong"
        );
        assert_eq!(page_title(None), "Something went wrong");
    }
}
