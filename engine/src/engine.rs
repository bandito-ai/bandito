/// Core BanditEngine — shared by both WASM and PyO3 wrappers.
///
/// Holds all Bayesian state for one bandit. Binding-specific wrappers
/// (wasm.rs, python.rs) delegate to this struct.
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::constants::{optimization_beta, DEFAULT_RELATIVE_LATENCY, MIN_QUERY_LENGTH};
use crate::features::{ArmIdentity, ArmIndexMap, build_feature_matrix};
use crate::linalg::{matvec, sample_thompson};
use crate::rng::Xoshiro256StarStar;

/// Sync response schema for one bandit (matches backend SyncResponse).
#[derive(Deserialize)]
pub(crate) struct BanditSync {
    pub bandit_id: i64,
    pub name: String,
    pub theta: Vec<f64>,
    #[serde(deserialize_with = "deserialize_flat_or_nested")]
    pub cholesky: Vec<f64>,
    pub dimensions: usize,
    #[serde(default = "default_optimization_mode")]
    pub optimization_mode: String,
    #[serde(default)]
    pub avg_latency_last_n: Option<f64>,
    #[serde(default)]
    pub arms: Vec<ArmSync>,
}

/// Accept either a flat [f64] array or a nested [[f64]] matrix and flatten it.
fn deserialize_flat_or_nested<'de, D>(deserializer: D) -> Result<Vec<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlatOrNested {
        Flat(Vec<f64>),
        Nested(Vec<Vec<f64>>),
    }

    match FlatOrNested::deserialize(deserializer)? {
        FlatOrNested::Flat(v) => Ok(v),
        FlatOrNested::Nested(rows) => Ok(rows.into_iter().flatten().collect()),
    }
}

fn default_optimization_mode() -> String {
    "base".to_string()
}

#[derive(Deserialize)]
pub(crate) struct ArmSync {
    pub arm_id: i64,
    pub model_name: String,
    pub model_provider: String,
    pub system_prompt: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub is_prompt_templated: bool,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub avg_latency_last_n: Option<f64>,
}

fn default_true() -> bool {
    true
}

/// Pull result returned as JSON from pull().
#[derive(Serialize, Deserialize)]
pub struct PullOutput {
    pub arm_index: usize,
    pub arm_id: i64,
    pub scores: HashMap<i64, f64>,
}

/// The core engine struct — no binding attributes.
///
/// Holds all Bayesian state for one bandit. WASM and PyO3 wrappers
/// each hold an instance of this and delegate calls.
pub struct BanditEngineCore {
    pub(crate) index_map: ArmIndexMap,
    pub(crate) theta: Vec<f64>,
    pub(crate) chol: Vec<f64>,          // flattened d x d row-major
    pub(crate) feature_matrix: Vec<f64>, // flattened n_arms x d row-major
    pub(crate) dimensions: usize,
    pub(crate) n_arms: usize,
    pub(crate) optimization_mode: String,
    pub(crate) active_arm_ids: HashSet<i64>,
    pub(crate) arm_ids: Vec<i64>,       // ordered (sorted by arm_id), all arms
    pub(crate) arm_identities: Vec<ArmIdentity>,
    pub(crate) arm_avg_latencies: HashMap<i64, Option<f64>>,
    pub(crate) avg_latency_last_n: Option<f64>,
    pub(crate) rng: Xoshiro256StarStar,
    // Metadata passed through for the client
    pub(crate) bandit_id: i64,
    pub(crate) bandit_name: String,
}

impl BanditEngineCore {
    /// Create a new engine from JSON. Testable on all targets.
    pub fn create(bandit_json: &str, seed: Option<u64>) -> Result<Self, String> {
        let b: BanditSync =
            serde_json::from_str(bandit_json).map_err(|e| e.to_string())?;
        Self::from_sync(b, seed)
    }

    /// Pull implementation returning Result<String, String>.
    pub fn pull_inner(
        &mut self,
        query_length: Option<usize>,
        exclude_ids: Option<Vec<i32>>,
    ) -> Result<String, String> {
        if self.n_arms == 0 {
            return Err("No arms available".to_string());
        }

        let beta = optimization_beta(&self.optimization_mode);
        let theta_tilde = sample_thompson(
            &self.theta,
            &self.chol,
            self.dimensions,
            beta,
            &mut self.rng,
        );

        // Update context columns in feature matrix
        let ql = query_length
            .map(|q| q.max(MIN_QUERY_LENGTH))
            .unwrap_or(MIN_QUERY_LENGTH);
        let log_ql = (ql as f64).ln();

        let d = self.dimensions;
        let m = &self.index_map;

        for (i, identity) in self.arm_identities.iter().enumerate() {
            let model_idx = m
                .model_index(&identity.model_name, &identity.model_provider)
                .ok_or_else(|| "arm model not in index map".to_string())?;

            // Block 3: log(query_length) * model [M+P, 2M+P)
            self.feature_matrix[i * d + m.n_models + m.n_prompts + model_idx] = log_ql;

            // Block 4: relative_latency * model [2M+P, 3M+P)
            let arm_latency = self.arm_avg_latencies.get(&identity.arm_id).copied().flatten();
            let bandit_latency = self.avg_latency_last_n;
            let rl = match (arm_latency, bandit_latency) {
                (Some(al), Some(bl)) if bl > 0.0 => al / bl,
                _ => DEFAULT_RELATIVE_LATENCY,
            };
            self.feature_matrix[i * d + 2 * m.n_models + m.n_prompts + model_idx] = rl;
        }

        // Score: feature_matrix @ theta_tilde
        let scores_array = matvec(&self.feature_matrix, &theta_tilde, self.n_arms, d);

        // Build scores map, masking inactive and excluded arms
        let exclude_set: HashSet<i64> = exclude_ids
            .unwrap_or_default()
            .into_iter()
            .map(|id| id as i64)
            .collect();

        let mut scores: HashMap<i64, f64> = HashMap::new();
        let mut best_idx: Option<usize> = None;
        let mut best_score = f64::NEG_INFINITY;

        for (i, identity) in self.arm_identities.iter().enumerate() {
            let is_active = self.active_arm_ids.contains(&identity.arm_id);
            let is_excluded = exclude_set.contains(&identity.arm_id);

            if is_active && !is_excluded {
                scores.insert(identity.arm_id, scores_array[i]);
                if scores_array[i] > best_score {
                    best_score = scores_array[i];
                    best_idx = Some(i);
                }
            }
        }

        let winner_idx = best_idx
            .ok_or_else(|| "All arms excluded or inactive".to_string())?;

        let output = PullOutput {
            arm_index: winner_idx,
            arm_id: self.arm_identities[winner_idx].arm_id,
            scores,
        };

        serde_json::to_string(&output).map_err(|e| e.to_string())
    }

    /// Update from sync — inner implementation.
    pub fn update_from_sync_inner(&mut self, bandit_json: &str) -> Result<(), String> {
        let b: BanditSync =
            serde_json::from_str(bandit_json).map_err(|e| e.to_string())?;

        let identities: Vec<ArmIdentity> = b
            .arms
            .iter()
            .map(|a| ArmIdentity {
                arm_id: a.arm_id,
                model_name: a.model_name.clone(),
                model_provider: a.model_provider.clone(),
                system_prompt: a.system_prompt.clone(),
            })
            .collect();

        if identities.is_empty() {
            return Err("No arms in sync response".to_string());
        }

        let index_map = ArmIndexMap::from_arms(&identities)?;

        let active_arm_ids: HashSet<i64> = b
            .arms
            .iter()
            .filter(|a| a.is_active)
            .map(|a| a.arm_id)
            .collect();

        let mut arm_ids: Vec<i64> = identities.iter().map(|a| a.arm_id).collect();
        arm_ids.sort();

        let arm_avg_latencies: HashMap<i64, Option<f64>> = b
            .arms
            .iter()
            .map(|a| (a.arm_id, a.avg_latency_last_n))
            .collect();

        self.feature_matrix = build_feature_matrix(&identities, &index_map);
        self.index_map = index_map;
        self.theta = b.theta;
        self.chol = b.cholesky;
        self.dimensions = b.dimensions;
        self.n_arms = identities.len();
        self.optimization_mode = b.optimization_mode;
        self.active_arm_ids = active_arm_ids;
        self.arm_ids = arm_ids;
        self.arm_identities = identities;
        self.arm_avg_latencies = arm_avg_latencies;
        self.avg_latency_last_n = b.avg_latency_last_n;
        self.bandit_id = b.bandit_id;
        self.bandit_name = b.name;

        Ok(())
    }

    // --- Getters ---

    pub fn bandit_id(&self) -> i64 {
        self.bandit_id
    }

    pub fn bandit_name(&self) -> &str {
        &self.bandit_name
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    pub fn num_arms(&self) -> usize {
        self.n_arms
    }

    pub fn get_arms_json(&self) -> String {
        #[derive(Serialize)]
        struct ArmInfo {
            arm_id: i64,
            model_name: String,
            model_provider: String,
            system_prompt: String,
            is_active: bool,
            avg_latency_last_n: Option<f64>,
        }

        let arms: Vec<ArmInfo> = self
            .arm_identities
            .iter()
            .map(|a| ArmInfo {
                arm_id: a.arm_id,
                model_name: a.model_name.clone(),
                model_provider: a.model_provider.clone(),
                system_prompt: a.system_prompt.clone(),
                is_active: self.active_arm_ids.contains(&a.arm_id),
                avg_latency_last_n: self.arm_avg_latencies.get(&a.arm_id).copied().flatten(),
            })
            .collect();

        serde_json::to_string(&arms).unwrap_or_else(|_| "[]".to_string())
    }

    pub(crate) fn from_sync(b: BanditSync, seed: Option<u64>) -> Result<BanditEngineCore, String> {
        let identities: Vec<ArmIdentity> = b
            .arms
            .iter()
            .map(|a| ArmIdentity {
                arm_id: a.arm_id,
                model_name: a.model_name.clone(),
                model_provider: a.model_provider.clone(),
                system_prompt: a.system_prompt.clone(),
            })
            .collect();

        if identities.is_empty() {
            return Err("No arms in sync response".to_string());
        }

        let index_map = ArmIndexMap::from_arms(&identities)?;

        let active_arm_ids: HashSet<i64> = b
            .arms
            .iter()
            .filter(|a| a.is_active)
            .map(|a| a.arm_id)
            .collect();

        let mut arm_ids: Vec<i64> = identities.iter().map(|a| a.arm_id).collect();
        arm_ids.sort();

        let arm_avg_latencies: HashMap<i64, Option<f64>> = b
            .arms
            .iter()
            .map(|a| (a.arm_id, a.avg_latency_last_n))
            .collect();

        let feature_matrix = build_feature_matrix(&identities, &index_map);

        let rng_seed = seed.unwrap_or_else(|| {
            let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
            for byte in b.name.as_bytes() {
                h ^= *byte as u64;
                h = h.wrapping_mul(0x100000001b3); // FNV prime
            }
            h ^= b.bandit_id as u64;
            h
        });

        Ok(BanditEngineCore {
            n_arms: identities.len(),
            dimensions: b.dimensions,
            index_map,
            theta: b.theta,
            chol: b.cholesky,
            feature_matrix,
            optimization_mode: b.optimization_mode,
            active_arm_ids,
            arm_ids,
            arm_identities: identities,
            arm_avg_latencies,
            avg_latency_last_n: b.avg_latency_last_n,
            rng: Xoshiro256StarStar::new(rng_seed),
            bandit_id: b.bandit_id,
            bandit_name: b.name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bandit_json() -> String {
        let dims = 8; // 3*2 + 2 = 8 (2 models, 2 prompts)
        let theta = vec![0.0; dims];
        let mut chol = vec![0.0; dims * dims];
        for i in 0..dims {
            chol[i * dims + i] = 1.0;
        }

        serde_json::json!({
            "bandit_id": 1,
            "name": "test-bandit",
            "theta": theta,
            "cholesky": chol,
            "dimensions": dims,
            "optimization_mode": "base",
            "avg_latency_last_n": 500.0,
            "arms": [
                {
                    "arm_id": 1,
                    "model_name": "gpt-4",
                    "model_provider": "OpenAI",
                    "system_prompt": "You are helpful",
                    "is_prompt_templated": false,
                    "is_active": true,
                    "avg_latency_last_n": 400.0
                },
                {
                    "arm_id": 2,
                    "model_name": "claude-sonnet",
                    "model_provider": "Anthropic",
                    "system_prompt": "You are helpful",
                    "is_prompt_templated": false,
                    "is_active": true,
                    "avg_latency_last_n": 600.0
                },
                {
                    "arm_id": 3,
                    "model_name": "gpt-4",
                    "model_provider": "OpenAI",
                    "system_prompt": "Be concise",
                    "is_prompt_templated": false,
                    "is_active": true,
                    "avg_latency_last_n": null
                }
            ]
        })
        .to_string()
    }

    #[test]
    fn test_engine_new() {
        let json = make_bandit_json();
        let engine = BanditEngineCore::create(&json, Some(42)).unwrap();
        assert_eq!(engine.bandit_id(), 1);
        assert_eq!(engine.bandit_name(), "test-bandit");
        assert_eq!(engine.dimensions(), 8);
        assert_eq!(engine.num_arms(), 3);
    }

    #[test]
    fn test_engine_pull() {
        let json = make_bandit_json();
        let mut engine = BanditEngineCore::create(&json, Some(42)).unwrap();

        let result_json = engine.pull_inner(Some(100), None).unwrap();
        let result: PullOutput = serde_json::from_str(&result_json).unwrap();

        // Should pick one of the 3 arms
        assert!(result.scores.len() == 3);
        assert!(result.scores.contains_key(&result.arm_id));
    }

    #[test]
    fn test_engine_pull_deterministic() {
        let json = make_bandit_json();
        let mut engine1 = BanditEngineCore::create(&json, Some(42)).unwrap();
        let mut engine2 = BanditEngineCore::create(&json, Some(42)).unwrap();

        let r1: PullOutput = serde_json::from_str(&engine1.pull_inner(Some(100), None).unwrap()).unwrap();
        let r2: PullOutput = serde_json::from_str(&engine2.pull_inner(Some(100), None).unwrap()).unwrap();
        assert_eq!(r1.arm_id, r2.arm_id);
        assert_eq!(r1.arm_index, r2.arm_index);
        assert_eq!(r1.scores, r2.scores);
    }

    #[test]
    fn test_engine_pull_with_exclude() {
        let json = make_bandit_json();
        let mut engine = BanditEngineCore::create(&json, Some(42)).unwrap();

        // Exclude arms 1 and 2, only arm 3 should be selectable
        let result_json = engine.pull_inner(None, Some(vec![1, 2])).unwrap();
        let result: PullOutput = serde_json::from_str(&result_json).unwrap();
        assert_eq!(result.arm_id, 3);
        assert_eq!(result.scores.len(), 1);
    }

    #[test]
    fn test_engine_pull_all_excluded() {
        let json = make_bandit_json();
        let mut engine = BanditEngineCore::create(&json, Some(42)).unwrap();

        let result = engine.pull_inner(None, Some(vec![1, 2, 3]));
        assert!(result.is_err());
    }

    #[test]
    fn test_engine_inactive_arms() {
        let dims = 8;
        let theta = vec![0.0; dims];
        let mut chol = vec![0.0; dims * dims];
        for i in 0..dims {
            chol[i * dims + i] = 1.0;
        }

        let json = serde_json::json!({
            "bandit_id": 1,
            "name": "test-bandit",
            "theta": theta,
            "cholesky": chol,
            "dimensions": dims,
            "arms": [
                {
                    "arm_id": 1,
                    "model_name": "gpt-4",
                    "model_provider": "OpenAI",
                    "system_prompt": "You are helpful",
                    "is_active": true
                },
                {
                    "arm_id": 2,
                    "model_name": "claude-sonnet",
                    "model_provider": "Anthropic",
                    "system_prompt": "You are helpful",
                    "is_active": false
                },
                {
                    "arm_id": 3,
                    "model_name": "gpt-4",
                    "model_provider": "OpenAI",
                    "system_prompt": "Be concise",
                    "is_active": true
                }
            ]
        })
        .to_string();

        let mut engine = BanditEngineCore::create(&json, Some(42)).unwrap();
        assert_eq!(engine.num_arms(), 3);

        let result_json = engine.pull_inner(None, None).unwrap();
        let result: PullOutput = serde_json::from_str(&result_json).unwrap();

        // Only active arms (1 and 3) should have scores
        assert_eq!(result.scores.len(), 2);
        assert!(result.scores.contains_key(&1));
        assert!(!result.scores.contains_key(&2));
        assert!(result.scores.contains_key(&3));
    }

    #[test]
    fn test_engine_update_from_sync() {
        let json = make_bandit_json();
        let mut engine = BanditEngineCore::create(&json, Some(42)).unwrap();

        let dims = 8;
        let theta = vec![0.5; dims];
        let mut chol = vec![0.0; dims * dims];
        for i in 0..dims {
            chol[i * dims + i] = 1.0;
        }

        let new_json = serde_json::json!({
            "bandit_id": 1,
            "name": "test-bandit",
            "theta": theta,
            "cholesky": chol,
            "dimensions": dims,
            "optimization_mode": "explore",
            "arms": [
                {
                    "arm_id": 1,
                    "model_name": "gpt-4",
                    "model_provider": "OpenAI",
                    "system_prompt": "You are helpful",
                    "is_active": true
                },
                {
                    "arm_id": 2,
                    "model_name": "claude-sonnet",
                    "model_provider": "Anthropic",
                    "system_prompt": "You are helpful",
                    "is_active": true
                },
                {
                    "arm_id": 3,
                    "model_name": "gpt-4",
                    "model_provider": "OpenAI",
                    "system_prompt": "Be concise",
                    "is_active": true
                }
            ]
        })
        .to_string();

        engine.update_from_sync_inner(&new_json).unwrap();
        let result = engine.pull_inner(None, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_engine_nested_cholesky() {
        // Backend sends cholesky as [[f64]] (2D matrix). Engine should flatten it.
        let dims = 8;
        let theta = vec![0.0; dims];
        let mut chol_nested: Vec<Vec<f64>> = vec![vec![0.0; dims]; dims];
        for i in 0..dims {
            chol_nested[i][i] = 1.0;
        }

        let json = serde_json::json!({
            "bandit_id": 1,
            "name": "test-nested",
            "theta": theta,
            "cholesky": chol_nested,
            "dimensions": dims,
            "arms": [
                {
                    "arm_id": 1,
                    "model_name": "gpt-4",
                    "model_provider": "OpenAI",
                    "system_prompt": "You are helpful",
                    "is_active": true
                },
                {
                    "arm_id": 2,
                    "model_name": "claude-sonnet",
                    "model_provider": "Anthropic",
                    "system_prompt": "You are helpful",
                    "is_active": true
                },
                {
                    "arm_id": 3,
                    "model_name": "gpt-4",
                    "model_provider": "OpenAI",
                    "system_prompt": "Be concise",
                    "is_active": true
                }
            ]
        })
        .to_string();

        let mut engine = BanditEngineCore::create(&json, Some(42)).unwrap();
        let result = engine.pull_inner(Some(100), None);
        assert!(result.is_ok());
    }
}
