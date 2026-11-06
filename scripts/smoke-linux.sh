#!/usr/bin/env bash
set -euo pipefail
: "${XDG_DATA_HOME:?Set an isolated data directory for this test}"
mkdir -p "$XDG_DATA_HOME"
log_file="$XDG_DATA_HOME/startup.log"
src-tauri/target/release/reponiq > "$log_file" 2>&1 &
app_pid=$!
cleanup() { kill "$app_pid" 2>/dev/null || true; wait "$app_pid" 2>/dev/null || true; }
trap cleanup EXIT
for attempt in $(seq 1 45); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    cat "$log_file"
    echo 'Native process exited before window/database initialization.' >&2
    exit 1
  fi
  if test -f "$XDG_DATA_HOME/reponiq/reponiq.sqlite" && xwininfo -root -tree | grep -F 'Reponiq' >/dev/null; then
    echo 'Linux native window and local database initialized.'
    cat "$log_file"
    exit 0
  fi
  sleep 1
done
cat "$log_file"
find "$XDG_DATA_HOME" -maxdepth 3 -type f -printf '%P\n'
echo 'Timed out waiting for the native window and database.' >&2
exit 1
