# Kanari L2 Validator Node Setup

ระบบ Kanari L2 Validator Node ที่ครบถ้วนสำหรับการรัน validator node, monitoring, และ management

## Quick Start

### 1. การติดตั้งแบบอัตโนมัติ (Linux/macOS)

```bash
# Download setup script
curl -O https://raw.githubusercontent.com/rooch-network/rooch/main/scripts/setup-validator.sh
chmod +x setup-validator.sh

# Run setup script
./setup-validator.sh
```

### 2. การติดตั้งด้วย Docker

```bash
# Clone repository
git clone https://github.com/rooch-network/rooch.git
cd rooch

# Copy configuration templates
cp config/validator.yaml.template config/validator.yaml
# Edit config/validator.yaml according to your needs

# Start validator with Docker Compose
docker-compose -f docker/docker-compose.validator.yml up -d
```

### 3. การติดตั้งแบบ Manual

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Build Kanari
git clone https://github.com/rooch-network/rooch.git
cd rooch
cargo build --release

# Install binary
sudo cp target/release/kanari /usr/local/bin/

# Initialize
kanari init

# Start validator
kanari server start -n local
```

## การใช้งาน

### เริ่มต้น Validator Node

#### Local Development
```bash
kanari server start -n local \
  --data-dir ~/.kanari/local \
  --proposer-block-interval 5
```

#### Production
```bash
kanari server start -n main \
  --data-dir /var/lib/kanari \
  --sequencer-account <your_sequencer_address> \
  --proposer-account <your_proposer_address>
```

### จัดการ Keys

```bash
# ดู accounts
kanari account list

# สร้าง account ใหม่
kanari account create --alias validator

# Import existing key
kanari account import --private-key <private_key>
```

### ตรวจสอบสถานะ

```bash
# ตรวจสอบ health
curl http://localhost:6767/health

# ตรวจสอบ metrics
curl http://localhost:9184/metrics

# ตรวจสอบ chain info
curl -X POST http://localhost:6767 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"kanari_getChainInfo","params":[],"id":1}'
```

## การตั้งค่า

### Network Types

| Network | Chain ID | Bitcoin Network | Description |
|---------|----------|-----------------|-------------|
| local   | 4        | regtest         | Development |
| dev     | 3        | regtest         | Integration Testing |
| test    | 2        | testnet         | Public Testing |
| main    | 1        | mainnet         | Production |

### Configuration Files

- **Client Config**: `~/.kanari/kanari.yaml`
- **Keystore**: `~/.kanari/kanari.keystore`
- **Validator Config**: `~/.kanari/validator.yaml`
- **Log Files**: `/var/log/kanari/`

### Key Components

1. **Sequencer**: จัดการการเรียงลำดับ transactions
2. **Proposer**: สร้างและเสนอ blocks ใหม่
3. **Validator**: ตรวจสอบความถูกต้องของ transactions และ blocks

## การ Monitor

### Systemd Service

```bash
# ตรวจสอบ status
sudo systemctl status kanari-validator

# ดู logs
sudo journalctl -u kanari-validator -f

# Restart service
sudo systemctl restart kanari-validator
```

### Monitoring Tools

1. **Prometheus**: http://localhost:9090
2. **Grafana**: http://localhost:3000 (admin/admin)
3. **Metrics API**: http://localhost:9184/metrics

### Health Check

```bash
# Manual health check
kanari-monitor.sh health

# API health check
curl http://localhost:6767/health
```

## การ Backup

### Automatic Backup
```bash
# สร้าง backup
sudo kanari-backup.sh

# Restore backup
tar -xzf /var/backups/kanari/kanari_backup_20240101_120000.tar.gz -C /
```

### Manual Backup
```bash
# Backup keys และ config
cp ~/.kanari/kanari.keystore ~/backup/
cp ~/.kanari/kanari.yaml ~/backup/

# Backup data
tar -czf kanari-data-backup.tar.gz /var/lib/kanari
```

## การแก้ไขปัญหา

### Common Issues

#### 1. Validator ไม่เริ่มต้น
```bash
# ตรวจสอบ logs
sudo journalctl -u kanari-validator -n 50

# ตรวจสอบ configuration
kanari server start -n local --dry-run

# ทำความสะอาด data
kanari server clean -n local
```

#### 2. Port conflicts
```bash
# ตรวจสอบ port usage
netstat -tulpn | grep 6767

# ใช้ port อื่น
kanari server start -n local --port 8080
```

#### 3. Key management issues
```bash
# ตรวจสอบ keystore
ls -la ~/.kanari/kanari.keystore

# Reset password
kanari account reset-password
```

### Debug Mode
```bash
# Enable debug logging
RUST_LOG=debug kanari server start -n local

# Specific module logging
RUST_LOG=kanari_sequencer=debug,kanari_proposer=info kanari server start -n local
```

## Security

### Best Practices

1. **Key Security**
   - เก็บ private keys ในที่ปลอดภัย
   - ใช้ hardware wallet สำหรับ production
   - สำรอง keystore files เป็นประจำ

2. **Network Security**
   - ใช้ firewall ป้องกัน unauthorized access
   - ใช้ VPN สำหรับ remote management
   - Enable SSL/TLS สำหรับ production

3. **System Security**
   - อัพเดท system และ dependencies เป็นประจำ
   - ใช้ dedicated user สำหรับ validator
   - Monitor system logs

### Firewall Configuration
```bash
# Ubuntu/Debian
sudo ufw allow 6767/tcp comment "Kanari RPC"
sudo ufw allow 9184/tcp comment "Kanari Metrics"
sudo ufw allow ssh
sudo ufw enable

# CentOS/RHEL
sudo firewall-cmd --permanent --add-port=6767/tcp
sudo firewall-cmd --permanent --add-port=9184/tcp
sudo firewall-cmd --reload
```

## Performance Tuning

### System Optimization
```bash
# Increase file descriptor limits
echo "kanari soft nofile 65536" >> /etc/security/limits.conf
echo "kanari hard nofile 65536" >> /etc/security/limits.conf

# Network tuning
echo "net.core.rmem_max = 134217728" >> /etc/sysctl.conf
echo "net.core.wmem_max = 134217728" >> /etc/sysctl.conf
sysctl -p
```

### Validator Settings
```bash
# Production settings
kanari server start -n main \
  --traffic-burst-size 200 \
  --traffic-per-second 0.1 \
  --proposer-block-interval 10 \
  --max-connections 1000
```

## Support

- **Documentation**: [Kanari Docs](https://kanari.network/docs)
- **GitHub**: [rooch-network/rooch](https://github.com/rooch-network/rooch)
- **Discord**: [Kanari Discord](https://discord.gg/kanari)
- **Telegram**: [Kanari Telegram](https://t.me/kanari_network)

## License

Apache 2.0 License. See [LICENSE](LICENSE) file for details.
