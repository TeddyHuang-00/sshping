#!/bin/bash
# Test: Wrong password should fail gracefully with auth error
set -euo pipefail

source /tmp/sshping-ci/sshd-env.sh 2>/dev/null || bash "$(dirname "$0")/../setup-sshd.sh"
source /tmp/sshping-ci/sshd-env.sh

echo "--- Test: Wrong password (should fail) ---"

set +e
OUTPUT=$(run $BINARY --agent false -p wrong_password -T 10 "localhost:$SSH_PORT" 2>&1)
EXIT_CODE=$?
set -e

echo "Exit code: $EXIT_CODE"
echo "$OUTPUT"

if [ "$EXIT_CODE" -eq 0 ]; then
	echo ">>> FAIL: Expected non-zero exit, got 0"
	exit 1
fi

if echo "$OUTPUT" | grep -qiE "(authentication failed|all methods failed|permission denied|error)"; then
	echo ">>> PASS: Wrong password correctly rejected (exit $EXIT_CODE, auth error present)"
else
	echo ">>> FAIL: No authentication failure message found in output"
	exit 1
fi
