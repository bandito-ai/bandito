/// Bandito Engine — single source of truth for Thompson Sampling math.
///
/// Compiled to:
/// - WASM (via wasm-bindgen, `--features wasm`) for the JS/TS SDK
/// - Native Python extension (via PyO3, `--features python`) for the Python SDK
/// - Pure library (no features) for testing and backend use
pub mod constants;
pub mod engine;
pub mod features;
pub mod linalg;
pub mod rng;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "python")]
pub mod python;

pub use constants::*;
pub use engine::{BanditEngineCore, PullOutput};
pub use features::{ArmIdentity, ArmIndexMap, build_feature_matrix, compute_dimensions};
pub use linalg::{
    bayesian_update_delta, bayesian_update_full, compute_posterior, matvec, safe_cholesky,
    sample_thompson,
};
pub use rng::Xoshiro256StarStar;
