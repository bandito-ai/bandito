/// Single source of truth for bandit engine constants.
///
/// Pure module — zero app imports. Safe for SDK and server.

/// Thompson Sampling exploration parameter (beta) per optimization mode.
/// Higher beta = more exploration (wider sampling from posterior).
/// "explore" favors discovering arm quality, "maximize" exploits known-best.
pub fn optimization_beta(mode: &str) -> f64 {
    match mode {
        "explore" => 1.5,
        "base" => 1.0,
        "maximize" => 0.5,
        _ => 1.0,
    }
}

/// Normalization ceiling for cost in composite reward penalty (dollars).
pub const MAX_COST: f64 = 5.0;

/// Normalization ceiling for latency in composite reward penalty (milliseconds).
pub const MAX_LATENCY: f64 = 60_000.0;

/// Jitter added to A_inv diagonal before Cholesky decomposition.
/// Prevents numerical failure when A is near-singular.
pub const CHOLESKY_JITTER: f64 = 1e-6;

/// Cold-start default for query length. log(1) = 0.0 is neutral.
pub const MIN_QUERY_LENGTH: usize = 1;

/// Cold-start default for relative latency. 1.0 means "average latency".
pub const DEFAULT_RELATIVE_LATENCY: f64 = 1.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_betas() {
        assert_eq!(optimization_beta("explore"), 1.5);
        assert_eq!(optimization_beta("base"), 1.0);
        assert_eq!(optimization_beta("maximize"), 0.5);
        assert_eq!(optimization_beta("unknown"), 1.0);
    }

    #[test]
    fn test_constants_values() {
        assert_eq!(MAX_COST, 5.0);
        assert_eq!(MAX_LATENCY, 60_000.0);
        assert_eq!(CHOLESKY_JITTER, 1e-6);
        assert_eq!(MIN_QUERY_LENGTH, 1);
        assert_eq!(DEFAULT_RELATIVE_LATENCY, 1.0);
    }
}
