#!/bin/bash
# Test: Default key discovery (sshping finds ~/.ssh/id_ed25519 automatically)
# Verifies that sshping uses the discovered default key (not fallback)
set -euo pipefail

source /tmp/sshping-ci/sshd-env.sh 2>/dev/null || bash "$(dirname "$0")/../setup-sshd.sh"
source /tmp/sshping-ci/sshd-env.sh

echo "--- Test: Default key discovery ---"

# Copy test key to ~/.ssh/id_ed25519 for default key discovery
mkdir -p "$HOME/.ssh"
cp "$SSH_KEY_PATH" "$HOME/.ssh/id_ed25519"
cp "$SSH_PUB_KEY_PATH" "$HOME/.ssh/id_ed25519.pub"
chmod 600 "$HOME/.ssh/id_ed25519"

OUTPUT=$(run $BINARY -vv --agent false -T 30 -c 50 -t 30 -r echo "localhost:$SSH_PORT" 2>&1)
echo "$OUTPUT"

# Clean up the default key we placed
rm -f "$HOME/.ssh/id_ed25519" "$HOME/.ssh/id_ed25519.pub"

if echo "$OUTPUT" | grep -q "Public key authentication succeeded"; then
	echo ">>> PASS: Default key discovery used public key authentication"
else
	echo ">>> FAIL: Public key authentication not confirmed"
	exit 1
fi
