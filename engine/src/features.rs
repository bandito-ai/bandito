/// Feature engineering for shared Linear Thompson Sampling.
///
/// All arms in a bandit share ONE posterior (theta, A, b). Arms are differentiated
/// by feature engineering: each arm gets a unique feature vector encoding its
/// identity (one-hot) plus context interaction terms.
///
/// Feature vector layout for an arm with M unique models and P unique prompts:
///
/// ```text
/// [model_one_hot(M) | prompt_one_hot(P) | log_query_len * model(M) | rel_latency * model(M)]
///  --- M dims ---   --- P dims ---      --- M dims ---              --- M dims ---
/// Total dimensions = 3M + P
/// ```
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Minimal arm representation decoupled from ORM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmIdentity {
    pub arm_id: i64,
    pub model_name: String,
    pub model_provider: String,
    pub system_prompt: String,
}

/// Compute total feature vector dimensionality.
/// dims = 3 * n_models + n_prompts
pub fn compute_dimensions(n_models: usize, n_prompts: usize) -> usize {
    3 * n_models + n_prompts
}

/// Maps arm identities to one-hot indices for feature construction.
///
/// Built once per bandit configuration. Sorting by arm_id ensures deterministic
/// ordering regardless of DB query order or insertion sequence.
#[derive(Debug, Clone)]
pub struct ArmIndexMap {
    pub model_to_index: HashMap<(String, String), usize>,
    pub prompt_to_index: HashMap<String, usize>,
    pub n_models: usize,
    pub n_prompts: usize,
    pub dimensions: usize,
}

impl ArmIndexMap {
    /// Build index mappings from a list of arm identities.
    ///
    /// Arms are sorted by arm_id to guarantee deterministic index assignment.
    pub fn from_arms(arms: &[ArmIdentity]) -> Result<Self, String> {
        if arms.is_empty() {
            return Err("Cannot build index map from empty arm list".to_string());
        }

        let mut sorted_arms: Vec<&ArmIdentity> = arms.iter().collect();
        sorted_arms.sort_by_key(|a| a.arm_id);

        let mut model_to_index: HashMap<(String, String), usize> = HashMap::new();
        let mut prompt_to_index: HashMap<String, usize> = HashMap::new();

        for arm in &sorted_arms {
            let key = (arm.model_name.clone(), arm.model_provider.clone());
            let model_count = model_to_index.len();
            model_to_index.entry(key).or_insert(model_count);

            let prompt_count = prompt_to_index.len();
            prompt_to_index
                .entry(arm.system_prompt.clone())
                .or_insert(prompt_count);
        }

        let n_models = model_to_index.len();
        let n_prompts = prompt_to_index.len();

        Ok(ArmIndexMap {
            model_to_index,
            prompt_to_index,
            n_models,
            n_prompts,
            dimensions: compute_dimensions(n_models, n_prompts),
        })
    }

    /// Get the model index for a given arm identity.
    pub fn model_index(&self, model_name: &str, model_provider: &str) -> Option<usize> {
        self.model_to_index
            .get(&(model_name.to_string(), model_provider.to_string()))
            .copied()
    }

    /// Get the prompt index for a given system prompt.
    pub fn prompt_index(&self, system_prompt: &str) -> Option<usize> {
        self.prompt_to_index.get(system_prompt).copied()
    }
}

/// Build a pre-allocated feature matrix with static one-hot blocks filled.
/// Context columns (log_query_len, rel_latency) are left as zeros.
///
/// Returns a flattened row-major matrix of shape (n_arms, dimensions).
pub fn build_feature_matrix(arms: &[ArmIdentity], index_map: &ArmIndexMap) -> Vec<f64> {
    let n_arms = arms.len();
    let dims = index_map.dimensions;
    let mut matrix = vec![0.0f64; n_arms * dims];

    // Iterate arms in their given order so that matrix row i corresponds
    // to arm_identities[i]. The ArmIndexMap handles deterministic model/prompt
    // index assignment via its own sort — row order is separate.
    for (row, arm) in arms.iter().enumerate() {
        let model_idx = index_map
            .model_index(&arm.model_name, &arm.model_provider)
            .expect("arm model not in index map");
        let prompt_idx = index_map
            .prompt_index(&arm.system_prompt)
            .expect("arm prompt not in index map");

        let base = row * dims;

        // Block 1: model one-hot [0, M)
        matrix[base + model_idx] = 1.0;

        // Block 2: prompt one-hot [M, M+P)
        matrix[base + index_map.n_models + prompt_idx] = 1.0;
    }

    matrix
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_arms() -> Vec<ArmIdentity> {
        vec![
            ArmIdentity {
                arm_id: 1,
                model_name: "gpt-4".to_string(),
                model_provider: "OpenAI".to_string(),
                system_prompt: "You are helpful".to_string(),
            },
            ArmIdentity {
                arm_id: 2,
                model_name: "claude-sonnet".to_string(),
                model_provider: "Anthropic".to_string(),
                system_prompt: "You are helpful".to_string(),
            },
            ArmIdentity {
                arm_id: 3,
                model_name: "gpt-4".to_string(),
                model_provider: "OpenAI".to_string(),
                system_prompt: "Be concise".to_string(),
            },
        ]
    }

    #[test]
    fn test_compute_dimensions() {
        assert_eq!(compute_dimensions(3, 2), 11);
        assert_eq!(compute_dimensions(1, 1), 4);
        assert_eq!(compute_dimensions(2, 2), 8);
    }

    #[test]
    fn test_arm_index_map_from_arms() {
        let arms = make_arms();
        let idx = ArmIndexMap::from_arms(&arms).unwrap();

        // 2 unique models: (gpt-4, OpenAI), (claude-sonnet, Anthropic)
        assert_eq!(idx.n_models, 2);
        // 2 unique prompts: "You are helpful", "Be concise"
        assert_eq!(idx.n_prompts, 2);
        // dims = 3*2 + 2 = 8
        assert_eq!(idx.dimensions, 8);
    }

    #[test]
    fn test_arm_index_map_empty() {
        let arms: Vec<ArmIdentity> = vec![];
        assert!(ArmIndexMap::from_arms(&arms).is_err());
    }

    #[test]
    fn test_arm_index_map_deterministic_ordering() {
        let arms = make_arms();
        let idx1 = ArmIndexMap::from_arms(&arms).unwrap();

        // Reversed order should produce the same mapping
        let mut reversed = arms.clone();
        reversed.reverse();
        let idx2 = ArmIndexMap::from_arms(&reversed).unwrap();

        assert_eq!(idx1.n_models, idx2.n_models);
        assert_eq!(idx1.n_prompts, idx2.n_prompts);
        assert_eq!(idx1.dimensions, idx2.dimensions);
        assert_eq!(idx1.model_to_index, idx2.model_to_index);
        assert_eq!(idx1.prompt_to_index, idx2.prompt_to_index);
    }

    #[test]
    fn test_build_feature_matrix() {
        let arms = make_arms();
        let idx = ArmIndexMap::from_arms(&arms).unwrap();
        let matrix = build_feature_matrix(&arms, &idx);

        let dims = idx.dimensions;
        assert_eq!(matrix.len(), 3 * dims); // 3 arms x 8 dims

        // Check each row has exactly 2 non-zero entries (model + prompt one-hot)
        for row in 0..3 {
            let row_sum: f64 = matrix[row * dims..(row + 1) * dims].iter().sum();
            assert_eq!(row_sum, 2.0, "Row {} should have exactly 2 one-hot entries", row);
        }
    }
}
