#!/bin/bash
# Check screenpipe database for most recent frames

DB_PATH="${SCREENPIPE_DB:-$HOME/.screenpipe/db.sqlite}"

echo "=== Screenpipe Database Check ==="
echo "Database: $DB_PATH"
echo ""

# Check if database exists
if [ ! -f "$DB_PATH" ]; then
    echo "❌ Database not found at $DB_PATH"
    exit 1
fi

echo "✅ Database found"
echo ""

# Get database size
DB_SIZE=$(du -h "$DB_PATH" | cut -f1)
echo "Database size: $DB_SIZE"
echo ""

# Count total frames
TOTAL_FRAMES=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM frames;")
echo "Total frames: $TOTAL_FRAMES"
echo ""

# Get most recent 10 frames
echo "=== Most Recent 10 Frames ==="
sqlite3 -header -column "$DB_PATH" <<EOF
SELECT 
    id,
    timestamp,
    app_name,
    SUBSTR(window_name, 1, 30) as window,
    CASE WHEN focused = 1 THEN '✓' ELSE ' ' END as foc
FROM frames 
ORDER BY timestamp DESC 
LIMIT 10;
EOF

echo ""

# Get timestamp stats
echo "=== Timestamp Statistics ==="
sqlite3 "$DB_PATH" <<EOF
SELECT 
    'Oldest frame: ' || MIN(timestamp),
    'Newest frame: ' || MAX(timestamp),
    'Time span: ' || 
        CAST((JULIANDAY(MAX(timestamp)) - JULIANDAY(MIN(timestamp))) AS INTEGER) || ' days'
FROM frames;
EOF

echo ""

# Check if screenpipe is running
echo "=== Screenpipe Process Status ==="
SCREENPIPE_PID=$(pgrep -x "screenpipe" 2>/dev/null || ps aux | grep -E "[s]creenpipe-v" | grep -v grep | awk '{print $2}' | head -1)
if [ -n "$SCREENPIPE_PID" ]; then
    echo "✅ screenpipe is running (PID: $SCREENPIPE_PID)"
    ps -p "$SCREENPIPE_PID" -o pid,etime,command | tail -1
else
    echo "❌ screenpipe is NOT running"
fi

echo ""

# Get time since last frame
echo "=== Time Since Last Frame ==="
LAST_TIMESTAMP=$(sqlite3 "$DB_PATH" "SELECT MAX(timestamp) FROM frames;")
echo "Last frame recorded: $LAST_TIMESTAMP"
echo "Current time: $(date -u +"%Y-%m-%dT%H:%M:%S")"

