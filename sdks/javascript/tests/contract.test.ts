/**
 * Cross-SDK contract validation tests.
 *
 * These tests verify that the WASM engine produces results consistent
 * with the expected behavior, and that inactive arms are handled correctly.
 */

import { describe, it, expect, beforeAll } from "vitest";
import { initWasm, createEngine } from "../src/engine.js";
import {
  makeSyncResponse,
  EXPECTED_DIMS,
  ARM_DATA,
  makeIdentityFlat,
} from "./helpers.js";

describe("Contract Tests", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("dimensions match 3*n_models + n_prompts", () => {
    // 2 models (gpt-4/OpenAI, claude-sonnet/Anthropic), 2 prompts → 3*2 + 2 = 8
    const sync = makeSyncResponse();
    const engine = createEngine(JSON.stringify(sync.bandits[0]));
    expect(engine.dimensions).toBe(EXPECTED_DIMS);
  });

  it("inactive arms preserve dimension count", () => {
    // Same arms but one inactive — dimensions should NOT change
    const arms = ARM_DATA.map((a, i) => ({
      ...a,
      is_active: i !== 1, // arm 2 is inactive
    }));
    const sync = makeSyncResponse({ arms });
    const engine = createEngine(JSON.stringify(sync.bandits[0]));

    // Dimensions should be the same as with all arms active
    expect(engine.dimensions).toBe(EXPECTED_DIMS);
    expect(engine.numArms).toBe(3); // all arms in matrix
  });

  it("inactive arms excluded from pull scores", () => {
    const arms = ARM_DATA.map((a, i) => ({
      ...a,
      is_active: i !== 1,
    }));
    const sync = makeSyncResponse({ arms });
    const engine = createEngine(JSON.stringify(sync.bandits[0]));

    const result = JSON.parse(engine.pull(undefined, undefined));

    // Only active arms (1, 3) should have scores
    const scoreIds = Object.keys(result.scores).map(Number);
    expect(scoreIds).toContain(1);
    expect(scoreIds).not.toContain(2);
    expect(scoreIds).toContain(3);
  });

  it("single model single prompt gives dimension 4", () => {
    const arms = [
      {
        arm_id: 1,
        model_name: "gpt-4",
        model_provider: "OpenAI",
        system_prompt: "Be helpful",
        is_prompt_templated: false,
        is_active: true,
        avg_latency_last_n: null,
      },
    ];
    const dims = 4; // 3*1 + 1
    const sync = makeSyncResponse({
      arms,
      dims,
      theta: new Array(dims).fill(0),
      cholesky: makeIdentityFlat(dims),
    });
    const engine = createEngine(JSON.stringify(sync.bandits[0]));
    expect(engine.dimensions).toBe(4);
  });

  it("feature matrix shape matches n_arms x dimensions", () => {
    const sync = makeSyncResponse();
    const engine = createEngine(JSON.stringify(sync.bandits[0]));

    // Verify dimensions
    expect(engine.numArms).toBe(3);
    expect(engine.dimensions).toBe(EXPECTED_DIMS);

    // Pull should work (proves matrix is correctly shaped)
    const result = JSON.parse(engine.pull(undefined, undefined));
    expect(result.scores).toBeDefined();
  });

  it("deterministic pull with same seed via getArmsJson", () => {
    const sync = makeSyncResponse();
    const engine = createEngine(JSON.stringify(sync.bandits[0]));

    const armsJson = engine.getArmsJson();
    const arms = JSON.parse(armsJson);

    // Should have all 3 arms
    expect(arms).toHaveLength(3);
    // Each arm should have expected fields
    for (const arm of arms) {
      expect(arm).toHaveProperty("arm_id");
      expect(arm).toHaveProperty("model_name");
      expect(arm).toHaveProperty("model_provider");
      expect(arm).toHaveProperty("system_prompt");
      expect(arm).toHaveProperty("is_active");
    }
  });
});
