/// WASM bindings for the Bandito engine.
///
/// Thin wrapper over BanditEngineCore — delegates all logic, only adds
/// wasm-bindgen attributes for the JS boundary.
use wasm_bindgen::prelude::*;

use crate::engine::BanditEngineCore;

/// The engine struct exposed to WASM. Wraps BanditEngineCore.
#[wasm_bindgen]
pub struct BanditEngine {
    inner: BanditEngineCore,
}

#[wasm_bindgen]
impl BanditEngine {
    /// Construct from sync response JSON for one bandit.
    #[wasm_bindgen(constructor)]
    pub fn new(bandit_json: &str) -> Result<BanditEngine, JsValue> {
        BanditEngineCore::create(bandit_json, None)
            .map(|inner| BanditEngine { inner })
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Construct with an explicit RNG seed (for testing).
    #[wasm_bindgen(js_name = newWithSeed)]
    pub fn new_with_seed(bandit_json: &str, seed: Option<u64>) -> Result<BanditEngine, JsValue> {
        BanditEngineCore::create(bandit_json, seed)
            .map(|inner| BanditEngine { inner })
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Pure math pull — returns JSON { arm_index, arm_id, scores }.
    #[wasm_bindgen(js_name = pull)]
    pub fn pull_wasm(
        &mut self,
        query_length: Option<usize>,
        exclude_ids: Option<Vec<i32>>,
    ) -> Result<String, JsValue> {
        self.inner
            .pull_inner(query_length, exclude_ids)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Update state from new sync response.
    #[wasm_bindgen(js_name = updateFromSync)]
    pub fn update_from_sync_wasm(&mut self, bandit_json: &str) -> Result<(), JsValue> {
        self.inner
            .update_from_sync_inner(bandit_json)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Get the bandit ID.
    #[wasm_bindgen(getter, js_name = banditId)]
    pub fn bandit_id(&self) -> i64 {
        self.inner.bandit_id()
    }

    /// Get the bandit name.
    #[wasm_bindgen(getter, js_name = banditName)]
    pub fn bandit_name(&self) -> String {
        self.inner.bandit_name().to_string()
    }

    /// Get dimensions.
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.inner.dimensions()
    }

    /// Get number of arms (all, including inactive).
    #[wasm_bindgen(getter, js_name = numArms)]
    pub fn num_arms(&self) -> usize {
        self.inner.num_arms()
    }

    /// Get the arm metadata as JSON array.
    #[wasm_bindgen(js_name = getArmsJson)]
    pub fn get_arms_json(&self) -> String {
        self.inner.get_arms_json()
    }
}
