/**
 * BanditoClient — main orchestrator for the JS/TS SDK.
 *
 * Mirrors the Python SDK's sync-first design:
 * - pull() is synchronous (WASM math, <1ms)
 * - connect(), grade(), sync(), close() are async (HTTP I/O)
 * - update() is synchronous (SQLite write + fire-and-forget flush)
 */

import { randomUUID } from "node:crypto";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { performance } from "node:perf_hooks";

import { initWasm, createEngine, type BanditEngine, type EnginePullResult } from "./engine.js";
import {
  type Arm,
  type PullResult,
  type BanditCache,
  type ArmWire,
  createArm,
  createPullResult,
} from "./models.js";
import { loadConfig, DEFAULT_BASE_URL } from "./config.js";
import { BanditoHTTP } from "./http.js";
import { EventStore, type EventPayload } from "./store.js";
import { prepareCloudPayload } from "./worker.js";

const DEFAULT_STORE_PATH = path.join(os.homedir(), ".bandito", "events.db");
const MAX_EVENT_RETRIES = 5;

export interface ClientOptions {
  apiKey?: string;
  baseUrl?: string;
  storePath?: string;
  dataStorage?: string;
}

export interface PullOptions {
  query?: string;
  exclude?: number[];
}

export interface UpdateOptions {
  queryText?: string;
  response?: string | Record<string, unknown>;
  reward?: number;
  cost?: number;
  latency?: number;
  inputTokens?: number;
  outputTokens?: number;
  segment?: Record<string, string>;
  failed?: boolean;
}

export class BanditoClient {
  private apiKey: string | undefined;
  private baseUrl: string | undefined;
  private storePath: string | undefined;
  private dataStorageArg: string | undefined;
  private dataStorage: string;

  private http: BanditoHTTP | null = null;
  private store: EventStore | null = null;
  private engines: Map<string, BanditEngine> = new Map();
  private bandits: Map<string, BanditCache> = new Map();
  private connected = false;
  private flushInterval: ReturnType<typeof setInterval> | null = null;
  private flushInProgress = false;
  private deadUuids: Set<string> = new Set();
  private retryCounts: Map<string, number> = new Map();

  constructor(options: ClientOptions = {}) {
    this.apiKey = options.apiKey;
    this.baseUrl = options.baseUrl;
    this.storePath = options.storePath;
    this.dataStorageArg = options.dataStorage;
    this.dataStorage = options.dataStorage ?? "local";
  }

  /**
   * Bootstrap: authenticate and hydrate in-memory state from cloud.
   *
   * Resolves config from: constructor args → env vars → ~/.bandito/config.toml.
   * Initializes WASM, creates HTTP client, SQLite store, fetches full state.
   */
  async connect(): Promise<void> {
    // Tear down previous connection if reconnecting
    if (this.connected) {
      await this.close();
    }

    // Init WASM (loads .wasm binary once)
    await initWasm();

    // Resolve config
    const config = loadConfig();
    const apiKey = this.apiKey ?? config.apiKey;
    if (!apiKey) {
      throw new Error(
        "apiKey required — pass it to constructor, set BANDITO_API_KEY, " +
          "or run `bandito signup`",
      );
    }

    const baseUrl = this.baseUrl ?? config.baseUrl;
    if (!this.dataStorageArg) {
      this.dataStorage = config.dataStorage;
    }

    this.http = new BanditoHTTP(baseUrl, apiKey);
    const storePath = this.storePath ?? DEFAULT_STORE_PATH;
    if (storePath !== ":memory:") {
      const dir = path.dirname(storePath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }
    }
    this.store = new EventStore(storePath);

    // Bootstrap: fetch state, hydrate cache, flush pending
    try {
      const data = await this.http.connect();
      this.applySync(data);

      // Reset retry state
      this.deadUuids.clear();
      this.retryCounts.clear();

      // Flush pending events from previous crash
      await this.flushPending();

      // Start periodic flush (every 30s)
      this.flushInterval = setInterval(() => {
        this.flushPending().catch(() => {});
      }, 30_000);

      this.connected = true;
    } catch (err) {
      this.store?.close();
      this.store = null;
      this.http = null;
      throw err;
    }
  }

  /**
   * Local Thompson Sampling decision. Synchronous, <1ms, no network.
   */
  pull(banditName: string, options: PullOptions = {}): PullResult {
    this.ensureConnected();

    const cache = this.bandits.get(banditName);
    if (!cache) {
      const available = [...this.bandits.keys()];
      throw new Error(
        `Unknown bandit '${banditName}'. Available: [${available.join(", ")}]`,
      );
    }

    if (cache.arms.length === 0) {
      throw new Error(`Bandit '${banditName}' has no active arms`);
    }

    const engine = this.engines.get(banditName)!;
    const queryLength = options.query?.length ?? undefined;
    const excludeIds = options.exclude?.map((id) => id) ?? undefined;

    const resultJson = engine.pull(queryLength, excludeIds);
    const raw: EnginePullResult = JSON.parse(resultJson);

    // Look up the winning arm from our cached active arms
    const winnerArm = cache.arms.find((a) => a.armId === raw.arm_id);
    if (!winnerArm) {
      throw new Error(
        `Engine selected arm ${raw.arm_id} but it's not in active arm cache for "${banditName}". ` +
          "This is likely a bug — please report it at https://github.com/bandito-ai/bandito/issues",
      );
    }

    return createPullResult({
      arm: winnerArm,
      eventId: randomUUID(),
      banditId: cache.banditId,
      banditName,
      scores: raw.scores,
      pullTime: performance.now(),
    });
  }

  /**
   * Record an LLM call outcome. Writes to SQLite first (crash-safe),
   * then fires off a non-blocking flush to cloud.
   */
  update(pullResult: PullResult, options: UpdateOptions = {}): void {
    this.ensureConnected();

    let reward = options.reward;
    if (options.failed && reward == null) {
      reward = 0.0;
    }

    // Auto-calculate latency from pull timestamp
    let latency = options.latency;
    if (latency == null && pullResult._pullTime > 0) {
      latency = performance.now() - pullResult._pullTime;
    }

    // Build event payload (snake_case for wire format)
    const event: EventPayload = {
      local_event_uuid: pullResult.eventId,
      bandit_id: pullResult.banditId,
      arm_id: pullResult.arm.armId,
      model_name: pullResult.arm.modelName,
      model_provider: pullResult.arm.modelProvider,
    };

    if (options.queryText != null) {
      (event as Record<string, unknown>).query_text = options.queryText;
    }
    if (options.response != null) {
      (event as Record<string, unknown>).response =
        typeof options.response === "string"
          ? { response: options.response }
          : options.response;
    }
    if (reward != null) {
      (event as Record<string, unknown>).early_reward = reward;
    }
    if (options.cost != null) {
      (event as Record<string, unknown>).cost = options.cost;
    }
    if (latency != null) {
      (event as Record<string, unknown>).latency = latency;
    }
    if (options.inputTokens != null) {
      (event as Record<string, unknown>).input_tokens = options.inputTokens;
    }
    if (options.outputTokens != null) {
      (event as Record<string, unknown>).output_tokens = options.outputTokens;
    }
    if (options.segment != null) {
      (event as Record<string, unknown>).segment = options.segment;
    }
    if (options.failed) {
      (event as Record<string, unknown>).run_error = true;
    }

    // Write to SQLite WAL first — survives crashes
    this.store!.push(event);

    // Fire-and-forget flush (errors logged inside flushPending)
    this.flushPending().catch(() => {});
  }

  /**
   * Send a human grade for an existing event. Async (HTTP).
   */
  async grade(eventId: string, grade: number): Promise<void> {
    this.ensureConnected();
    await this.http!.submitGrade(eventId, grade);
  }

  /**
   * Explicit state refresh from cloud.
   */
  async sync(): Promise<void> {
    this.ensureConnected();
    const data = await this.http!.heartbeat();

    const prevBandits = new Map(this.bandits);
    const prevEngines = new Map(this.engines);
    try {
      this.applySync(data);
    } catch (err) {
      // Rollback on malformed response — keep last-known-good state
      this.bandits = prevBandits;
      this.engines = prevEngines;
      console.warn("[bandito] Sync response malformed — keeping last-known-good state", err);
    }
  }

  /**
   * Shut down: clear interval, flush remaining events, close connections.
   */
  async close(): Promise<void> {
    if (this.flushInterval) {
      clearInterval(this.flushInterval);
      this.flushInterval = null;
    }

    // Final flush
    if (this.store && this.http) {
      await this.flushPending();
    }

    this.store?.close();
    this.store = null;
    this.http = null;
    this.engines.clear();
    this.bandits.clear();
    this.connected = false;
  }

  // --- Internal ---

  private ensureConnected(): void {
    if (!this.connected) {
      throw new Error("Not connected — call connect() first");
    }
  }

  private applySync(data: Record<string, unknown>): void {
    const banditsData = (data.bandits ?? []) as Record<string, unknown>[];

    this.bandits.clear();
    this.engines.clear();

    for (const b of banditsData) {
      const arms = (b.arms ?? []) as ArmWire[];
      if (arms.length === 0) continue;

      const activeArms: Arm[] = arms
        .filter((a) => a.is_active)
        .map((a) => createArm(a));

      const cache: BanditCache = {
        banditId: Number(b.bandit_id),
        name: b.name as string,
        arms: activeArms,
        armWire: arms,
        optimizationMode: (b.optimization_mode as string) ?? "base",
        avgLatencyLastN: b.avg_latency_last_n as number | null,
        budget: b.budget as number | null,
        totalCost: b.total_cost as number | null,
      };

      this.bandits.set(cache.name, cache);

      // Create WASM engine from the bandit JSON
      const engine = createEngine(JSON.stringify(b));
      this.engines.set(cache.name, engine);
    }
  }

  private async flushPending(): Promise<void> {
    if (this.flushInProgress || !this.store || !this.http) return;
    this.flushInProgress = true;

    try {
      const pending = this.store.pending();
      if (pending.length === 0) return;

      // Filter out dead events
      const alive = this.deadUuids.size > 0
        ? pending.filter((e) => !this.deadUuids.has(e.local_event_uuid))
        : pending;
      if (alive.length === 0) return;

      // Prepare payload (strip metadata/text as configured)
      const payload = prepareCloudPayload(
        alive as Record<string, unknown>[],
        this.dataStorage !== "local",
      );

      const result = await this.http.ingestEvents(payload);

      // Parse per-event errors
      const errors = (result.errors ?? []) as {
        local_event_uuid?: string;
        reason?: string;
      }[];
      const erroredUuids = new Set(
        errors.map((e) => e.local_event_uuid).filter(Boolean) as string[],
      );

      // Update retry counts
      for (const uid of erroredUuids) {
        const count = (this.retryCounts.get(uid) ?? 0) + 1;
        this.retryCounts.set(uid, count);
        if (count >= MAX_EVENT_RETRIES) {
          this.deadUuids.add(uid);
        }
      }

      // Mark accepted events as flushed
      const flushedUuids = alive
        .map((e) => e.local_event_uuid)
        .filter((uid) => !erroredUuids.has(uid));
      if (flushedUuids.length > 0) {
        this.store.markFlushed(flushedUuids);
      }
    } catch (err) {
      // Flush failure is non-fatal — events stay pending for next attempt
      console.warn("[bandito] Event flush failed — will retry", err);
    } finally {
      this.flushInProgress = false;
    }
  }
}
