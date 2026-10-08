#!/usr/bin/env bash
# Usage: annotated.sh TITLE COMMAND [ARGS…]
#
# Runs COMMAND. When it fails, the end of its output also becomes an error annotation titled TITLE, so
# the reason shows on the run's page and through the checks API, without opening the log.
set -uo pipefail
title=$1
shift
log=$(mktemp)
"$@" 2>&1 | tee "$log"
status=${PIPESTATUS[0]}
if [ "$status" -ne 0 ]; then
  # Colour codes are dropped; workflow commands encode %, CR and LF in the message (docs.github.com,
  # "Workflow commands").
  message=$(tail -n 30 "$log" | sed -e 's/\x1b\[[0-9;]*m//g' -e 's/%/%25/g' -e 's/\r/%0D/g' | awk 'BEGIN { ORS = "%0A" } { print }')
  echo "::error title=${title}::${message}"
fi
rm -f "$log"
exit "$status"
