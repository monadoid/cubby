DROP TABLE IF EXISTS live_summaries;
DROP TABLE IF EXISTS live_summary_state;

CREATE TABLE live_summaries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  frame_id INTEGER NOT NULL UNIQUE REFERENCES frames(id) ON DELETE CASCADE,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  event_label TEXT NOT NULL,
  event_detail TEXT NOT NULL,
  event_app TEXT,
  event_window TEXT,
  event_confidence REAL,
  event_time DATETIME NOT NULL,
  error TEXT,
  created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_live_summaries_created_at
  ON live_summaries(created_at);
