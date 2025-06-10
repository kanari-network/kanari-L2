# วิธีตั้งค่า Kanari Node

คู่มือการตั้งค่า Kanari Node สำหรับการพัฒนาและการใช้งานจริง

## สารบัญ

1. [ข้อกำหนดระบบ](#ข้อกำหนดระบบ)
2. [การติดตั้ง](#การติดตั้ง)
3. [การตั้งค่าเริ่มต้น](#การตั้งค่าเริ่มต้น)
4. [การเริ่มต้น Node](#การเริ่มต้น-node)
5. [ประเภทของ Network](#ประเภทของ-network)
6. [การตั้งค่า Bitcoin Integration](#การตั้งค่า-bitcoin-integration)
7. [การใช้งานขั้นสูง](#การใช้งานขั้นสูง)
8. [การแก้ไขปัญหา](#การแก้ไขปัญหา)
9. [การกระจาย Node ไปเครื่องอื่น](#การกระจาย-node-ไปเครื่องอื่น)

## ข้อกำหนดระบบ

### ขั้นต่ำ
- **Operating System**: Linux, macOS, Windows
- **RAM**: 4GB ขั้นต่ำ, 8GB แนะนำ
- **Storage**: 20GB พื้นที่ว่างขั้นต่ำ
- **Network**: การเชื่อมต่ออินเทอร์เน็ตที่เสถียร

### Software Dependencies
- **Rust**: 1.75.0 หรือใหม่กว่า
- **Git**: สำหรับ clone repository
- **Bitcoin Core** (ถ้าต้องการ Bitcoin integration)

## การติดตั้ง

### 1. ติดตั้ง Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 2. Clone และ Build Kanari
```bash
# Clone repository
git clone https://github.com/rooch-network/rooch.git
cd rooch

# Build จาก source
cargo build --release

# Copy binary ไปยัง PATH (Linux/macOS)
cp target/release/kanari ~/.cargo/bin/

# สำหรับ Windows
copy target\release\kanari.exe %USERPROFILE%\.cargo\bin\
```

### 3. ตรวจสอบการติดตั้ง
```bash
kanari --version
```

## การตั้งค่าเริ่มต้น

### 1. Initialize Kanari Config
```bash
kanari init
```

คำสั่งนี้จะ:
- สร้าง keypair ใหม่
- ตั้งค่า default network environment
- สร้างไฟล์ config ที่ `~/.kanari/kanari.yaml`

### 2. ตั้งค่า Manual (ถ้าต้องการ)
```bash
# ตั้งค่าด้วย custom server URL
kanari init --server-url http://127.0.0.1:6767

# ตั้งค่าด้วย custom mnemonic
kanari init --mnemonic-phrase "your twelve word mnemonic phrase here"

# ข้าม password setup (สำหรับ testing)
kanari init --skip-password
```

## การเริ่มต้น Node

### 1. Local Development Node
```bash
# เริ่ม local node พื้นฐาน
kanari server start -n local

# เริ่มด้วย debug logging
RUST_LOG=debug kanari server start -n local

# เริ่มด้วย custom port
kanari server start -n local -p 8080
```

### 2. ด้วย Custom Configuration
```bash
# ระบุ data directory
kanari server start -n local --data-dir /path/to/custom/data

# ตั้งค่า proposer interval
kanari server start -n local --proposer-block-interval 10

# ตั้งค่า sequencer และ proposer accounts
kanari server start -n local \
  --sequencer-account bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh \
  --proposer-account bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh
```

## ประเภทของ Network

### 1. Local Network
```bash
kanari server start -n local
```
- **ChainID**: 4
- **Bitcoin Network**: regtest
- **ใช้สำหรับ**: Development และ Testing
- **คุณสมบัติ**: Full control, Fast blocks, No external dependencies

### 2. Dev Network
```bash
kanari server start -n dev
```
- **ChainID**: 3
- **Bitcoin Network**: regtest
- **RPC**: https://dev-seed.kanari.site/
- **ใช้สำหรับ**: Development และ Integration Testing

### 3. Test Network
```bash
kanari server start -n test
```
- **ChainID**: 2
- **Bitcoin Network**: test
- **RPC**: https://test-seed.kanari.site/
- **ใช้สำหรับ**: Public Testing

### 4. Main Network
```bash
kanari server start -n main
```
- **ChainID**: 1
- **Bitcoin Network**: main
- **RPC**: https://main-seed.kanari.site/
- **ใช้สำหรับ**: Production

## การตั้งค่า Bitcoin Integration

### 1. ติดตั้ง Bitcoin Core
```bash
# Ubuntu/Debian
sudo apt-get install bitcoind

# macOS (ใช้ Homebrew)
brew install bitcoin

# หรือ download จาก https://bitcoin.org/en/download
```

### 2. ตั้งค่า Bitcoin Node
สร้างไฟล์ `~/.bitcoin/bitcoin.conf`:
```ini
# Bitcoin Core configuration
regtest=1
server=1
rpcuser=kanariuser
rpcpassword=kanaripass
rpcport=18443
rpcbind=127.0.0.1
rpcallowip=127.0.0.1
txindex=1
```

### 3. เริ่ม Bitcoin Node
```bash
bitcoind -daemon
```

### 4. เริ่ม Kanari พร้อม Bitcoin Integration
```bash
kanari server start -n local \
  --btc-rpc-url http://127.0.0.1:18443 \
  --btc-rpc-username kanariuser \
  --btc-rpc-password kanaripass \
  --btc-sync-block-interval 1
```

## การใช้งานขั้นสูง

### 1. Genesis Configuration
```bash
# Initialize genesis สำหรับ custom network
kanari genesis init -d /path/to/data -n local

# ใช้ custom genesis config
kanari server start -n local --genesis-config /path/to/genesis.json
```

### 2. DA (Data Availability) Configuration
```bash
# ตั้งค่า DA backend
kanari server start -n local \
  --da-backend celestia \
  --da-min-block-to-submit 100 \
  --background-submit-interval 30
```

### 3. Metrics และ Monitoring
```bash
# เปิด metrics endpoint (default port: 9184)
export METRICS_HOST_PORT=9184
kanari server start -n local

# ตั้งค่า rate limiting
kanari server start -n local \
  --traffic-burst-size 1000 \
  --traffic-per-second 10.0
```

### 4. API Endpoints
เมื่อ node ทำงานแล้ว จะมี endpoints ต่อไปนี้:

- **JSON-RPC**: `http://localhost:6767/`
- **WebSocket**: `ws://localhost:6767/`
- **Metrics**: `http://localhost:9184/metrics`
- **SSE Events**: `http://localhost:6767/subscribe/sse/events`
- **SSE Transactions**: `http://localhost:6767/subscribe/sse/transactions`

## การแก้ไขปัญหา

### 1. ปัญหาทั่วไป

#### Node ไม่สามารถเริ่มได้
```bash
# ทำความสะอาด data directory
kanari server clean -n local

# Initialize genesis ใหม่
kanari genesis init -d ~/.kanari/local -n local
```

#### Port ถูกใช้งานอยู่
```bash
# ใช้ port อื่น
kanari server start -n local -p 8080

# หรือหา process ที่ใช้ port 6767
lsof -i :6767  # Linux/macOS
netstat -ano | findstr :6767  # Windows
```

#### Bitcoin connection ล้มเหลว
```bash
# ตรวจสอบ Bitcoin node status
bitcoin-cli -regtest getblockchaininfo

# ตรวจสอบ bitcoin.conf
cat ~/.bitcoin/bitcoin.conf

# Restart Bitcoin node
bitcoin-cli -regtest stop
bitcoind -daemon
```

### 2. Debugging

#### เปิด Debug Logging
```bash
RUST_LOG=debug kanari server start -n local
```

#### ตรวจสอบ Log Levels
```bash
# Trace level (มาก)
RUST_LOG=trace kanari server start -n local

# Specific module logging
RUST_LOG=kanari_rpc_server=debug kanari server start -n local

# Multiple modules
RUST_LOG=kanari_rpc_server=debug,kanari_sequencer=info kanari server start -n local
```

### 3. Performance Tuning

#### สำหรับ Production
```bash
kanari server start -n main \
  --traffic-burst-size 200 \
  --traffic-per-second 0.1 \
  --proposer-block-interval 5 \
  --data-dir /var/lib/kanari
```

#### สำหรับ Development
```bash
kanari server start -n local \
  --traffic-burst-size 5000 \
  --traffic-per-second 0.001 \
  --proposer-block-interval 1
```

## การกระจาย Node ไปเครื่องอื่น

### 1. การเตรียม Binary สำหรับกระจาย

#### สร้าง Binary Release
```bash
# Build optimized binary
cargo build --release

# หรือ build สำหรับ target specific
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-pc-windows-gnu
cargo build --release --target aarch64-apple-darwin
```

#### แพคเกจ Binary
```bash
# Linux/macOS
tar -czf kanari-node-v1.0.0-linux-x64.tar.gz -C target/release kanari

# Windows
powershell Compress-Archive -Path target\release\kanari.exe -DestinationPath kanari-node-v1.0.0-windows-x64.zip
```

### 2. การติดตั้งบนเครื่องปลายทาง

#### วิธีที่ 1: การติดตั้งจาก Pre-built Binary
```bash
# Download และติดตั้ง
wget https://releases.kanari.site/v1.0.0/kanari-node-v1.0.0-linux-x64.tar.gz
tar -xzf kanari-node-v1.0.0-linux-x64.tar.gz
sudo mv kanari /usr/local/bin/
chmod +x /usr/local/bin/kanari
```

#### วิธีที่ 2: การ Build จาก Source
```bash
# ติดตั้ง dependencies
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone และ build
git clone https://github.com/rooch-network/rooch.git
cd rooch
cargo build --release
sudo cp target/release/kanari /usr/local/bin/
```

### 3. การตั้งค่า Multi-Node Network

#### Node Configuration สำหรับ Network
```bash
# Node 1 (Primary/Bootstrap Node)
kanari server start -n custom \
  --data-dir /var/lib/kanari/node1 \
  --port 6767 \
  --peer-id node1 \
  --bootstrap-peers ""

# Node 2 (Secondary Node)
kanari server start -n custom \
  --data-dir /var/lib/kanari/node2 \
  --port 6768 \
  --peer-id node2 \
  --bootstrap-peers "/ip4/192.168.1.100/tcp/6767/p2p/node1"

# Node 3 (Tertiary Node)
kanari server start -n custom \
  --data-dir /var/lib/kanari/node3 \
  --port 6769 \
  --peer-id node3 \
  --bootstrap-peers "/ip4/192.168.1.100/tcp/6767/p2p/node1,/ip4/192.168.1.101/tcp/6768/p2p/node2"
```

### 4. Load Balancer Setup

#### Nginx Configuration
```nginx
upstream kanari_nodes {
    server 192.168.1.100:6767 weight=3;
    server 192.168.1.101:6768 weight=2;
    server 192.168.1.102:6769 weight=1;
}

server {
    listen 80;
    server_name kanari.example.com;
    
    location / {
        proxy_pass http://kanari_nodes;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        
        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

#### HAProxy Configuration
```
global
    daemon
    maxconn 4096

defaults
    mode http
    timeout connect 5000ms
    timeout client 50000ms
    timeout server 50000ms

frontend kanari_frontend
    bind *:80
    default_backend kanari_backend

backend kanari_backend
    balance roundrobin
    option httpchk GET /health
    server node1 192.168.1.100:6767 check
    server node2 192.168.1.101:6768 check
    server node3 192.168.1.102:6769 check
```

### 5. Docker Deployment

#### Dockerfile สำหรับ Production
```dockerfile
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY kanari /usr/local/bin/
RUN chmod +x /usr/local/bin/kanari

EXPOSE 6767 9184

VOLUME ["/data"]

CMD ["kanari", "server", "start", "-n", "main", "--data-dir", "/data"]
```

#### Docker Compose สำหรับ Multi-Node
```yaml
version: '3.8'

services:
  kanari-node1:
    image: kanari:latest
    container_name: kanari-node1
    ports:
      - "6767:6767"
      - "9184:9184"
    volumes:
      - ./data/node1:/data
    environment:
      - RUST_LOG=info
    command: >
      kanari server start -n custom
      --data-dir /data
      --port 6767
      --peer-id node1

  kanari-node2:
    image: kanari:latest
    container_name: kanari-node2
    ports:
      - "6768:6767"
      - "9185:9184"
    volumes:
      - ./data/node2:/data
    environment:
      - RUST_LOG=info
    command: >
      kanari server start -n custom
      --data-dir /data
      --port 6767
      --peer-id node2
      --bootstrap-peers "/ip4/kanari-node1/tcp/6767/p2p/node1"
    depends_on:
      - kanari-node1

  kanari-node3:
    image: kanari:latest
    container_name: kanari-node3
    ports:
      - "6769:6767"
      - "9186:9184"
    volumes:
      - ./data/node3:/data
    environment:
      - RUST_LOG=info
    command: >
      kanari server start -n custom
      --data-dir /data
      --port 6767
      --peer-id node3
      --bootstrap-peers "/ip4/kanari-node1/tcp/6767/p2p/node1,/ip4/kanari-node2/tcp/6767/p2p/node2"
    depends_on:
      - kanari-node1
      - kanari-node2

  nginx:
    image: nginx:alpine
    container_name: kanari-lb
    ports:
      - "80:80"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
    depends_on:
      - kanari-node1
      - kanari-node2
      - kanari-node3
```

### 6. Systemd Service Configuration

#### สร้าง Service File
```ini
# /etc/systemd/system/kanari.service
[Unit]
Description=Kanari Node
After=network.target
Wants=network.target

[Service]
Type=simple
User=kanari
Group=kanari
WorkingDirectory=/home/kanari
ExecStart=/usr/local/bin/kanari server start -n main --data-dir /var/lib/kanari
Restart=always
RestartSec=10
Environment=RUST_LOG=info

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/kanari

[Install]
WantedBy=multi-user.target
```

#### การจัดการ Service
```bash
# สร้าง user สำหรับ kanari
sudo useradd -r -s /bin/false kanari
sudo mkdir -p /var/lib/kanari
sudo chown kanari:kanari /var/lib/kanari

# เปิดใช้งาน service
sudo systemctl daemon-reload
sudo systemctl enable kanari
sudo systemctl start kanari

# ตรวจสอบสถานะ
sudo systemctl status kanari

# ดู logs
sudo journalctl -u kanari -f
```

### 7. การ Monitor และ Health Check

#### Health Check Script
```bash
#!/bin/bash
# health-check.sh

NODE_URL="http://localhost:6767"
HEALTH_ENDPOINT="$NODE_URL/health"

# ตรวจสอบการตอบสนองของ node
if curl -s -f "$HEALTH_ENDPOINT" > /dev/null; then
    echo "Node is healthy"
    exit 0
else
    echo "Node is unhealthy"
    exit 1
fi
```

#### Monitoring with Prometheus
```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'kanari-nodes'
    static_configs:
      - targets: 
        - '192.168.1.100:9184'
        - '192.168.1.101:9184'
        - '192.168.1.102:9184'
    metrics_path: /metrics
    scrape_interval: 10s
```

### 8. Network Security

#### Firewall Configuration (iptables)
```bash
# อนุญาต connection สำหรับ Kanari
sudo iptables -A INPUT -p tcp --dport 6767 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 9184 -j ACCEPT

# อนุญาตเฉพาะ IP ที่เชื่อถือได้
sudo iptables -A INPUT -p tcp -s 192.168.1.0/24 --dport 6767 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 6767 -j DROP

# บันทึก rules
sudo iptables-save > /etc/iptables/rules.v4
```

#### SSL/TLS Configuration
```bash
# สร้าง SSL certificate
sudo certbot certonly --standalone -d kanari.example.com

# Update nginx config สำหรับ HTTPS
server {
    listen 443 ssl;
    server_name kanari.example.com;
    
    ssl_certificate /etc/letsencrypt/live/kanari.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/kanari.example.com/privkey.pem;
    
    location / {
        proxy_pass http://kanari_nodes;
        # ... other proxy settings
    }
}
```

### 9. Backup และ Disaster Recovery

#### Automated Backup Script
```bash
#!/bin/bash
# backup-kanari.sh

BACKUP_DIR="/backup/kanari"
DATA_DIR="/var/lib/kanari"
DATE=$(date +%Y%m%d_%H%M%S)

# สร้าง backup directory
mkdir -p $BACKUP_DIR

# Stop node สำหรับ consistent backup
sudo systemctl stop kanari

# Backup data
tar -czf "$BACKUP_DIR/kanari_backup_$DATE.tar.gz" -C "$DATA_DIR" .

# Start node อีกครั้ง
sudo systemctl start kanari

# ลบ backup เก่า (เก็บไว้ 7 วัน)
find $BACKUP_DIR -name "kanari_backup_*.tar.gz" -mtime +7 -delete

echo "Backup completed: kanari_backup_$DATE.tar.gz"
```

#### Cron Job สำหรับ Automated Backup
```bash
# เพิ่มใน crontab
crontab -e

# Backup ทุกวันเวลา 2:00 AM
0 2 * * * /home/kanari/backup-kanari.sh
```

### 10. การอัพเดท และ Maintenance

#### Rolling Update Script
```bash
#!/bin/bash
# rolling-update.sh

NODES=("192.168.1.100" "192.168.1.101" "192.168.1.102")
NEW_BINARY="/tmp/kanari-new"

for node in "${NODES[@]}"; do
    echo "Updating node: $node"
    
    # Copy new binary
    scp $NEW_BINARY root@$node:/usr/local/bin/kanari-new
    
    # Update node
    ssh root@$node "
        systemctl stop kanari
        mv /usr/local/bin/kanari /usr/local/bin/kanari-old
        mv /usr/local/bin/kanari-new /usr/local/bin/kanari
        chmod +x /usr/local/bin/kanari
        systemctl start kanari
        sleep 30
        systemctl status kanari
    "
    
    echo "Node $node updated successfully"
    sleep 60  # รอให้ node sync ก่อนอัพเดท node ถัดไป
done
```

### 11. Performance Optimization สำหรับ Production

#### System Tuning
```bash
# เพิ่ม file descriptor limit
echo "kanari soft nofile 65536" >> /etc/security/limits.conf
echo "kanari hard nofile 65536" >> /etc/security/limits.conf

# Network tuning
echo "net.core.rmem_max = 134217728" >> /etc/sysctl.conf
echo "net.core.wmem_max = 134217728" >> /etc/sysctl.conf
echo "net.ipv4.tcp_rmem = 4096 65536 134217728" >> /etc/sysctl.conf
echo "net.ipv4.tcp_wmem = 4096 65536 134217728" >> /etc/sysctl.conf

sysctl -p
```

#### Resource Allocation
```bash
# สำหรับ production node
kanari server start -n main \
  --data-dir /var/lib/kanari \
  --traffic-burst-size 200 \
  --traffic-per-second 0.1 \
  --proposer-block-interval 5 \
  --max-connections 1000
```

## การใช้งาน CLI Commands

### 1. Server Management
```bash
# Start server
kanari server start -n local

# Clean data
kanari server clean -n local
```

### 2. Move Development
```bash
# Create new Move project
kanari move new my_project

# Build Move project
kanari move build -p my_project

# Publish Move project
kanari move publish -p my_project
```

### 3. Account Management
```bash
# List accounts
kanari account list

# Create new account
kanari account create

# Import account
kanari account import --private-key <key>
```

### 4. Transaction Management
```bash
# Send transaction
kanari transaction send --to <address> --amount <amount>

# Query transaction
kanari transaction get --hash <tx_hash>
```

## Configuration Files

### 1. Client Config (`~/.kanari/kanari.yaml`)
```yaml
keystore_path: ~/.kanari/kanari.keystore
active_address: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
active_env: "local"
envs:
  - alias: "local"
    rpc: "http://127.0.0.1:6767"
    ws: null
  - alias: "dev"
    rpc: "https://dev-seed.kanari.site"
    ws: null
```

### 2. Genesis Config (ตัวอย่าง)
```json
{
  "chain_id": 4,
  "genesis_config": {
    "sequencer_account": "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
    "bitcoin_network": 0,
    "timestamp": 1640995200000
  }
}
```

## Security Notes

1. **Private Keys**: เก็บ private keys อย่างปลอดภัย
2. **Network Access**: ใช้ firewall สำหรับ production
3. **Regular Updates**: อัพเดท Kanari เป็นประจำ
4. **Backup**: สำรองข้อมูล keystore และ config files

## ข้อมูลเพิ่มเติม

- **Documentation**: https://kanari.site/
- **GitHub**: https://github.com/rooch-network/rooch
- **Discord**: [Kanari Community Discord]
- **Examples**: ดูตัวอย่างการใช้งานในโฟลเดอร์ `examples/`

---

*คู่มือนี้จัดทำขึ้นสำหรับ Kanari Node version ล่าสุด หากพบปัญหาหรือต้องการความช่วยเหลือ กรุณาติดต่อ community*