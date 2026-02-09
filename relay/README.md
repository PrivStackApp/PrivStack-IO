# PrivStack Relay Server

P2P relay and bootstrap node for PrivStack sync.

## What it does

1. **Kademlia DHT** - Helps clients discover each other across the internet
2. **Relay** - Routes traffic between clients that can't connect directly (NAT traversal)

## Deployment

### 1. Copy to VPS

```bash
scp -r relay_server root@YOUR_VPS_IP:/opt/privstack
```

### 2. Build on VPS

```bash
ssh root@YOUR_VPS_IP
cd /opt/privstack

# Install Rust if needed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Build
cargo build --release
```

### 3. Set up firewall

```bash
ufw allow 22/tcp    # SSH
ufw allow 4001/udp  # PrivStack relay (QUIC)
ufw allow 4002/tcp  # PrivStack relay HTTP identity API
ufw enable
```

### 4. Install as systemd service

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

### 5. Check status

```bash
systemctl status privstack-relay
journalctl -u privstack-relay -f
```

## Query relay identity

The relay exposes an HTTP endpoint for clients to discover its PeerId and addresses at runtime:

```bash
curl http://YOUR_VPS_IP:4002/api/v1/identity
```

This eliminates the need to hardcode PeerIds in client code.

## Useful commands

```bash
# View logs
journalctl -u privstack-relay -f

# Restart
systemctl restart privstack-relay

# Run manually with debug
systemctl stop privstack-relay
/usr/local/bin/privstack-relay --verbose

# Check listening ports
ss -ulnp | grep 4001
```
