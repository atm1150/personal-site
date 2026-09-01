//! Per-IP rate limiting for the site listener. One shared governor (GCRA:
//! burst capacity plus steady replenishment) keyed on the peer address - the
//! server terminates TLS itself, so the peer is the client. Forwarded-header
//! keying only becomes relevant if a proxy is ever put in front.

use std::sync::Arc;
use std::time::Duration;

use governor::middleware::NoOpMiddleware;
use hyper_util::rt::TokioTimer;
use tower_governor::governor::{GovernorConfig, GovernorConfigBuilder};
use tower_governor::key_extractor::PeerIpKeyExtractor;

/// Requests one IP may land back-to-back before throttling. Sized for several
/// simultaneous cold page loads (~9 requests each) behind one shared IP.
pub const BURST_SIZE: u32 = 40;

/// One spent unit of burst capacity returns per this interval (5/sec sustained).
pub const REPLENISH_INTERVAL: Duration = Duration::from_millis(200);

/// How often idle entries are evicted from the per-IP map.
pub const EVICTION_INTERVAL: Duration = Duration::from_secs(60);

/// Longest a handler may take to produce a response (the body may stream
/// longer). SSR renders in milliseconds; anything near this bound is wedged.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Longest a connection may dribble request headers before hyper closes it
/// (the slowloris bound). Applied on the TLS listener via its hyper builder;
/// hyper's own 30s default needs a timer axum-server never installs, so
/// without this explicit wiring no header timeout is active at all.
pub const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Largest accepted request body. The site takes no uploads, so this is pure
/// headroom over the zero-length bodies real traffic sends.
pub const BODY_LIMIT_BYTES: usize = 16 * 1024;

// Provisional: reasoned, not measured; retune against real traffic.
/// How often an idle HTTP/2 connection is PING-probed for liveness.
pub const H2_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);
/// How long to wait for a PING ack before closing an HTTP/2 connection.
pub const H2_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(10);

/// The one governor shape this site uses: peer-IP keyed, no per-request
/// state middleware.
pub type SiteGovernor = Arc<GovernorConfig<PeerIpKeyExtractor, NoOpMiddleware>>;

/// request-level bounds (rate limiting, request timeout, body size) applied by [`crate::router`].
pub struct RequestLimits {
    pub governor: Option<SiteGovernor>,
    pub request_timeout: Duration,
}

impl RequestLimits {
    pub fn production() -> Self {
        Self {
            governor: Some(site_governor()),
            request_timeout: REQUEST_TIMEOUT,
        }
    }

    /// No rate limiting - for tests exercising unrelated behavior, where a
    /// shared limiter would couple test outcomes to request counts.
    pub fn disabled() -> Self {
        Self {
            governor: None,
            request_timeout: REQUEST_TIMEOUT,
        }
    }
}

/// Connection-level bounds applied to each listener's hyper builder, before
/// any request exists. Separate from [`RequestLimits`] because these are
/// consumed at connection setup (slowloris territory), not in the router.
pub struct ConnectionLimits {
    pub header_read_timeout: Duration,
    pub h2_keepalive_interval: Duration,
    pub h2_keepalive_timeout: Duration,
}

impl ConnectionLimits {
    pub fn production() -> Self {
        Self {
            header_read_timeout: HEADER_READ_TIMEOUT,
            h2_keepalive_interval: H2_KEEPALIVE_INTERVAL,
            h2_keepalive_timeout: H2_KEEPALIVE_TIMEOUT,
        }
    }

    /// Production values with only the header-read timeout overridden - the
    /// injection point socket tests use to keep the bound sub-second.
    pub fn with_header_read_timeout(header_read_timeout: Duration) -> Self {
        Self {
            header_read_timeout,
            ..Self::production()
        }
    }
}

/// Apply the connection bounds to a listener's hyper builder: http1 gets the
/// header-read timeout, http2 gets keep-alive (hyper has no h2 header-read knob).
/// Each side needs the timer, or hyper panics on header_read_timeout.
pub fn apply_connection_bounds<A: axum_server::Address, Acc>(
    server: &mut axum_server::Server<A, Acc>,
    bounds: &ConnectionLimits,
) {
    server
        .http_builder()
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(bounds.header_read_timeout);
    server
        .http_builder()
        .http2()
        .timer(TokioTimer::new())
        .keep_alive_interval(Some(bounds.h2_keepalive_interval))
        .keep_alive_timeout(bounds.h2_keepalive_timeout);
}

/// Build the one governor this site uses: peer-IP keyed, no per-request
/// state middleware.
fn site_governor() -> SiteGovernor {
    Arc::new(
        GovernorConfigBuilder::default()
            .per_millisecond(
                u64::try_from(REPLENISH_INTERVAL.as_millis())
                    .expect("interval fits in u64 milliseconds"),
            )
            .burst_size(BURST_SIZE)
            .finish()
            .expect("burst and interval are nonzero constants"),
    )
}

/// Spawn periodic eviction of idle per-IP entries; without it the limiter's
/// map grows with every distinct client ever seen.
pub fn spawn_eviction(config: &SiteGovernor) {
    let limiter = Arc::clone(config.limiter());
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(EVICTION_INTERVAL);
        loop {
            tick.tick().await;
            limiter.retain_recent();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_limits_carry_a_governor() {
        assert!(RequestLimits::production().governor.is_some());
    }

    #[test]
    fn retain_recent_keeps_a_freshly_seen_key() {
        // Only recent-key retention; stale eviction needs clock control the governor doesn't expose.
        let governor = RequestLimits::production()
            .governor
            .expect("production has a governor");
        let limiter = governor.limiter();
        let key = std::net::IpAddr::from([10, 0, 0, 1]);
        let _ = limiter.check_key(&key);
        assert_eq!(limiter.len(), 1, "the seen key is tracked");
        limiter.retain_recent();
        assert_eq!(limiter.len(), 1, "a freshly seen key survives a sweep");
    }

    #[test]
    fn connection_limits_production_uses_the_header_read_constant() {
        assert_eq!(
            ConnectionLimits::production().header_read_timeout,
            HEADER_READ_TIMEOUT
        );
    }

    #[test]
    fn connection_limits_can_override_only_the_header_read_timeout() {
        let c = ConnectionLimits::with_header_read_timeout(Duration::from_millis(100));
        assert_eq!(c.header_read_timeout, Duration::from_millis(100));
        assert_eq!(c.h2_keepalive_interval, H2_KEEPALIVE_INTERVAL);
    }
}
