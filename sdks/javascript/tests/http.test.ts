import { describe, it, expect, beforeAll, afterAll, afterEach } from "vitest";
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";
import { BanditoHTTP } from "../src/http.js";

const BASE_URL = "http://localhost:9999";

const handlers = [
  http.post(`${BASE_URL}/api/v1/sync/connect`, () => {
    return HttpResponse.json({ bandits: [] });
  }),
  http.post(`${BASE_URL}/api/v1/sync/heartbeat`, () => {
    return HttpResponse.json({ bandits: [] });
  }),
  http.post(`${BASE_URL}/api/v1/events`, async ({ request }) => {
    const body = (await request.json()) as Record<string, unknown>;
    return HttpResponse.json({
      accepted: (body.events as unknown[])?.length ?? 0,
      errors: [],
    });
  }),
  http.patch(`${BASE_URL}/api/v1/events/:uuid/grade`, () => {
    return HttpResponse.json({ success: true });
  }),
];

const server = setupServer(...handlers);

describe("BanditoHTTP", () => {
  beforeAll(() => server.listen({ onUnhandledRequest: "error" }));
  afterEach(() => server.resetHandlers());
  afterAll(() => server.close());

  it("connect returns bandits", async () => {
    const client = new BanditoHTTP(BASE_URL, "test-key");
    const result = await client.connect();
    expect(result).toHaveProperty("bandits");
  });

  it("heartbeat returns bandits", async () => {
    const client = new BanditoHTTP(BASE_URL, "test-key");
    const result = await client.heartbeat();
    expect(result).toHaveProperty("bandits");
  });

  it("ingestEvents sends batch", async () => {
    const client = new BanditoHTTP(BASE_URL, "test-key");
    const result = await client.ingestEvents([
      { local_event_uuid: "u1", bandit_id: 1, arm_id: 1 },
    ]);
    expect(result.accepted).toBe(1);
  });

  it("submitGrade sends grade", async () => {
    const client = new BanditoHTTP(BASE_URL, "test-key");
    const result = await client.submitGrade("test-uuid", 0.9);
    expect(result.success).toBe(true);
  });

  it("throws on 4xx errors without retrying", async () => {
    server.use(
      http.post(`${BASE_URL}/api/v1/sync/connect`, () => {
        return HttpResponse.json({ detail: "Unauthorized" }, { status: 401 });
      }),
    );

    const client = new BanditoHTTP(BASE_URL, "bad-key");
    await expect(client.connect()).rejects.toThrow("HTTP 401");
  });

  it("sends X-API-Key header", async () => {
    let receivedKey: string | null = null;
    server.use(
      http.post(`${BASE_URL}/api/v1/sync/connect`, ({ request }) => {
        receivedKey = request.headers.get("X-API-Key");
        return HttpResponse.json({ bandits: [] });
      }),
    );

    const client = new BanditoHTTP(BASE_URL, "my-secret-key");
    await client.connect();
    expect(receivedKey).toBe("my-secret-key");
  });
});
