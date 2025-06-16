# การตั้งค่า Kanari L2 Validator Node

คู่มือการตั้งค่า Kanari L2 Validator Node สำหรับการ validate transactions และ propose blocks

## สารบัญ

1. [ข้อกำหนดระบบ](#ข้อกำหนดระบบ)
2. [การติดตั้ง](#การติดตั้ง)
3. [การตั้งค่า Validator Keys](#การตั้งค่า-validator-keys)
4. [การตั้งค่า Validator Node](#การตั้งค่า-validator-node)
5. [การรัน Validator Node](#การรัน-validator-node)
6. [การตั้งค่า Network ต่าง ๆ](#การตั้งค่า-network-ต่าง-ๆ)
7. [การ Monitor และ Maintenance](#การ-monitor-และ-maintenance)
8. [การแก้ไขปัญหา](#การแก้ไขปัญหา)

## ข้อกำหนดระบบ

### สำหรับ Validator Node
- **Operating System**: Linux (Ubuntu 20.04+ แนะนำ), macOS, Windows
- **RAM**: 8GB ขั้นต่ำ, 16GB แนะนำ
- **Storage**: 100GB SSD ขั้นต่ำ
- **Network**: การเชื่อมต่ออินเทอร์เน็ตที่เสถียร (10Mbps+)
- **CPU**: 4 cores ขั้นต่ำ

### Software Dependencies
- **Rust**: 1.75.0 หรือใหม่กว่า
- **Git**: สำหรับ clone repository
- **Bitcoin Core**: สำหรับ Bitcoin integration (optional)

## การติดตั้ง

### 1. ติดตั้ง Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup update
```

### 2. Clone และ Build Kanari
```bash
# Clone repository
git clone https://github.com/rooch-network/rooch.git
cd rooch

# ตรวจสอบ Rust version
rustc --version

# Build จาก source (อาจใช้เวลา 10-30 นาที)
cargo build --release

# Copy binary ไปยัง system PATH
sudo cp target/release/kanari /usr/local/bin/
# หรือสำหรับ Linux/macOS
cp target/release/kanari ~/.cargo/bin/
```

### 3. ตรวจสอบการติดตั้ง
```bash
kanari --version
```

## การตั้งค่า Validator Keys

### 1. สร้าง Validator Keys
```bash
# สร้าง keystore และ validator keys
kanari init

# ตัวอย่าง output:
# Generated new keypair for address: bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh
# Secret Recovery Phrase: [12 words mnemonic]
# Kanari client config file generated at ~/.kanari/kanari.yaml
```

### 2. Import Existing Keys (ถ้ามี)
```bash
# Import จาก mnemonic
kanari init --mnemonic-phrase "your twelve word mnemonic phrase here"

# Import จาก private key
kanari account import --private-key <your_private_key>
```

### 3. สร้าง Validator Keys แยกต่างหาก
```bash
# สร้าง account สำหรับ sequencer
kanari account create --alias sequencer

# สร้าง account สำหรับ proposer
kanari account create --alias proposer

# ดู accounts ที่มี
kanari account list
```

## การตั้งค่า Validator Node

### 1. การตั้งค่าเบื้องต้น

#### ตั้งค่า Genesis Configuration
```bash
# สร้าง genesis config สำหรับ local network
kanari genesis init -d ~/.kanari/local -n local

# หรือสำหรับ custom network
kanari genesis init -d /path/to/custom/data -n custom
```

#### ตั้งค่า Data Directory
```bash
# สร้าง data directory
sudo mkdir -p /var/lib/kanari
sudo chown $USER:$USER /var/lib/kanari

# หรือใช้ home directory
mkdir -p ~/.kanari/validator-data
```

### 2. การตั้งค่า Configuration Files

#### ไฟล์ Validator Config
สร้างไฟล์ `~/.kanari/validator.yaml`:
```yaml
# Validator Configuration
validator:
  # Validator identity
  validator_address: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
  
  # Network settings
  network: "local"  # local, dev, test, main
  
  # Data directory
  data_dir: "/var/lib/kanari"
  
  # Sequencer settings
  sequencer:
    account: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
    block_interval: 5  # seconds
    
  # Proposer settings  
  proposer:
    account: "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
    interval: 10  # seconds
    
  # API settings
  rpc:
    port: 6767
    host: "0.0.0.0"
    
  # Metrics
  metrics:
    enabled: true
    port: 9184
```

## การรัน Validator Node

### 1. รัน Local Validator Node
```bash
# รัน validator node แบบพื้นฐาน
kanari server start -n local

# รัน validator node ด้วย custom settings
kanari server start -n local \
  --data-dir /var/lib/kanari \
  --sequencer-account bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh \
  --proposer-account bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh \
  --proposer-block-interval 5

# รัน validator node ด้วย debug logging
RUST_LOG=debug kanari server start -n local
```

### 2. รัน Production Validator Node
```bash
# รัน validator node สำหรับ production
kanari server start -n main \
  --data-dir /var/lib/kanari \
  --sequencer-account <your_sequencer_address> \
  --proposer-account <your_proposer_address> \
  --proposer-block-interval 10 \
  --traffic-burst-size 200 \
  --traffic-per-second 0.1

# รัน validator node ด้วย custom port
kanari server start -n main \
  --port 8080 \
  --data-dir /var/lib/kanari
```

### 3. รัน Validator Node ด้วย Docker
```dockerfile
# Dockerfile
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY target/release/kanari /usr/local/bin/
RUN chmod +x /usr/local/bin/kanari

EXPOSE 6767 9184

VOLUME ["/data", "/keys"]

ENTRYPOINT ["kanari"]
CMD ["server", "start", "-n", "main", "--data-dir", "/data"]
```

```bash
# Build Docker image
docker build -t kanari-validator .

# Run validator container
docker run -d \
  --name kanari-validator \
  -p 6767:6767 \
  -p 9184:9184 \
  -v /var/lib/kanari:/data \
  -v ~/.kanari:/keys \
  kanari-validator
```

## การตั้งค่า Network ต่าง ๆ

### 1. Local Development Network
```bash
# ใช้สำหรับ development และ testing
kanari server start -n local \
  --data-dir ~/.kanari/local \
  --proposer-block-interval 1
```
- **ChainID**: 4
- **Bitcoin Network**: regtest
- **คุณสมบัติ**: Fast blocks, Full control

### 2. Development Network
```bash
# เชื่อมต่อกับ dev network
kanari server start -n dev \
  --data-dir ~/.kanari/dev
```
- **ChainID**: 3
- **Bitcoin Network**: regtest
- **RPC**: https://dev-seed.kanari.site/

### 3. Test Network
```bash
# เชื่อมต่อกับ test network
kanari server start -n test \
  --data-dir ~/.kanari/test
```
- **ChainID**: 2
- **Bitcoin Network**: testnet
- **RPC**: https://test-seed.kanari.site/

### 4. Main Network
```bash
# เชื่อมต่อกับ main network
kanari server start -n main \
  --data-dir /var/lib/kanari \
  --proposer-block-interval 10
```
- **ChainID**: 1
- **Bitcoin Network**: mainnet
- **RPC**: https://main-seed.kanari.site/

## การ Monitor และ Maintenance

### 1. Monitoring Validator Status
```bash
# ตรวจสอบ validator status
curl http://localhost:6767/health

# ตรวจสอบ metrics
curl http://localhost:9184/metrics

# ตรวจสอบ latest block
curl -X POST http://localhost:6767 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"kanari_getChainInfo","params":[],"id":1}'
```

### 2. Systemd Service Configuration
```ini
# /etc/systemd/system/kanari-validator.service
[Unit]
Description=Kanari Validator Node
After=network.target
Wants=network.target

[Service]
Type=simple
User=kanari
Group=kanari
WorkingDirectory=/home/kanari
ExecStart=/usr/local/bin/kanari server start -n main \
  --data-dir /var/lib/kanari \
  --sequencer-account bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh \
  --proposer-account bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh
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

```bash
# เปิดใช้งาน service
sudo systemctl daemon-reload
sudo systemctl enable kanari-validator
sudo systemctl start kanari-validator

# ตรวจสอบ status
sudo systemctl status kanari-validator

# ดู logs
sudo journalctl -u kanari-validator -f
```

### 3. Log Management
```bash
# ดู logs แบบ real-time
tail -f ~/.kanari/logs/kanari.log

# ดู logs ด้วย specific level
RUST_LOG=debug kanari server start -n local

# ดู logs ของ specific module
RUST_LOG=kanari_sequencer=debug,kanari_proposer=info kanari server start -n local
```

### 4. Backup และ Recovery
```bash
# Backup validator data
tar -czf kanari-backup-$(date +%Y%m%d).tar.gz \
  ~/.kanari/kanari.yaml \
  ~/.kanari/kanari.keystore \
  /var/lib/kanari

# Restore from backup
tar -xzf kanari-backup-20240101.tar.gz -C /

# Backup keys only
cp ~/.kanari/kanari.keystore ~/kanari-keys-backup.keystore
```

## การแก้ไขปัญหา

### 1. Validator ไม่สามารถเริ่มได้

#### ตรวจสอบ key configuration
```bash
# ตรวจสอบ accounts
kanari account list

# ตรวจสอบ active address
kanari account info

# ตรวจสอบ keystore
ls -la ~/.kanari/kanari.keystore
```

#### ตรวจสอบ data directory
```bash
# ทำความสะอาด data directory
kanari server clean -n local

# สร้าง genesis ใหม่
kanari genesis init -d ~/.kanari/local -n local
```

### 2. ปัญหา Network Connection
```bash
# ตรวจสอบ port availability
netstat -tulpn | grep 6767
lsof -i :6767

# ใช้ port อื่น
kanari server start -n local --port 8080

# ตรวจสอบ firewall
sudo ufw status
sudo ufw allow 6767
```

### 3. ปัญหา Key Management
```bash
# ตรวจสอบ password
kanari account list

# Reset password
kanari account reset-password

# Import key ใหม่
kanari account import --private-key <your_key>
```

### 4. ปัญหา Performance
```bash
# ตรวจสอบ system resources
htop
df -h
free -h

# ปรับแต่ง performance
kanari server start -n main \
  --traffic-burst-size 100 \
  --traffic-per-second 0.5 \
  --proposer-block-interval 5
```

### 5. Debug Mode
```bash
# รัน validator ด้วย debug logging
RUST_LOG=trace kanari server start -n local

# ดู specific module logs
RUST_LOG=kanari_sequencer=debug kanari server start -n local

# ดู transaction validation logs
RUST_LOG=kanari_executor=debug kanari server start -n local
```

## คำสั่งเสริม

### 1. Validator Management
```bash
# ตรวจสอบ validator status
kanari validator status

# ตรวจสอบ block production
kanari validator blocks

# ตรวจสอบ validator performance
kanari validator metrics
```

### 2. Transaction Management
```bash
# ตรวจสอบ transaction pool
curl -X POST http://localhost:6767 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"kanari_getPendingTransactions","params":[],"id":1}'

# Submit transaction
kanari transaction submit --data <tx_data>
```

### 3. Network Information
```bash
# ตรวจสอบ network info
curl -X POST http://localhost:6767 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"kanari_getChainInfo","params":[],"id":1}'

# ตรวจสอบ peers
curl -X POST http://localhost:6767 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"kanari_getPeers","params":[],"id":1}'
```

## Security Best Practices

1. **Key Management**
   - เก็บ private keys ในที่ปลอดภัย
   - ใช้ hardware wallet สำหรับ production
   - สำรอง keystore files เป็นประจำ

2. **Network Security**
   - ใช้ firewall ป้องกัน ports ที่ไม่จำเป็น
   - ใช้ VPN สำหรับ remote access
   - ตั้งค่า SSL/TLS สำหรับ RPC endpoints

3. **System Security**
   - อัพเดท OS และ dependencies เป็นประจำ
   - ใช้ dedicated user สำหรับ validator
   - Monitor system logs

4. **Operational Security**
   - ใช้ monitoring และ alerting
   - มี disaster recovery plan
   - ทดสอบ backup restoration เป็นประจำ

## ข้อมูลเพิ่มเติม

- **Documentation**: https://kanari.network/docs
- **GitHub**: https://github.com/rooch-network/rooch
- **Discord**: https://discord.gg/kanari
- **Telegram**: https://t.me/kanari_network

---

**หมายเหตุ**: คู่มือนี้อาจมีการเปลี่ยนแปลงตามการอัพเดทของ Kanari Network ควรตรวจสอบเอกสารล่าสุดเป็นประจำ
