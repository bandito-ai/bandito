/**
 * HTTP transport — fetch-based client for cloud API.
 *
 * Retry config: 3 attempts, exponential backoff (0.5s, 1s, 2s),
 * retries only 5xx and network errors. Never retries 4xx.
 */

const MAX_RETRIES = 3;
const RETRY_BACKOFF_BASE = 500; // ms — 500, 1000, 2000

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function isRetryable(status: number): boolean {
  return status >= 500;
}

export class BanditoHTTP {
  private baseUrl: string;
  private apiKey: string;
  private timeout: number;

  constructor(baseUrl: string, apiKey: string, timeout: number = 10_000) {
    // Validate URL scheme — reject non-http(s) protocols (file://, javascript:, etc.)
    let parsed: URL;
    try {
      parsed = new URL(baseUrl);
    } catch {
      throw new Error(`Invalid baseUrl: "${baseUrl}"`);
    }
    if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
      throw new Error(
        `baseUrl must use http or https, got "${parsed.protocol}" — check your config`,
      );
    }
    this.baseUrl = `${baseUrl.replace(/\/$/, "")}/api/v1`;
    this.apiKey = apiKey;
    this.timeout = timeout;
  }

  private async request(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<Record<string, unknown>> {
    let lastError: Error | null = null;

    for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), this.timeout);

      try {
        const resp = await fetch(`${this.baseUrl}${path}`, {
          method,
          headers: {
            "X-API-Key": this.apiKey,
            "Content-Type": "application/json",
          },
          body: body != null ? JSON.stringify(body) : undefined,
          signal: controller.signal,
        });

        clearTimeout(timer);

        if (!resp.ok) {
          const text = await resp.text().catch(() => "");
          // Truncate body to avoid leaking sensitive content into error messages/logs
          const preview = text.length > 200 ? `${text.slice(0, 200)}…` : text;
          if (!isRetryable(resp.status) || attempt === MAX_RETRIES - 1) {
            throw new Error(
              `HTTP ${resp.status} on ${method} ${path}: ${preview}`,
            );
          }
          // Retryable server error — fall through to retry
          lastError = new Error(
            `HTTP ${resp.status} on ${method} ${path}: ${preview}`,
          );
        } else {
          return (await resp.json()) as Record<string, unknown>;
        }
      } catch (err) {
        clearTimeout(timer);
        lastError = err as Error;

        // AbortError = timeout, TypeError = network error (in fetch)
        const isNetworkOrTimeout =
          (err as Error).name === "AbortError" ||
          (err as Error).name === "TypeError";
        if (!isNetworkOrTimeout && attempt < MAX_RETRIES - 1) {
          // Non-retryable (4xx already handled above)
          throw err;
        }
        if (attempt === MAX_RETRIES - 1) {
          throw err;
        }
      }

      // Exponential backoff
      const delay = RETRY_BACKOFF_BASE * 2 ** attempt;
      await sleep(delay);
    }

    throw lastError!;
  }

  /** POST /sync/connect — SDK bootstrap. */
  async connect(): Promise<Record<string, unknown>> {
    return this.request("POST", "/sync/connect");
  }

  /** POST /sync/heartbeat — periodic state refresh. */
  async heartbeat(): Promise<Record<string, unknown>> {
    return this.request("POST", "/sync/heartbeat", {});
  }

  /** POST /events — batch event ingestion. */
  async ingestEvents(
    events: Record<string, unknown>[],
  ): Promise<Record<string, unknown>> {
    return this.request("POST", "/events", { events });
  }

  /** PATCH /events/{uuid}/grade — submit human grade. */
  async submitGrade(
    eventUuid: string,
    grade: number,
  ): Promise<Record<string, unknown>> {
    return this.request("PATCH", `/events/${eventUuid}/grade`, {
      grade,
      is_graded: true,
    });
  }
}
