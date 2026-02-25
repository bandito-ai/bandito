/**
 * Shared test fixtures and utilities.
 */

import type { ArmWire } from "../src/models.js";

/** Standard test arm data matching Python SDK test fixtures. */
export const ARM_DATA: ArmWire[] = [
  {
    arm_id: 1,
    model_name: "gpt-4",
    model_provider: "OpenAI",
    system_prompt: "You are helpful",
    is_prompt_templated: false,
    is_active: true,
    avg_latency_last_n: 400.0,
  },
  {
    arm_id: 2,
    model_name: "claude-sonnet",
    model_provider: "Anthropic",
    system_prompt: "You are helpful",
    is_prompt_templated: false,
    is_active: true,
    avg_latency_last_n: 600.0,
  },
  {
    arm_id: 3,
    model_name: "gpt-4",
    model_provider: "OpenAI",
    system_prompt: "Be concise",
    is_prompt_templated: false,
    is_active: true,
    avg_latency_last_n: null,
  },
];

/** Expected dimensions: 2 models, 2 prompts → 3*2 + 2 = 8 */
export const EXPECTED_DIMS = 8;

/**
 * Build a full sync response matching the backend SyncResponse schema.
 */
export function makeSyncResponse(overrides?: {
  arms?: ArmWire[];
  theta?: number[];
  cholesky?: number[];
  dims?: number;
  optimizationMode?: string;
}) {
  const arms = overrides?.arms ?? ARM_DATA;
  const dims = overrides?.dims ?? EXPECTED_DIMS;
  const theta = overrides?.theta ?? new Array(dims).fill(0.0);
  const cholesky =
    overrides?.cholesky ?? makeIdentityFlat(dims);

  return {
    bandits: [
      {
        bandit_id: 1,
        name: "test-bandit",
        theta,
        cholesky,
        dimensions: dims,
        optimization_mode: overrides?.optimizationMode ?? "base",
        avg_latency_last_n: 500.0,
        budget: null,
        total_cost: null,
        arms,
      },
    ],
  };
}

/** Create a flattened identity matrix. */
export function makeIdentityFlat(d: number): number[] {
  const m = new Array(d * d).fill(0.0);
  for (let i = 0; i < d; i++) {
    m[i * d + i] = 1.0;
  }
  return m;
}
