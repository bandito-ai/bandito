/**
 * SQLite WAL durability layer — crash-safe event storage.
 *
 * Events are written here immediately after update(). Background flush
 * sends them to cloud. If the process crashes, pending events survive
 * and are retried on next connect().
 */

import Database from "better-sqlite3";

const SCHEMA = `
CREATE TABLE IF NOT EXISTS events (
    local_event_uuid TEXT PRIMARY KEY,
    bandit_id        INTEGER NOT NULL,
    arm_id           INTEGER NOT NULL,
    payload          TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'pending',
    created_at       REAL NOT NULL,
    human_reward     REAL,
    graded_at        REAL,
    s3_exported      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_events_status ON events(status);
`;

const MIGRATION_GRADING = [
  "ALTER TABLE events ADD COLUMN human_reward REAL",
  "ALTER TABLE events ADD COLUMN graded_at REAL",
  "ALTER TABLE events ADD COLUMN s3_exported INTEGER NOT NULL DEFAULT 0",
];

export interface EventPayload {
  local_event_uuid: string;
  bandit_id: number;
  arm_id: number;
  model_name: string;
  model_provider: string;
  bandit_name?: string;
  [key: string]: unknown;
}

export class EventStore {
  private db: Database.Database;
  private pushStmt: Database.Statement;
  private pendingStmt: Database.Statement;

  constructor(dbPath: string = ":memory:") {
    this.db = new Database(dbPath);
    this.db.pragma("journal_mode = WAL");
    this.db.pragma("busy_timeout = 5000");
    this.db.pragma("synchronous = NORMAL");
    this.db.exec(SCHEMA);
    this.migrate();

    // Pre-compile frequently-used statements
    this.pushStmt = this.db.prepare(
      `INSERT OR IGNORE INTO events
       (local_event_uuid, bandit_id, arm_id, payload, status, created_at)
       VALUES (?, ?, ?, ?, 'pending', ?)`,
    );
    this.pendingStmt = this.db.prepare(
      `SELECT local_event_uuid, payload FROM events WHERE status = 'pending'
       ORDER BY created_at ASC LIMIT ?`,
    );
  }

  /** Insert a pending event. */
  push(event: EventPayload): void {
    this.pushStmt.run(
      event.local_event_uuid,
      event.bandit_id,
      event.arm_id,
      JSON.stringify(event),
      Date.now() / 1000,
    );
  }

  /** Return up to `limit` pending events (oldest first). */
  pending(limit: number = 50): EventPayload[] {
    const rows = this.pendingStmt.all(limit) as {
      local_event_uuid: string;
      payload: string;
    }[];
    const results: EventPayload[] = [];
    const corrupt: string[] = [];
    for (const row of rows) {
      try {
        results.push(JSON.parse(row.payload) as EventPayload);
      } catch {
        console.warn(
          `[bandito] Discarding corrupt event ${row.local_event_uuid} from store`,
        );
        corrupt.push(row.local_event_uuid);
      }
    }
    if (corrupt.length > 0) {
      this.markFlushed(corrupt);
    }
    return results;
  }

  /** Mark events as successfully flushed to cloud. */
  markFlushed(uuids: string[]): void {
    if (uuids.length === 0) return;
    const placeholders = uuids.map(() => "?").join(",");
    this.db
      .prepare(
        `UPDATE events SET status = 'flushed'
         WHERE local_event_uuid IN (${placeholders})`,
      )
      .run(...uuids);
  }

  /** Record a human grade locally. */
  markGraded(uuid: string, reward: number): void {
    this.db
      .prepare(
        `UPDATE events SET human_reward = ?, graded_at = ?
         WHERE local_event_uuid = ?`,
      )
      .run(reward, Date.now() / 1000, uuid);
  }

  /** Return un-exported events as {event, ts} for S3 dump. */
  pendingS3(limit: number = 100): Array<{ event: Record<string, unknown>; ts: number }> {
    const rows = this.db
      .prepare(
        `SELECT payload, created_at FROM events WHERE s3_exported = 0
         ORDER BY created_at ASC LIMIT ?`,
      )
      .all(limit) as { payload: string; created_at: number }[];
    return rows.map((r) => ({ event: JSON.parse(r.payload) as Record<string, unknown>, ts: r.created_at }));
  }

  /** Mark events as successfully exported to S3. */
  markS3Exported(uuids: string[]): void {
    if (uuids.length === 0) return;
    const placeholders = uuids.map(() => "?").join(",");
    this.db
      .prepare(`UPDATE events SET s3_exported = 1 WHERE local_event_uuid IN (${placeholders})`)
      .run(...uuids);
  }

  /** Close the database connection. */
  close(): void {
    this.db.close();
  }

  private migrate(): void {
    for (const stmt of MIGRATION_GRADING) {
      try {
        this.db.exec(stmt);
      } catch {
        // Column already exists
      }
    }
  }
}
