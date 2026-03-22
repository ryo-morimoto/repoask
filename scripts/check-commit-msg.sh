#!/usr/bin/env bash
set -euo pipefail

MSG_FILE="$1"
MSG=$(head -1 "$MSG_FILE")

# Conventional Commits: type(scope)?: description
# Allowed types: feat, fix, chore, docs, refactor, test, perf, ci, build, style, revert
PATTERN='^(feat|fix|chore|docs|refactor|test|perf|ci|build|style|revert)(\([a-z0-9-]+\))?!?: .+'
if ! echo "$MSG" | grep -qE "$PATTERN"; then
    echo "ERROR: Commit message does not follow Conventional Commits."
    echo ""
    echo "  Expected: <type>[optional scope]: <description>"
    echo "  Types:    feat, fix, chore, docs, refactor, test, perf, ci, build, style, revert"
    echo ""
    echo "  Got: $MSG"
    exit 1
fi

# ASCII-only check (no CJK, no emoji in title line)
if echo "$MSG" | grep -qP '[^\x00-\x7F]'; then
    echo "ERROR: Commit message title must be ASCII-only (English)."
    echo ""
    echo "  Got: $MSG"
    exit 1
fi

# Length check: title <= 72 chars
if [ ${#MSG} -gt 72 ]; then
    echo "ERROR: Commit message title exceeds 72 characters (got ${#MSG})."
    echo ""
    echo "  Got: $MSG"
    exit 1
fi
