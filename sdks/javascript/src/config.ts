/**
 * Config loader — reads ~/.bandito/config.toml and env vars.
 *
 * Resolution order: constructor args → env vars → TOML → defaults.
 * Same file as the Python SDK uses.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { parse as parseToml } from "smol-toml";

export const DEFAULT_BASE_URL = "https://bandito-api.onrender.com";
const CONFIG_DIR = path.join(os.homedir(), ".bandito");
const CONFIG_FILE = path.join(CONFIG_DIR, "config.toml");

export interface BanditoConfig {
  apiKey: string | null;
  baseUrl: string;
  dataStorage: string; // "local" or "cloud"
}

/**
 * Load config from TOML file + env var overrides.
 */
export function loadConfig(): BanditoConfig {
  const config: BanditoConfig = {
    apiKey: null,
    baseUrl: DEFAULT_BASE_URL,
    dataStorage: "local",
  };

  // TOML file first
  if (fs.existsSync(CONFIG_FILE)) {
    try {
      const content = fs.readFileSync(CONFIG_FILE, "utf-8");
      const data = parseToml(content);
      if (data.api_key) config.apiKey = data.api_key;
      if (data.base_url) config.baseUrl = data.base_url;
      if (data.data_storage) config.dataStorage = data.data_storage;
    } catch {
      // Failed to parse TOML — fall back to env vars
    }
  }

  // Env vars override
  const envKey = process.env.BANDITO_API_KEY;
  if (envKey) config.apiKey = envKey;

  const envUrl = process.env.BANDITO_BASE_URL;
  if (envUrl) config.baseUrl = envUrl;

  const envStorage = process.env.BANDITO_DATA_STORAGE;
  if (envStorage) config.dataStorage = envStorage;

  return config;
}
