#!/bin/bash
# Test: SSH agent authentication
# Verifies that sshping authenticates via ssh-agent (not file key fallback)
set -euo pipefail

source /tmp/sshping-ci/sshd-env.sh 2>/dev/null || bash "$(dirname "$0")/../setup-sshd.sh"
source /tmp/sshping-ci/sshd-env.sh

echo "--- Test: SSH agent authentication ---"

# Start a fresh agent and add the key
eval "$(ssh-agent -s)" >/dev/null
ssh-add "$SSH_KEY_PATH" 2>&1

OUTPUT=$(run $BINARY -vvv -T 30 -c 50 -t 60 "localhost:$SSH_PORT" 2>&1)
echo "$OUTPUT"

# Kill the agent we started
ssh-agent -k >/dev/null 2>&1 || true

if echo "$OUTPUT" | grep -q "SSH agent authentication succeeded"; then
	echo ">>> PASS: Agent authentication was used"
else
	echo ">>> FAIL: Agent not used (may have fallen back to file keys)"
	exit 1
fi
