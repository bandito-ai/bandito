import { describe, it, expect, beforeAll, afterAll, afterEach } from "vitest";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import { BanditoClient } from "../src/client.js";
import { makeSyncResponse } from "./helpers.js";

const BASE_URL = "http://localhost:9998";
const syncData = makeSyncResponse();

const handlers = [
  http.post(`${BASE_URL}/api/v1/sync/connect`, () => {
    return HttpResponse.json(syncData);
  }),
  http.post(`${BASE_URL}/api/v1/sync/heartbeat`, () => {
    return HttpResponse.json(syncData);
  }),
  http.post(`${BASE_URL}/api/v1/events`, async ({ request }) => {
    const body = (await request.json()) as { events: unknown[] };
    return HttpResponse.json({
      accepted: body.events?.length ?? 0,
      errors: [],
    });
  }),
  http.patch(`${BASE_URL}/api/v1/events/:uuid/grade`, () => {
    return HttpResponse.json({ success: true });
  }),
];

const server = setupServer(...handlers);

describe("BanditoClient", () => {
  beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
  afterEach(() => server.resetHandlers());
  afterAll(() => server.close());

  it("connect + pull + update lifecycle", async () => {
    const client = new BanditoClient({
      apiKey: "test-key",
      baseUrl: BASE_URL,
      storePath: ":memory:",
    });

    await client.connect();

    const result = client.pull("test-bandit", { query: "hello world" });
    expect(result).toHaveProperty("arm");
    expect(result).toHaveProperty("eventId");
    expect(result.banditName).toBe("test-bandit");
    expect(result.arm.modelName).toBeTruthy();

    // update should not throw
    client.update(result, {
      response: "hi there",
      reward: 0.8,
    });

    await client.close();
  });

  it("pull returns scores for all active arms", async () => {
    const client = new BanditoClient({
      apiKey: "test-key",
      baseUrl: BASE_URL,
      storePath: ":memory:",
    });

    await client.connect();
    const result = client.pull("test-bandit");
    expect(Object.keys(result.scores)).toHaveLength(3);
    await client.close();
  });

  it("pull with exclude masks specified arms", async () => {
    const client = new BanditoClient({
      apiKey: "test-key",
      baseUrl: BASE_URL,
      storePath: ":memory:",
    });

    await client.connect();
    const result = client.pull("test-bandit", { exclude: [1, 2] });
    expect(result.arm.armId).toBe(3);
    await client.close();
  });

  it("pull throws for unknown bandit", async () => {
    const client = new BanditoClient({
      apiKey: "test-key",
      baseUrl: BASE_URL,
      storePath: ":memory:",
    });

    await client.connect();
    expect(() => client.pull("nonexistent")).toThrow("Unknown bandit");
    await client.close();
  });

  it("pull throws when not connected", () => {
    const client = new BanditoClient({ apiKey: "test-key" });
    expect(() => client.pull("test-bandit")).toThrow("Not connected");
  });

  it("grade sends human grade", async () => {
    let gradedUuid: string | undefined;
    let gradedValue: number | undefined;

    server.use(
      http.patch(`${BASE_URL}/api/v1/events/:uuid/grade`, async ({ params, request }) => {
        gradedUuid = params.uuid as string;
        const body = (await request.json()) as { grade: number };
        gradedValue = body.grade;
        return HttpResponse.json({ success: true });
      }),
    );

    const client = new BanditoClient({
      apiKey: "test-key",
      baseUrl: BASE_URL,
      storePath: ":memory:",
    });

    await client.connect();
    await client.grade("my-event-id", 0.9);
    expect(gradedUuid).toBe("my-event-id");
    expect(gradedValue).toBe(0.9);
    await client.close();
  });

  it("sync refreshes state", async () => {
    const client = new BanditoClient({
      apiKey: "test-key",
      baseUrl: BASE_URL,
      storePath: ":memory:",
    });

    await client.connect();
    await client.sync();
    // Should still work after sync
    const result = client.pull("test-bandit");
    expect(result).toHaveProperty("arm");
    await client.close();
  });

  it("update with failed flag sets reward to 0", async () => {
    let receivedEvents: Record<string, unknown>[] = [];

    server.use(
      http.post(`${BASE_URL}/api/v1/events`, async ({ request }) => {
        const body = (await request.json()) as { events: Record<string, unknown>[] };
        receivedEvents = body.events;
        return HttpResponse.json({ accepted: body.events.length, errors: [] });
      }),
    );

    const client = new BanditoClient({
      apiKey: "test-key",
      baseUrl: BASE_URL,
      storePath: ":memory:",
    });

    await client.connect();
    const result = client.pull("test-bandit");
    client.update(result, { failed: true });

    // Wait for flush
    await new Promise((r) => setTimeout(r, 200));

    expect(receivedEvents.length).toBeGreaterThan(0);
    expect(receivedEvents[0].early_reward).toBe(0.0);
    await client.close();
  });

  it("pull throws when not connected (explicit client)", () => {
    const client = new BanditoClient({
      apiKey: "test-key",
      baseUrl: BASE_URL,
      storePath: ":memory:",
    });
    expect(() => client.pull("any-bandit")).toThrow("Not connected");
  });
});
