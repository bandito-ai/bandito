import { describe, it, expect } from "vitest";
import { createArm, createPullResult } from "../src/models.js";

describe("createArm", () => {
  it("creates a frozen arm with correct properties", () => {
    const arm = createArm({
      arm_id: 1,
      model_name: "gpt-4",
      model_provider: "OpenAI",
      system_prompt: "You are helpful",
      is_prompt_templated: false,
    });

    expect(arm.armId).toBe(1);
    expect(arm.modelName).toBe("gpt-4");
    expect(arm.modelProvider).toBe("OpenAI");
    expect(arm.systemPrompt).toBe("You are helpful");
    expect(arm.isPromptTemplated).toBe(false);
    expect(arm.model).toBe("gpt-4");
    expect(arm.prompt).toBe("You are helpful");
    expect(Object.isFrozen(arm)).toBe(true);
  });

  it("defaults isPromptTemplated to false", () => {
    const arm = createArm({
      arm_id: 1,
      model_name: "gpt-4",
      model_provider: "OpenAI",
      system_prompt: "test",
    });
    expect(arm.isPromptTemplated).toBe(false);
  });
});

describe("createPullResult", () => {
  it("creates a frozen pull result with getters", () => {
    const arm = createArm({
      arm_id: 1,
      model_name: "gpt-4",
      model_provider: "OpenAI",
      system_prompt: "You are helpful",
    });

    const result = createPullResult({
      arm,
      eventId: "test-uuid",
      banditId: 1,
      banditName: "test-bandit",
      scores: { 1: 0.5, 2: 0.3 },
      pullTime: 100.0,
    });

    expect(result.arm).toBe(arm);
    expect(result.eventId).toBe("test-uuid");
    expect(result.banditId).toBe(1);
    expect(result.banditName).toBe("test-bandit");
    expect(result.model).toBe("gpt-4");
    expect(result.prompt).toBe("You are helpful");
    expect(result._pullTime).toBe(100.0);
    expect(Object.isFrozen(result)).toBe(true);
    expect(Object.isFrozen(result.scores)).toBe(true);
  });
});
