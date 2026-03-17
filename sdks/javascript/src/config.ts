/**
 * Config loader — reads ~/.bandito/config.toml and env vars.
 *
 * Priority order (highest wins): constructor args → env vars → TOML → defaults.
 * Same file as the Python SDK uses.
 */

import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import { parse as parseToml } from "smol-toml";

export const DEFAULT_BASE_URL = "https://bandito-api.onrender.com";
const CONFIG_DIR = path.join(os.homedir(), ".bandito");
const CONFIG_FILE = path.join(CONFIG_DIR, "config.toml");

export interface S3Config {
  bucket: string;
  prefix: string;
  region: string;
  /** Optional custom endpoint for MinIO / LocalStack / other S3-compatible stores. */
  endpoint?: string;
}

export interface JudgeConfig {
  apiKey: string | null;
  model: string;
}

export interface BanditoConfig {
  apiKey: string | null;
  baseUrl: string;
  dataStorage: string; // "local", "cloud", or "s3"
  s3: S3Config | null;
  judge: JudgeConfig;
}

/**
 * Load config from TOML file + env var overrides.
 */
export function loadConfig(): BanditoConfig {
  const config: BanditoConfig = {
    apiKey: null,
    baseUrl: DEFAULT_BASE_URL,
    dataStorage: "local",
    s3: null,
    judge: { apiKey: null, model: "gpt-4o-mini" },
  };

  // TOML file first
  if (fs.existsSync(CONFIG_FILE)) {
    try {
      const content = fs.readFileSync(CONFIG_FILE, "utf-8");
      const data = parseToml(content) as Record<string, unknown>;
      if (data.api_key) config.apiKey = data.api_key as string;
      if (data.base_url) config.baseUrl = data.base_url as string;
      if (data.data_storage) config.dataStorage = data.data_storage as string;
      const s3Data = data.s3 as Record<string, unknown> | undefined;
      const s3Bucket = ((s3Data?.bucket as string | undefined) ?? "").trim();
      if (s3Bucket) {
        const s3Endpoint = ((s3Data?.endpoint as string | undefined) ?? "").trim() || undefined;
        config.s3 = {
          bucket: s3Bucket,
          prefix: ((s3Data?.prefix as string | undefined) ?? "").trim() || "bandito",
          region: ((s3Data?.region as string | undefined) ?? "").trim() || "us-east-1",
          ...(s3Endpoint ? { endpoint: s3Endpoint } : {}),
        };
      }
      const judgeData = data.judge as Record<string, unknown> | undefined;
      if (judgeData?.api_key) config.judge.apiKey = judgeData.api_key as string;
      if (judgeData?.model) config.judge.model = judgeData.model as string;
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

  // S3 env vars (useful for container deployments without a config file)
  const envS3Bucket = (process.env.BANDITO_S3_BUCKET ?? "").trim();
  if (envS3Bucket) {
    const envEndpoint = (process.env.BANDITO_S3_ENDPOINT ?? config.s3?.endpoint ?? "").trim() || undefined;
    config.s3 = {
      bucket: envS3Bucket,
      prefix: (process.env.BANDITO_S3_PREFIX ?? config.s3?.prefix ?? "").trim() || "bandito",
      region: (process.env.BANDITO_S3_REGION ?? config.s3?.region ?? "").trim() || "us-east-1",
      ...(envEndpoint ? { endpoint: envEndpoint } : {}),
    };
    // Setting BANDITO_S3_BUCKET implicitly activates s3 mode unless
    // the user has explicitly chosen a different mode via BANDITO_DATA_STORAGE.
    if (!process.env.BANDITO_DATA_STORAGE) {
      config.dataStorage = "s3";
    }
  } else if (config.s3) {
    const envPrefix = (process.env.BANDITO_S3_PREFIX ?? "").trim();
    const envRegion = (process.env.BANDITO_S3_REGION ?? "").trim();
    const envEndpoint = (process.env.BANDITO_S3_ENDPOINT ?? "").trim();
    if (envPrefix) config.s3.prefix = envPrefix;
    if (envRegion) config.s3.region = envRegion;
    if (envEndpoint) config.s3.endpoint = envEndpoint;
  }

  const envJudgeKey = process.env.JUDGE_API_KEY;
  if (envJudgeKey) config.judge.apiKey = envJudgeKey;

  return config;
}
