use crate::errors::AppError;
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use leptos_axum::ResponseOptions;
use leptos_meta::Title;

/// Renders [`AppError`]s as a page, and (during SSR) sets the HTTP response
/// status from the first error so an unmatched route returns a real 404 rather
/// than a 200 with not-found content (a soft-404).
///
/// Fed by the `Routes` fallback for unmatched routes today; reused as an
/// `ErrorBoundary` fallback when a component first renders `Err`.
#[component]
pub fn ErrorTemplate(#[prop(into)] errors: Signal<Errors>) -> impl IntoView {
    // Downcast the type-erased errors back to AppError so we can read status codes.
    let errors = Memo::new(move |_| {
        errors
            .get_untracked()
            .into_iter()
            .filter_map(|(_, v)| v.downcast_ref::<AppError>().cloned())
            .collect::<Vec<_>>()
    });

    // Only the first error's status is sent; sufficient while a page renders one error.
    #[cfg(feature = "ssr")]
    {
        if let Some(response) = use_context::<ResponseOptions>()
            && let Some(first) = errors.read_untracked().first()
        {
            response.set_status(first.status_code());
        }
    }

    view! {
        <Title text="Page not found"/>
        <div class="not-found-page">
            {move || {
                errors
                    .get()
                    .into_iter()
                    .map(|error| {
                        let code = error.status_code();
                        view! {
                            <h1>{code.as_u16()} " " {code.canonical_reason().unwrap_or_default()}</h1>
                            <p>{error.to_string()}</p>
                        }
                    })
                    .collect_view()
            }}
            <p><a href="/">"Back to home"</a></p>
        </div>
    }
}
