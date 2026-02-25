import { describe, it, expect, beforeAll } from "vitest";
import { initWasm, createEngine } from "../src/engine.js";
import { makeSyncResponse, EXPECTED_DIMS } from "./helpers.js";

describe("WASM Engine", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("creates an engine from sync response", () => {
    const sync = makeSyncResponse();
    const banditJson = JSON.stringify(sync.bandits[0]);
    const engine = createEngine(banditJson);

    expect(Number(engine.banditId)).toBe(1);
    expect(engine.banditName).toBe("test-bandit");
    expect(engine.dimensions).toBe(EXPECTED_DIMS);
    expect(engine.numArms).toBe(3);
  });

  it("pull returns valid arm selection", () => {
    const sync = makeSyncResponse();
    const engine = createEngine(JSON.stringify(sync.bandits[0]));

    const resultJson = engine.pull(100, undefined);
    const result = JSON.parse(resultJson);

    expect(result).toHaveProperty("arm_id");
    expect(result).toHaveProperty("arm_index");
    expect(result).toHaveProperty("scores");
    expect(Object.keys(result.scores)).toHaveLength(3);
  });

  it("pull with exclude masks arms", () => {
    const sync = makeSyncResponse();
    const engine = createEngine(JSON.stringify(sync.bandits[0]));

    // Exclude arms 1 and 2
    const resultJson = engine.pull(undefined, [1, 2]);
    const result = JSON.parse(resultJson);

    expect(result.arm_id).toBe(3);
    expect(Object.keys(result.scores)).toHaveLength(1);
  });

  it("inactive arms are excluded from scores", () => {
    const arms = [
      { ...makeSyncResponse().bandits[0].arms[0], is_active: true },
      { ...makeSyncResponse().bandits[0].arms[1], is_active: false },
      { ...makeSyncResponse().bandits[0].arms[2], is_active: true },
    ];
    const sync = makeSyncResponse({ arms });
    const engine = createEngine(JSON.stringify(sync.bandits[0]));

    const resultJson = engine.pull(undefined, undefined);
    const result = JSON.parse(resultJson);

    // Only 2 active arms should have scores
    expect(Object.keys(result.scores)).toHaveLength(2);
    expect(result.scores).not.toHaveProperty("2");
  });

  it("updateFromSync refreshes state", () => {
    const sync = makeSyncResponse();
    const engine = createEngine(JSON.stringify(sync.bandits[0]));

    // Update with new theta
    const newSync = makeSyncResponse({
      theta: new Array(EXPECTED_DIMS).fill(0.5),
      optimizationMode: "explore",
    });
    engine.updateFromSync(JSON.stringify(newSync.bandits[0]));

    // Should still produce valid pulls
    const resultJson = engine.pull(undefined, undefined);
    const result = JSON.parse(resultJson);
    expect(result).toHaveProperty("arm_id");
  });

  it("dimensions match expected value", () => {
    const sync = makeSyncResponse();
    const engine = createEngine(JSON.stringify(sync.bandits[0]));
    // 2 models, 2 prompts → 3*2 + 2 = 8
    expect(engine.dimensions).toBe(8);
  });
});
