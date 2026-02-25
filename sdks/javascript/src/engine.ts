/**
 * Thin wrapper around the WASM engine import.
 *
 * Handles async WASM initialization (loading .wasm binary happens once)
 * and re-exports the BanditEngine constructor for the client.
 */

import type { BanditEngine as WasmBanditEngine } from "../../engine/pkg/bandito_engine";

let wasmModule: typeof import("../../engine/pkg/bandito_engine") | null = null;

/**
 * Initialize the WASM module. Must be called before creating BanditEngine instances.
 * Safe to call multiple times — only loads once.
 */
export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  wasmModule = await import("../../engine/pkg/bandito_engine");
}

/**
 * Create a BanditEngine from a sync response JSON string.
 * Requires initWasm() to have been called first.
 */
export function createEngine(banditJson: string): WasmBanditEngine {
  if (!wasmModule) {
    throw new Error("WASM not initialized — call initWasm() first");
  }
  return new wasmModule.BanditEngine(banditJson);
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
