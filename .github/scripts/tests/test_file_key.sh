#!/bin/bash
# Test: File-based key authentication (no agent)
# Verifies that sshping authenticates via file key (not fallback)
set -euo pipefail

source /tmp/sshping-ci/sshd-env.sh 2>/dev/null || bash "$(dirname "$0")/../setup-sshd.sh"
source /tmp/sshping-ci/sshd-env.sh

echo "--- Test: File-based key authentication ---"

OUTPUT=$(run $BINARY -vv --agent false -i "$SSH_KEY_PATH" -T 30 -c 50 -t 60 "localhost:$SSH_PORT" 2>&1)
echo "$OUTPUT"

if echo "$OUTPUT" | grep -q "Public key authentication succeeded"; then
	echo ">>> PASS: File-based key authentication was used"
else
	echo ">>> FAIL: Public key authentication not confirmed"
	exit 1
fi
