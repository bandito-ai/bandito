/**
 * Bandito SDK — contextual bandit optimization for LLM selection.
 *
 * Recommended (explicit client):
 *   import { BanditoClient } from 'bandito';
 *
 *   const client = new BanditoClient({ apiKey: 'bnd_...' });
 *   await client.connect();
 *   const result = client.pull('my-chatbot', { query: userMessage });
 *   // ... call LLM with result.model, result.prompt ...
 *   client.update(result, { response: response.text });
 *   await client.close();
 *
 * Module-level singleton (convenience):
 *   import { connect, pull, update, close } from 'bandito';
 *
 *   await connect({ apiKey: 'bnd_...' });
 *   const result = pull('my-chatbot', { query: userMessage });
 *   update(result, { response: response.text });
 *   await close();
 */

export { BanditoClient, type ClientOptions, type PullOptions, type UpdateOptions } from "./client.js";
export { type Arm, type PullResult } from "./models.js";

import { BanditoClient, type ClientOptions, type PullOptions, type UpdateOptions } from "./client.js";
import type { PullResult } from "./models.js";

let _client: BanditoClient | null = null;

function getClient(): BanditoClient {
  if (!_client) {
    throw new Error("Not connected — call connect() first");
  }
  return _client;
}

/** Connect to the Bandito cloud and hydrate local state. */
export async function connect(options: ClientOptions = {}): Promise<void> {
  if (_client) {
    await _client.close();
  }
  _client = new BanditoClient(options);
  await _client.connect();
}

/** Local Thompson Sampling decision. <1ms, no network. */
export function pull(banditName: string, options?: PullOptions): PullResult {
  return getClient().pull(banditName, options);
}

/** Record an LLM call outcome (writes to SQLite, fire-and-forget flush). */
export function update(pullResult: PullResult, options?: UpdateOptions): void {
  getClient().update(pullResult, options);
}

/** Send a human grade for an existing event. */
export async function grade(eventId: string, gradeValue: number): Promise<void> {
  await getClient().grade(eventId, gradeValue);
}

/** Explicit state refresh from cloud. */
export async function sync(): Promise<void> {
  await getClient().sync();
}

/** Shut down: flush events, close connections. */
export async function close(): Promise<void> {
  if (_client) {
    await _client.close();
    _client = null;
  }
}
