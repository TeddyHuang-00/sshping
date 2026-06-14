#!/bin/bash
# Test: Echo test only (character echo latency measurement)
set -euo pipefail

source /tmp/sshping-ci/sshd-env.sh 2>/dev/null || bash "$(dirname "$0")/../setup-sshd.sh"
source /tmp/sshping-ci/sshd-env.sh

echo "--- Test: Echo test only ---"
run $BINARY --agent false -i "$SSH_KEY_PATH" -T 30 -c 100 -t 60 -r echo "localhost:$SSH_PORT"
echo ">>> PASS: Echo test completed successfully"
