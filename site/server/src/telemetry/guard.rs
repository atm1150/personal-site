//! The telemetry guard: owns the OTel providers and flushes them on shutdown, plus
//! the panic hook that force-flushes so a panic's record still reaches the collector.

use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;

/// Owns the OTel providers. Dropping it shuts them down, draining the batch
/// buffers — hold it for the whole process lifetime.
///
/// Opaque over a private [`Providers`] enum so the "both providers or neither"
/// invariant is structural: there is no way to hold a lone tracer or lone logger,
/// and only this module can mint a guard.
#[must_use = "dropping the guard shuts telemetry down; hold it for the process lifetime"]
pub struct TelemetryGuard(Providers);

/// The two states a guard can be in. The exporting path owns both providers; the
/// stdout-only path owns neither. A single enum makes the mixed states (one provider
/// present, the other absent) unrepresentable.
enum Providers {
    /// Stdout-only path — no OTLP providers, nothing to flush.
    Inert,
    /// Owns the OTLP providers; dropping drains their batch buffers.
    Exporting {
        tracer_provider: SdkTracerProvider,
        logger_provider: SdkLoggerProvider,
    },
}

impl TelemetryGuard {
    pub(super) fn new(
        tracer_provider: SdkTracerProvider,
        logger_provider: SdkLoggerProvider,
    ) -> Self {
        Self(Providers::Exporting {
            tracer_provider,
            logger_provider,
        })
    }

    /// A guard for the stdout-only path — nothing to flush.
    pub(super) fn inert() -> Self {
        Self(Providers::Inert)
    }

    /// The owned tracer provider, if this guard is on the exporting path. Lets
    /// [`install`](super::TelemetryConfig::install) publish it as the global
    /// provider without `layers()` having to perform that side effect itself.
    pub(super) fn tracer_provider(&self) -> Option<&SdkTracerProvider> {
        match &self.0 {
            Providers::Inert => None,
            Providers::Exporting {
                tracer_provider, ..
            } => Some(tracer_provider),
        }
    }

    /// Flush and shut down the providers. `Drop` calls this, so explicit calls
    /// are rarely needed.
    pub fn shutdown(&self) {
        let Providers::Exporting {
            tracer_provider,
            logger_provider,
        } = &self.0
        else {
            return;
        };
        if let Err(e) = tracer_provider.shutdown() {
            eprintln!("telemetry: tracer provider shutdown error: {e}");
        }
        if let Err(e) = logger_provider.shutdown() {
            eprintln!("telemetry: logger provider shutdown error: {e}");
        }
    }

    /// Install a panic hook that records the panic through `tracing` and
    /// force-flushes the providers, so the panic record is exported even when a
    /// task panic (which does not unwind `main`) or `panic = "abort"` prevents
    /// the guard's `Drop` from running. The hook captures cloned provider handles
    /// by move, so no global is needed.
    // can cause issues if it panics itself, minor risk for big benefit
    pub(super) fn install_panic_hook(&self) {
        let providers = match &self.0 {
            Providers::Inert => None,
            Providers::Exporting {
                tracer_provider,
                logger_provider,
            } => Some((tracer_provider.clone(), logger_provider.clone())),
        };
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let location = info.location().map(|l| l.to_string()).unwrap_or_default();
            let message = panic_message(info);
            tracing::error!(panic.message = %message, panic.location = %location, "server panicked");

            // Best-effort flush before the process can die.
            if let Some((tracer, logger)) = &providers {
                let _ = tracer.force_flush();
                let _ = logger.force_flush();
            }

            previous(info);
        }));
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    panic_payload_message(info.payload())
}

/// Extract a readable message from a panic payload: `panic!` produces `&str` or
/// `String` payloads; anything else (a `panic_any` value) collapses to a placeholder.
/// Split from [`panic_message`] because a `PanicHookInfo` cannot be constructed in
/// tests, while the payload matching — the part with branches — can be tested directly.
fn panic_payload_message(payload: &dyn std::any::Any) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use opentelemetry::trace::{Tracer as _, TracerProvider as _};
    use opentelemetry_sdk::error::OTelSdkResult;
    use opentelemetry_sdk::logs::InMemoryLogExporter;
    use opentelemetry_sdk::trace::{SpanData, SpanExporter};

    use super::*;

    /// Records exported span names and keeps them across shutdown — unlike the
    /// SDK's `InMemorySpanExporter`, which clears its records when shut down and
    /// so cannot observe a flush that shutdown itself triggered.
    #[derive(Debug, Clone, Default)]
    struct RetainingExporter(Arc<Mutex<Vec<String>>>);

    impl SpanExporter for RetainingExporter {
        async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
            let mut names = self.0.lock().expect("exporter lock");
            names.extend(batch.into_iter().map(|span| span.name.into_owned()));
            Ok(())
        }
    }

    fn exporting_guard() -> (TelemetryGuard, RetainingExporter) {
        let span_exporter = RetainingExporter::default();
        let tracer_provider = SdkTracerProvider::builder()
            .with_batch_exporter(span_exporter.clone())
            .build();
        let logger_provider = SdkLoggerProvider::builder()
            .with_batch_exporter(InMemoryLogExporter::default())
            .build();
        // Emit one span *before* minting the guard; it sits in the batch buffer
        // (default flush interval is seconds away) until something drains it.
        tracer_provider
            .tracer("guard-test")
            .in_span("probe", |_| {});
        (
            TelemetryGuard::new(tracer_provider, logger_provider),
            span_exporter,
        )
    }

    /// The guard's whole contract: dropping it drains the batch buffers, so spans
    /// emitted just before shutdown still reach the exporter.
    #[test]
    fn drop_flushes_buffered_spans() {
        let (guard, span_exporter) = exporting_guard();
        drop(guard);
        let names = span_exporter.0.lock().expect("exporter lock");
        assert_eq!(*names, vec!["probe".to_owned()]);
    }

    #[test]
    fn shutdown_is_idempotent() {
        let (guard, span_exporter) = exporting_guard();
        guard.shutdown();
        guard.shutdown(); // second call may log an already-shut-down error, but must not panic
        drop(guard); // and neither may the shutdown Drop runs
        assert_eq!(
            span_exporter.0.lock().expect("exporter lock").len(),
            1,
            "repeated shutdowns do not duplicate the flushed span"
        );
    }

    #[test]
    fn inert_guard_shutdown_is_a_no_op() {
        let guard = TelemetryGuard::inert();
        guard.shutdown();
        drop(guard);
    }

    #[test]
    fn panic_payload_message_extracts_str_payload() {
        assert_eq!(panic_payload_message(&"boom"), "boom");
    }

    #[test]
    fn panic_payload_message_extracts_string_payload() {
        assert_eq!(panic_payload_message(&"boom".to_owned()), "boom");
    }

    #[test]
    fn panic_payload_message_falls_back_on_unknown_payload() {
        // `std::panic::panic_any` allows arbitrary payloads.
        assert_eq!(panic_payload_message(&42_i32), "unknown panic");
    }

    // `install_panic_hook` itself stays untested: the panic hook is process-global
    // state, and tests run in parallel threads of one process — installing it here
    // would race every other test
}
