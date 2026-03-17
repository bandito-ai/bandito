/**
 * Thin wrapper around the WASM engine import.
 *
 * Handles async WASM initialization (loading .wasm binary happens once)
 * and re-exports the BanditEngine constructor for the client.
 */

import type { BanditEngine as WasmBanditEngine } from "../wasm/bandito_engine";

let wasmModule: typeof import("../wasm/bandito_engine") | null = null;

/**
 * Initialize the WASM module. Must be called before creating BanditEngine instances.
 * Safe to call multiple times — only loads once.
 */
export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  wasmModule = await import("../wasm/bandito_engine");
}

/**
 * Create a BanditEngine from a sync response JSON string.
 * Seeds the RNG with OS entropy to prevent predictable arm selection.
 * Requires initWasm() to have been called first.
 */
export function createEngine(banditJson: string): WasmBanditEngine {
  if (!wasmModule) {
    throw new Error("WASM not initialized — call initWasm() first");
  }
  // Generate 8 bytes of OS entropy and pack into a u64 BigInt seed
  const seedBytes = new Uint8Array(8);
  globalThis.crypto.getRandomValues(seedBytes);
  let seed = 0n;
  for (let i = 0; i < 8; i++) {
    seed |= BigInt(seedBytes[i]) << BigInt(i * 8);
  }
  return wasmModule.BanditEngine.newWithSeed(banditJson, seed);
}


/**
 * Update an existing BanditEngine with new sync response data.
 * Preserves RNG state (avoids the "always picks same arm" bug).
 */
export function updateEngine(engine: WasmBanditEngine, banditJson: string): void {
  engine.updateFromSync(banditJson);
}

export type { WasmBanditEngine as BanditEngine };

/**
 * Pull result parsed from engine JSON output.
 */
export interface EnginePullResult {
  arm_index: number;
  arm_id: number;
  scores: Record<number, number>;
}
