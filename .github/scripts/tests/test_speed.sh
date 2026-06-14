#!/bin/bash
# Test: Speed test only (file transfer throughput, small file)
set -euo pipefail

source /tmp/sshping-ci/sshd-env.sh 2>/dev/null || bash "$(dirname "$0")/../setup-sshd.sh"
source /tmp/sshping-ci/sshd-env.sh

echo "--- Test: Speed test only (small file) ---"
run $BINARY --agent false -i "$SSH_KEY_PATH" -T 30 -s 100KB -u 32KB -z /tmp/sshping-ci-speed.tmp -r speed "localhost:$SSH_PORT"

# Clean up temp file
rm -f /tmp/sshping-ci-speed.tmp
echo ">>> PASS: Speed test completed successfully"
