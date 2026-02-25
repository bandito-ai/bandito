import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { EventStore } from "../src/store.js";

describe("EventStore", () => {
  let store: EventStore;

  beforeEach(() => {
    store = new EventStore(":memory:");
  });

  afterEach(() => {
    store.close();
  });

  it("push and retrieve pending events", () => {
    store.push({
      local_event_uuid: "uuid-1",
      bandit_id: 1,
      arm_id: 1,
      extra_field: "hello",
    });
    store.push({
      local_event_uuid: "uuid-2",
      bandit_id: 1,
      arm_id: 2,
    });

    const pending = store.pending();
    expect(pending).toHaveLength(2);
    expect(pending[0].local_event_uuid).toBe("uuid-1");
    expect(pending[0].extra_field).toBe("hello");
    expect(pending[1].local_event_uuid).toBe("uuid-2");
  });

  it("respects limit on pending", () => {
    for (let i = 0; i < 10; i++) {
      store.push({
        local_event_uuid: `uuid-${i}`,
        bandit_id: 1,
        arm_id: 1,
      });
    }
    const pending = store.pending(3);
    expect(pending).toHaveLength(3);
  });

  it("marks events as flushed", () => {
    store.push({ local_event_uuid: "a", bandit_id: 1, arm_id: 1 });
    store.push({ local_event_uuid: "b", bandit_id: 1, arm_id: 1 });
    store.push({ local_event_uuid: "c", bandit_id: 1, arm_id: 1 });

    store.markFlushed(["a", "b"]);

    const pending = store.pending();
    expect(pending).toHaveLength(1);
    expect(pending[0].local_event_uuid).toBe("c");
  });

  it("deduplicates by UUID (INSERT OR IGNORE)", () => {
    store.push({ local_event_uuid: "dup", bandit_id: 1, arm_id: 1 });
    store.push({ local_event_uuid: "dup", bandit_id: 1, arm_id: 2 });

    const pending = store.pending();
    expect(pending).toHaveLength(1);
    // First insert wins
    expect(pending[0].arm_id).toBe(1);
  });

  it("markFlushed with empty array is a no-op", () => {
    store.markFlushed([]);
    expect(store.pending()).toHaveLength(0);
  });

  it("records human grade locally", () => {
    store.push({ local_event_uuid: "graded", bandit_id: 1, arm_id: 1 });
    store.markFlushed(["graded"]);
    store.markGraded("graded", 0.9);
    // No error = success (we trust SQLite)
  });
});
