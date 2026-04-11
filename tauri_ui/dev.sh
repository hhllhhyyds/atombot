#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UI_DIR="$SCRIPT_DIR/ui"
TAURI_DIR="$SCRIPT_DIR/src-tauri"

# Kill any existing process on port 8080
lsof -ti:8080 | xargs kill -9 2>/dev/null || true

# Start HTTP server in background
cd "$UI_DIR"
python3 -m http.server 8080 &
SERVER_PID=$!

echo "HTTP server started on http://localhost:8080 (PID: $SERVER_PID)"
echo "Starting Tauri dev..."

# Cleanup on exit
trap "kill $SERVER_PID 2>/dev/null || true" EXIT

cd "$TAURI_DIR"
exec cargo tauri dev
