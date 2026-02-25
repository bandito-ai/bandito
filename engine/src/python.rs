/// PyO3 bindings for the Bandito engine.
///
/// Thin wrapper over BanditEngineCore — delegates all logic, only adds
/// PyO3 attributes for the Python boundary.
///
/// Same pattern as wasm.rs: JSON string in, JSON string out.
use pyo3::prelude::*;

use crate::engine::BanditEngineCore;

/// Python-facing BanditEngine. Holds all Bayesian state for one bandit.
///
/// Usage:
///     from bandito._engine import BanditEngine
///     engine = BanditEngine('{"bandit_id": 1, ...}')
///     result_json = engine.pull(query_length=100)
#[pyclass(name = "BanditEngine")]
pub struct PyBanditEngine {
    inner: BanditEngineCore,
}

#[pymethods]
impl PyBanditEngine {
    /// Construct from sync response JSON for one bandit.
    /// Optional seed for deterministic RNG (testing).
    #[new]
    #[pyo3(signature = (bandit_json, seed=None))]
    fn new(bandit_json: &str, seed: Option<u64>) -> PyResult<Self> {
        BanditEngineCore::create(bandit_json, seed)
            .map(|inner| PyBanditEngine { inner })
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
    }

    /// Pure math pull — returns JSON string { arm_index, arm_id, scores }.
    #[pyo3(signature = (query_length=None, exclude_ids=None))]
    fn pull(
        &mut self,
        query_length: Option<usize>,
        exclude_ids: Option<Vec<i32>>,
    ) -> PyResult<String> {
        self.inner
            .pull_inner(query_length, exclude_ids)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
    }

    /// Update state from new sync response JSON.
    fn update_from_sync(&mut self, bandit_json: &str) -> PyResult<()> {
        self.inner
            .update_from_sync_inner(bandit_json)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
    }

    /// Get the bandit ID.
    #[getter]
    fn bandit_id(&self) -> i64 {
        self.inner.bandit_id()
    }

    /// Get the bandit name.
    #[getter]
    fn bandit_name(&self) -> &str {
        self.inner.bandit_name()
    }

    /// Get dimensions.
    #[getter]
    fn dimensions(&self) -> usize {
        self.inner.dimensions()
    }

    /// Get number of arms (all, including inactive).
    #[getter]
    fn num_arms(&self) -> usize {
        self.inner.num_arms()
    }

    /// Get the arm metadata as JSON array string.
    fn get_arms_json(&self) -> String {
        self.inner.get_arms_json()
    }
}

/// Python module entry point.
#[pymodule]
fn _engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBanditEngine>()?;
    Ok(())
}
