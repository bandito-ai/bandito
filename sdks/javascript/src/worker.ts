/**
 * Payload utilities for cloud event ingestion.
 */

const TEXT_FIELDS = ["query_text", "response"] as const;
const METADATA_FIELDS = ["model_name", "model_provider"] as const;

/**
 * Return shallow copies of events ready for cloud ingest.
 *
 * Always strips model_name/model_provider (only needed in local SQLite for TUI).
 * Strips query_text/response when includeText is false (dataStorage="local").
 */
export function prepareCloudPayload(
  events: Record<string, unknown>[],
  includeText: boolean,
): Record<string, unknown>[] {
  return events.map((event) => {
    const copy = { ...event };
    for (const field of METADATA_FIELDS) {
      delete copy[field];
    }
    if (!includeText) {
      for (const field of TEXT_FIELDS) {
        delete copy[field];
      }
    }
    return copy;
  });
}
