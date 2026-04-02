#!/usr/bin/env bash
set -euo pipefail

LCOV_FILE="${1:-lcov.info}"
BASELINE_FILE="${2:-.metrics/coverage-baseline}"

if [ ! -f "$LCOV_FILE" ]; then
    echo "ERROR: LCOV file not found: $LCOV_FILE"
    exit 1
fi

if [ ! -f "$BASELINE_FILE" ]; then
    echo "ERROR: Coverage baseline not found: $BASELINE_FILE"
    echo "Add a line like 'line=85.1234' to the baseline file."
    exit 1
fi

BASELINE=$(awk -F= '/^line=/{print $2}' "$BASELINE_FILE" | tr -d '[:space:]')
if [ -z "$BASELINE" ]; then
    echo "ERROR: Could not read 'line=' coverage baseline from $BASELINE_FILE"
    exit 1
fi

if ! echo "$BASELINE" | grep -qE '^[0-9]+([.][0-9]+)?$'; then
    echo "ERROR: Invalid baseline value '$BASELINE' in $BASELINE_FILE"
    exit 1
fi

ACTUAL=$(awk -F: '
    /^LF:/ { lf += $2 }
    /^LH:/ { lh += $2 }
    END {
        if (lf == 0) {
            printf "0.0000"
            exit 0
        }

        printf "%.4f", (lh / lf) * 100
    }
' "$LCOV_FILE")

if awk -v actual="$ACTUAL" -v baseline="$BASELINE" 'BEGIN { exit !(actual >= baseline) }'; then
    echo "Coverage OK: line coverage ${ACTUAL}% (baseline ${BASELINE}%)"
    exit 0
fi

echo "ERROR: Coverage regression detected."
echo "  line coverage: ${ACTUAL}%"
echo "  baseline:      ${BASELINE}%"
exit 1
