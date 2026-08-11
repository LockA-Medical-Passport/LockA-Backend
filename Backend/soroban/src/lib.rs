//! Stellar/Soroban RPC client, transaction building, and contract bindings
//! (see issue #11 onward).
//!
//! Instrumentation convention: annotate every RPC-calling function with
//! `#[tracing::instrument(skip(...))]` (skip large/sensitive arguments such
//! as signing keys or full transaction payloads). The subscriber configured
//! in `telemetry::init` records a `close` event with `time.busy` /
//! `time.idle` for every span, so instrumenting a function is all that's
//! needed to make its latency observable — no manual timing code.

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
