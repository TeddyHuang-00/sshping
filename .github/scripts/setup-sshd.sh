#!/bin/bash
# Shared setup script for SSH ping CI integration tests.
# Installs openssh-server, generates test key, creates sshd config,
# starts sshd in background, and writes env vars to /tmp/sshping-ci/sshd-env.sh
#
# Usage: bash .github/scripts/setup-sshd.sh
# Source the output: source /tmp/sshping-ci/sshd-env.sh 2>/dev/null
set -euo pipefail

SSH_PORT=2222
TEST_PREFIX="/tmp/sshping-ci"
ENV_FILE="$TEST_PREFIX/sshd-env.sh"

# Idempotency check: if env file already exists and sshd is running, skip setup
if [ -f "$ENV_FILE" ]; then
	# shellcheck source=/dev/null
	source "$ENV_FILE"
	if [ -n "${SSHD_PID:-}" ] && kill -0 "$SSHD_PID" 2>/dev/null; then
		echo "sshd already running (PID $SSHD_PID), skipping setup"
		exit 0
	fi
fi

# Clean any leftover state from previous runs
rm -rf "$TEST_PREFIX"

# Create directory structure
mkdir -p "$TEST_PREFIX"/{.ssh,sshd}

echo "--- Install openssh-server ---"
sudo apt-get update -qq
sudo apt-get install -y -qq openssh-server 2>&1 | tail -1

echo "--- Generate test SSH key pair ---"
ssh-keygen -t ed25519 -f "$TEST_PREFIX/.ssh/id_ed25519" -N "" -C "sshping-ci@test" 2>&1
chmod 600 "$TEST_PREFIX/.ssh/id_ed25519"

echo "--- Set up authorized_keys (standard location) ---"
mkdir -p -m 700 "$HOME/.ssh"
cat "$TEST_PREFIX/.ssh/id_ed25519.pub" >>"$HOME/.ssh/authorized_keys"
chmod 600 "$HOME/.ssh/authorized_keys"

echo "--- Generate sshd host keys ---"
sudo rm -f /etc/ssh/ssh_host_*
sudo ssh-keygen -A 2>&1
sudo ssh-keygen -t ed25519 -f "$TEST_PREFIX/sshd/ssh_host_ed25519_key" -N "" -C "" 2>&1
sudo chown "$USER:$USER" "$TEST_PREFIX/sshd/ssh_host_ed25519_key" \
	"$TEST_PREFIX/sshd/ssh_host_ed25519_key.pub"
chmod 600 "$TEST_PREFIX/sshd/ssh_host_ed25519_key"

echo "--- Create custom sshd_config ---"
cat >"$TEST_PREFIX/sshd/sshd_config" <<SSHDCFG
Port $SSH_PORT
HostKey $TEST_PREFIX/sshd/ssh_host_ed25519_key
UsePAM yes
PubkeyAuthentication yes
PasswordAuthentication no
ChallengeResponseAuthentication no
PermitEmptyPasswords no
StrictModes no
LogLevel VERBOSE
PrintMotd no
PrintLastLog no
PidFile $TEST_PREFIX/sshd/sshd.pid
AcceptEnv LANG LC_*
Subsystem sftp /usr/lib/openssh/sftp-server
SSHDCFG

echo "--- Fix home directory permissions ---"
chmod go-w "$HOME"

# Create privilege separation directory (missing on GitHub Actions runners)
sudo mkdir -p /run/sshd

echo "--- Start sshd ---"
sudo /usr/sbin/sshd -f "$TEST_PREFIX/sshd/sshd_config" -D -e 2>&1 &
SSHD_PID=$!
echo "sshd PID: $SSHD_PID"

# Wait for sshd to be ready (use port check only — $! tracks sudo, not sshd)
for i in $(seq 1 10); do
	if nc -z 127.0.0.1 $SSH_PORT 2>/dev/null; then
		echo "sshd ready on port $SSH_PORT (attempt $i)"
		break
	fi
	sleep 1
done

# Verify sshd is listening
if ! nc -z 127.0.0.1 $SSH_PORT; then
	echo "ERROR: sshd not listening on port $SSH_PORT"
	exit 1
fi

echo "--- Write env file ---"
cat >"$ENV_FILE" <<EOF
export SSH_PORT=$SSH_PORT
export SSH_KEY_PATH=$TEST_PREFIX/.ssh/id_ed25519
export SSH_PUB_KEY_PATH=$TEST_PREFIX/.ssh/id_ed25519.pub
export SSHD_PID=$SSHD_PID
export TEST_PREFIX=$TEST_PREFIX
export BINARY="target/debug/sshping"

# Run a command with trace: echo to stderr, then execute
run() { echo "cmd: \$*" >&2; "\$@"; }
EOF

# Source env file so variables are available in this script too
# shellcheck source=/dev/null
source "$ENV_FILE"

echo "--- Verify basic SSH connectivity ---"
ssh -o StrictHostKeyChecking=no \
	-o UserKnownHostsFile=/dev/null \
	-p "$SSH_PORT" \
	-i "$SSH_KEY_PATH" \
	127.0.0.1 echo "SSH connection OK" 2>&1 || {
	echo "ERROR: Basic SSH connectivity test failed. Dumping sshd log:"
	sudo journalctl -u sshd --since "1 minute ago" 2>/dev/null || true
	# If no journalctl, try reading the pidfile
	if [ -f "$TEST_PREFIX/sshd/sshd.pid" ]; then
		echo "sshd PID file: $(cat "$TEST_PREFIX/sshd/sshd.pid")"
	fi
	ps aux | grep sshd || true
	exit 1
}

echo "Setup complete. Env file: $ENV_FILE"
