# Relay Deployment

The PrivStack relay server (`privstack-relay`) is a standalone Rust binary that provides P2P peer discovery and NAT traversal for clients that cannot connect directly.

## What It Does

1. **Kademlia DHT** — Helps clients discover each other across the internet
2. **QUIC Relay** — Routes encrypted traffic between clients behind NATs
3. **DCUtR** — Coordinates direct connection upgrades (hole-punching)
4. **Identity API** — HTTP endpoint for clients to discover the relay's PeerId

## Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 4001 | UDP (QUIC) | P2P relay and DHT traffic |
| 4002 | TCP (HTTP) | Identity discovery API |

## Dependencies

The relay uses:

- **libp2p 0.56** — QUIC, mDNS, Kademlia, Noise, Yamux, Relay, DCUtR, Identify
- **Tokio 1.43** — Async runtime
- **Axum 0.8** — HTTP API server
- **Clap 4.5** — CLI argument parsing

## Building

```bash
cd PrivStack-IO/relay
cargo build --release
```

Output: `target/release/privstack-relay`

The release profile enables LTO, single codegen unit, abort-on-panic, and symbol stripping for a small, optimized binary.

## VPS Deployment

### 1. Copy to Server

```bash
scp -r relay/ root@YOUR_VPS_IP:/opt/privstack
```

### 2. Build on Server

```bash
ssh root@YOUR_VPS_IP
cd /opt/privstack

# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

cargo build --release
```

### 3. Firewall Configuration

```bash
ufw allow 22/tcp    # SSH
ufw allow 4001/udp  # QUIC relay
ufw allow 4002/tcp  # HTTP identity API
ufw enable
```

### 4. Systemd Service

```bash
# Create system user
useradd --system --no-create-home privstack

# Install binary
cp target/release/privstack-relay /usr/local/bin/
chown privstack:privstack /usr/local/bin/privstack-relay

# Create data directory
mkdir -p /var/lib/privstack
chown -R privstack:privstack /var/lib/privstack

# Create service file
cat > /etc/systemd/system/privstack-relay.service << 'EOF'
[Unit]
Description=PrivStack P2P Relay
After=network.target

[Service]
Type=simple
User=privstack
ExecStart=/usr/local/bin/privstack-relay --identity /var/lib/privstack/relay-identity.key
Restart=always
RestartSec=5
WorkingDirectory=/var/lib/privstack

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
systemctl daemon-reload
systemctl enable privstack-relay
systemctl start privstack-relay
```

### 5. Verify

```bash
# Check service status
systemctl status privstack-relay

# View logs
journalctl -u privstack-relay -f

# Test identity endpoint
curl http://localhost:4002/api/v1/identity
```

## Identity API

Clients discover the relay's PeerId and multiaddresses at runtime:

```bash
curl http://YOUR_VPS_IP:4002/api/v1/identity
```

Response:

```json
{
  "peer_id": "12D3KooW...",
  "addresses": [
    "/ip4/YOUR_VPS_IP/udp/4001/quic-v1"
  ]
}
```

This eliminates hardcoded PeerIds in client configurations. Clients fetch the relay identity on startup and use it for DHT bootstrapping.

## CLI Options

| Flag | Description |
|------|-------------|
| `--identity <path>` | Path to persistent identity key file |
| `--verbose` | Enable debug-level logging |

## Useful Commands

```bash
# View logs
journalctl -u privstack-relay -f

# Restart
systemctl restart privstack-relay

# Run manually with debug logging
systemctl stop privstack-relay
/usr/local/bin/privstack-relay --verbose

# Check listening ports
ss -ulnp | grep 4001
ss -tlnp | grep 4002
```
