//! Validated page-metadata types and the component that emits them into the head.

use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use thiserror::Error;
use url::Url;

/// Search results truncate titles around this many characters.
pub const TITLE_MAX_CHARS: usize = 70;
/// Search snippets show roughly 50-160 characters of a description.
pub const DESCRIPTION_MIN_CHARS: usize = 50;
pub const DESCRIPTION_MAX_CHARS: usize = 160;

fn parse_http_url(value: &str) -> Result<Url, MetaError> {
    let parsed = Url::parse(value).map_err(|_| MetaError::NotAbsolute(value.to_owned()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(MetaError::NotAbsolute(value.to_owned()));
    }
    Ok(parsed)
}

#[derive(Debug, Error, PartialEq)]
pub enum MetaError {
    #[error("value must not be empty")]
    Empty,
    #[error("{context}: {len} chars exceeds max {max}")]
    TooLong {
        context: &'static str,
        len: usize,
        max: usize,
    },
    #[error("{context}: {len} chars under min {min}")]
    TooShort {
        context: &'static str,
        len: usize,
        min: usize,
    },
    #[error("must be an absolute http(s) URL: {0}")]
    NotAbsolute(String),
    #[error("must not carry a path, query, or fragment: {0}")]
    NotBare(String),
}

/// A page `<title>` (also og:title), bounded to what search results display.
#[derive(Clone, Debug, PartialEq)]
pub struct MetaTitle(String);

impl MetaTitle {
    pub fn new(value: impl Into<String>) -> Result<Self, MetaError> {
        let value = value.into();
        let len = value.chars().count();
        if len == 0 {
            return Err(MetaError::Empty);
        }
        if len > TITLE_MAX_CHARS {
            return Err(MetaError::TooLong {
                context: "title",
                len,
                max: TITLE_MAX_CHARS,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A meta/og description, bounded to the search-snippet window.
#[derive(Clone, Debug, PartialEq)]
pub struct MetaDescription(String);

impl MetaDescription {
    pub fn new(value: impl Into<String>) -> Result<Self, MetaError> {
        let value = value.into();
        let len = value.chars().count();
        if len == 0 {
            return Err(MetaError::Empty);
        }
        if len < DESCRIPTION_MIN_CHARS {
            return Err(MetaError::TooShort {
                context: "description",
                len,
                min: DESCRIPTION_MIN_CHARS,
            });
        }
        if len > DESCRIPTION_MAX_CHARS {
            return Err(MetaError::TooLong {
                context: "description",
                len,
                max: DESCRIPTION_MAX_CHARS,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An og:image URL; the OpenGraph spec requires it absolute.
#[derive(Clone, Debug, PartialEq)]
pub struct OgImageUrl(String);

impl OgImageUrl {
    pub fn new(value: &str) -> Result<Self, MetaError> {
        let parsed = parse_http_url(value)?;
        Ok(Self(parsed.into()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The site's public origin (scheme + host + optional port), the prefix for
/// og:url and the canonical link. Runtime truth: arrives via PUBLIC_BASE_URL.
#[derive(Clone, Debug, PartialEq)]
pub struct PublicBaseUrl {
    origin: String,
    /// The bare host, captured during the one `url` parse so `host()` costs
    /// callers no re-parsing.
    host: String,
}

impl PublicBaseUrl {
    pub fn new(value: &str) -> Result<Self, MetaError> {
        let parsed = parse_http_url(value)?;
        if parsed.path() != "/" || parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(MetaError::NotBare(value.to_owned()));
        }
        // Can't fail: `parse_http_url` restricted the scheme to http/https,
        // which always carry a host. The `url` crate lowercases it during
        // parsing - the casing the www redirect's comparison relies on
        // (pinned by a test below).
        let host = parsed
            .host_str()
            .expect("http(s) URLs always have a host")
            .to_owned();
        // Url normalizes a bare origin to a trailing "/"; store it bare so
        // join() is a plain concatenation with the request path.
        let mut origin: String = parsed.into();
        origin.truncate(origin.trim_end_matches('/').len());
        Ok(Self { origin, host })
    }

    pub fn as_str(&self) -> &str {
        &self.origin
    }

    /// Absolute URL for a request path (paths always start with '/').
    pub fn join(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }

    /// The bare host (no scheme, no port), lowercased.
    pub fn host(&self) -> &str {
        &self.host
    }
}

/// Everything a page must declare about itself. Named, newtyped fields:
/// a missing field is a compile error, a bad value never constructs.
#[derive(Clone)]
pub struct PageMeta {
    pub title: MetaTitle,
    pub description: MetaDescription,
    pub image: Option<OgImageUrl>,
}

/// Emits a page's head metadata. Every tag here is unconditional given its
/// PageMeta (values are compile-time literals), so server and client render
/// identical leptos_meta sequences - which the hydration cursor requires.
/// og:url and the canonical link deliberately live in shell() instead: their
/// value is server-only knowledge (PUBLIC_BASE_URL), and shell never runs on
/// the client, so they stay outside hydration entirely.
#[component]
pub fn PageMetaTags(meta: PageMeta) -> impl IntoView {
    view! {
        <Title text=meta.title.as_str().to_owned()/>
        <Meta name="description" content=meta.description.as_str().to_owned()/>
        <Meta property="og:title" content=meta.title.as_str().to_owned()/>
        <Meta property="og:description" content=meta.description.as_str().to_owned()/>
        <Meta property="og:type" content="website"/>
        {meta
            .image
            .map(|image| view! { <Meta property="og:image" content=image.as_str().to_owned()/> })}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The two live pages' literals must always construct; these tests are the
    // compile-adjacent guarantee that a copy edit cannot ship an invalid value.
    #[test]
    fn home_page_literals_are_valid() {
        MetaTitle::new("atmil — portfolio").unwrap();
        MetaDescription::new(
            "Personal website for displaying a code portfolio and expressing my thoughts",
        )
        .unwrap();
    }

    #[test]
    fn resume_page_literals_are_valid() {
        MetaTitle::new("Andrew Miller — Software Engineer").unwrap();
        MetaDescription::new("Resume of Andrew Miller, software engineer in Seattle, WA.").unwrap();
    }

    #[test]
    fn empty_title_rejected() {
        assert_eq!(MetaTitle::new("").unwrap_err(), MetaError::Empty);
    }

    #[test]
    fn overlong_title_rejected() {
        let long = "x".repeat(TITLE_MAX_CHARS + 1);
        assert!(matches!(
            MetaTitle::new(long).unwrap_err(),
            MetaError::TooLong { .. }
        ));
    }

    #[test]
    fn short_description_rejected() {
        assert!(matches!(
            MetaDescription::new("Too short.").unwrap_err(),
            MetaError::TooShort { .. }
        ));
    }

    #[test]
    fn overlong_description_rejected() {
        let long = "x".repeat(DESCRIPTION_MAX_CHARS + 1);
        assert!(matches!(
            MetaDescription::new(long).unwrap_err(),
            MetaError::TooLong { .. }
        ));
    }

    #[test]
    fn relative_og_image_rejected() {
        assert!(matches!(
            OgImageUrl::new("/og-card.png").unwrap_err(),
            MetaError::NotAbsolute(_)
        ));
    }

    #[test]
    fn absolute_og_image_accepted() {
        let url = OgImageUrl::new("https://example.test/og-card.png").unwrap();
        assert_eq!(url.as_str(), "https://example.test/og-card.png");
    }

    #[test]
    fn base_url_trailing_slash_normalized() {
        let base = PublicBaseUrl::new("https://example.test/").unwrap();
        assert_eq!(base.as_str(), "https://example.test");
    }

    #[test]
    fn base_url_with_port_accepted() {
        let base = PublicBaseUrl::new("https://localhost:4000").unwrap();
        assert_eq!(base.as_str(), "https://localhost:4000");
    }

    #[test]
    fn base_url_with_path_rejected() {
        assert!(matches!(
            PublicBaseUrl::new("https://example.test/sub").unwrap_err(),
            MetaError::NotBare(_)
        ));
    }

    #[test]
    fn base_url_non_http_rejected() {
        assert!(matches!(
            PublicBaseUrl::new("ftp://example.test").unwrap_err(),
            MetaError::NotAbsolute(_)
        ));
    }

    #[test]
    fn base_url_malformed_rejected() {
        assert!(matches!(
            PublicBaseUrl::new("not a url").unwrap_err(),
            MetaError::NotAbsolute(_)
        ));
    }

    #[test]
    fn join_produces_absolute_page_urls() {
        let base = PublicBaseUrl::new("https://example.test").unwrap();
        assert_eq!(base.join("/"), "https://example.test/");
        assert_eq!(base.join("/resume"), "https://example.test/resume");
    }

    #[test]
    fn host_returns_the_bare_hostname() {
        let base = PublicBaseUrl::new("https://example.test:4000").unwrap();
        assert_eq!(base.host(), "example.test");
    }

    #[test]
    fn host_is_lowercased_even_for_mixed_case_input() {
        // The `url` crate normalizes a registered-name host to lowercase
        // during parsing; this pins that behavior rather than assuming it.
        let base = PublicBaseUrl::new("https://Example.TEST").unwrap();
        assert_eq!(base.host(), "example.test");
    }
}
