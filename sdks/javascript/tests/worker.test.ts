import { describe, it, expect } from "vitest";
import { prepareCloudPayload } from "../src/worker.js";

describe("prepareCloudPayload", () => {
  const events = [
    {
      local_event_uuid: "uuid-1",
      bandit_id: 1,
      arm_id: 1,
      model_name: "gpt-4",
      model_provider: "OpenAI",
      query_text: "hello",
      response: { response: "hi" },
      early_reward: 0.8,
    },
  ];

  it("always strips metadata fields", () => {
    const result = prepareCloudPayload(events, true);
    expect(result[0]).not.toHaveProperty("model_name");
    expect(result[0]).not.toHaveProperty("model_provider");
  });

  it("keeps text when includeText is true", () => {
    const result = prepareCloudPayload(events, true);
    expect(result[0].query_text).toBe("hello");
    expect(result[0].response).toEqual({ response: "hi" });
  });

  it("strips text when includeText is false", () => {
    const result = prepareCloudPayload(events, false);
    expect(result[0]).not.toHaveProperty("query_text");
    expect(result[0]).not.toHaveProperty("response");
  });

  it("preserves other fields", () => {
    const result = prepareCloudPayload(events, false);
    expect(result[0].local_event_uuid).toBe("uuid-1");
    expect(result[0].bandit_id).toBe(1);
    expect(result[0].arm_id).toBe(1);
    expect(result[0].early_reward).toBe(0.8);
  });

  it("does not mutate original events", () => {
    prepareCloudPayload(events, false);
    expect(events[0].model_name).toBe("gpt-4");
    expect(events[0].query_text).toBe("hello");
  });
});
