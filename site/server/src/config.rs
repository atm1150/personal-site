//! App-level runtime config (env = runtime truth; validated once at startup).

use app::meta::{MetaError, PublicBaseUrl};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ConfigError {
    /// A deployed instance must know its public origin; silence would ship
    /// pages whose social cards point nowhere.
    #[error("PUBLIC_BASE_URL is required when LEPTOS_ENV=PROD")]
    BaseUrlMissingInProd,
    #[error("PUBLIC_BASE_URL is invalid: {0}")]
    BaseUrlInvalid(#[from] MetaError),
}

/// Resolve PUBLIC_BASE_URL. Absent = fallback (tags omitted) in dev; absent
/// under PROD is a hard failure; present-but-malformed always fails. Pure so
/// tests never mutate process env.
pub fn resolve_public_base_url(
    is_prod: bool,
    raw: Option<&str>,
) -> Result<Option<PublicBaseUrl>, ConfigError> {
    match raw {
        Some(value) => Ok(Some(PublicBaseUrl::new(value)?)),
        None if is_prod => Err(ConfigError::BaseUrlMissingInProd),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_in_dev_falls_back() {
        assert_eq!(resolve_public_base_url(false, None), Ok(None));
    }

    #[test]
    fn absent_in_prod_fails() {
        assert_eq!(
            resolve_public_base_url(true, None),
            Err(ConfigError::BaseUrlMissingInProd)
        );
    }

    #[test]
    fn valid_value_resolves_in_both() {
        for is_prod in [false, true] {
            let resolved = resolve_public_base_url(is_prod, Some("https://example.test"))
                .expect("valid base URL should resolve")
                .expect("present value should be Some");
            assert_eq!(resolved.as_str(), "https://example.test");
        }
    }

    #[test]
    fn malformed_value_fails_even_in_dev() {
        assert!(matches!(
            resolve_public_base_url(false, Some("not a url")),
            Err(ConfigError::BaseUrlInvalid(_))
        ));
    }
}
