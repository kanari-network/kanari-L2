#!/bin/bash

# Kanari L2 Validator Node Setup Script
# This script helps you set up a Kanari L2 validator node

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
KANARI_VERSION="latest"
KANARI_USER="kanari"
KANARI_HOME="/home/$KANARI_USER"
KANARI_DATA_DIR="/var/lib/kanari"
KANARI_LOG_DIR="/var/log/kanari"

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
check_root() {
    if [[ $EUID -eq 0 ]]; then
        print_error "This script should not be run as root"
        exit 1
    fi
}

# Check system requirements
check_requirements() {
    print_status "Checking system requirements..."
    
    # Check OS
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        print_success "Operating System: Linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        print_success "Operating System: macOS"
    else
        print_error "Unsupported operating system: $OSTYPE"
        exit 1
    fi
    
    # Check memory
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        MEMORY_GB=$(free -g | awk '/^Mem:/{print $2}')
        if [ "$MEMORY_GB" -lt 4 ]; then
            print_warning "Low memory detected: ${MEMORY_GB}GB (recommended: 8GB+)"
        else
            print_success "Memory: ${MEMORY_GB}GB"
        fi
    fi
    
    # Check disk space
    DISK_SPACE=$(df -BG / | awk 'NR==2{print $4}' | sed 's/G//')
    if [ "$DISK_SPACE" -lt 50 ]; then
        print_warning "Low disk space: ${DISK_SPACE}GB (recommended: 100GB+)"
    else
        print_success "Disk space: ${DISK_SPACE}GB"
    fi
}

# Install dependencies
install_dependencies() {
    print_status "Installing dependencies..."
    
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        # Update package list
        sudo apt-get update
        
        # Install required packages
        sudo apt-get install -y \
            curl \
            git \
            build-essential \
            pkg-config \
            libssl-dev \
            ca-certificates \
            gnupg \
            lsb-release
        
        print_success "Dependencies installed"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        # Check if Homebrew is installed
        if ! command -v brew &> /dev/null; then
            print_status "Installing Homebrew..."
            /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        fi
        
        # Install dependencies
        brew install curl git openssl pkg-config
        print_success "Dependencies installed"
    fi
}

# Install Rust
install_rust() {
    print_status "Installing Rust..."
    
    if command -v rustc &> /dev/null; then
        RUST_VERSION=$(rustc --version | cut -d' ' -f2)
        print_success "Rust already installed: $RUST_VERSION"
        return
    fi
    
    # Install Rust
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
    
    # Update Rust
    rustup update
    
    print_success "Rust installed successfully"
}

# Install Kanari
install_kanari() {
    print_status "Installing Kanari..."
    
    # Clone repository
    if [ -d "rooch" ]; then
        print_status "Repository already exists, updating..."
        cd rooch
        git pull
    else
        git clone https://github.com/rooch-network/rooch.git
        cd rooch
    fi
    
    # Build Kanari
    print_status "Building Kanari (this may take 10-30 minutes)..."
    cargo build --release
    
    # Install binary
    sudo cp target/release/kanari /usr/local/bin/
    sudo chmod +x /usr/local/bin/kanari
    
    # Verify installation
    KANARI_VERSION=$(kanari --version)
    print_success "Kanari installed: $KANARI_VERSION"
    
    cd ..
}

# Create system user
create_user() {
    print_status "Creating system user..."
    
    if id "$KANARI_USER" &>/dev/null; then
        print_success "User $KANARI_USER already exists"
        return
    fi
    
    sudo useradd -m -s /bin/bash $KANARI_USER
    sudo usermod -aG sudo $KANARI_USER
    
    print_success "User $KANARI_USER created"
}

# Setup directories
setup_directories() {
    print_status "Setting up directories..."
    
    # Create data directory
    sudo mkdir -p $KANARI_DATA_DIR
    sudo chown $KANARI_USER:$KANARI_USER $KANARI_DATA_DIR
    
    # Create log directory
    sudo mkdir -p $KANARI_LOG_DIR
    sudo chown $KANARI_USER:$KANARI_USER $KANARI_LOG_DIR
    
    # Create config directory
    sudo mkdir -p $KANARI_HOME/.kanari
    sudo chown $KANARI_USER:$KANARI_USER $KANARI_HOME/.kanari
    
    print_success "Directories created"
}

# Initialize Kanari
initialize_kanari() {
    print_status "Initializing Kanari..."
    
    # Switch to kanari user
    sudo -u $KANARI_USER bash -c "
        cd $KANARI_HOME
        kanari init --skip-password
    "
    
    print_success "Kanari initialized"
}

# Create systemd service
create_service() {
    print_status "Creating systemd service..."
    
    cat << EOF | sudo tee /etc/systemd/system/kanari-validator.service > /dev/null
[Unit]
Description=Kanari Validator Node
After=network.target
Wants=network.target

[Service]
Type=simple
User=$KANARI_USER
Group=$KANARI_USER
WorkingDirectory=$KANARI_HOME
ExecStart=/usr/local/bin/kanari server start -n local --data-dir $KANARI_DATA_DIR
Restart=always
RestartSec=10
Environment=RUST_LOG=info

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$KANARI_DATA_DIR
ReadWritePaths=$KANARI_LOG_DIR

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable kanari-validator
    
    print_success "Systemd service created"
}

# Setup firewall
setup_firewall() {
    print_status "Setting up firewall..."
    
    if command -v ufw &> /dev/null; then
        sudo ufw allow 6767/tcp comment "Kanari RPC"
        sudo ufw allow 9184/tcp comment "Kanari Metrics"
        sudo ufw allow ssh
        print_success "Firewall configured"
    else
        print_warning "UFW not installed, skipping firewall setup"
    fi
}

# Create monitoring script
create_monitoring() {
    print_status "Creating monitoring script..."
    
    cat << 'EOF' | sudo tee /usr/local/bin/kanari-monitor.sh > /dev/null
#!/bin/bash

# Kanari Validator Monitor Script

check_health() {
    echo "=== Kanari Validator Health Check ==="
    echo "Timestamp: $(date)"
    echo
    
    # Check service status
    echo "Service Status:"
    systemctl is-active kanari-validator
    echo
    
    # Check if port is open
    echo "Port Status:"
    if netstat -tulpn | grep -q :6767; then
        echo "✓ Port 6767 is open"
    else
        echo "✗ Port 6767 is not open"
    fi
    
    # Check API health
    echo "API Health:"
    if curl -s -f http://localhost:6767/health > /dev/null; then
        echo "✓ API is responding"
    else
        echo "✗ API is not responding"
    fi
    
    # Check disk usage
    echo "Disk Usage:"
    df -h /var/lib/kanari
    
    # Check memory usage
    echo "Memory Usage:"
    free -h
    
    echo "================================"
}

case "$1" in
    health)
        check_health
        ;;
    *)
        echo "Usage: $0 {health}"
        exit 1
        ;;
esac
EOF

    sudo chmod +x /usr/local/bin/kanari-monitor.sh
    print_success "Monitoring script created"
}

# Create backup script
create_backup() {
    print_status "Creating backup script..."
    
    cat << 'EOF' | sudo tee /usr/local/bin/kanari-backup.sh > /dev/null
#!/bin/bash

# Kanari Validator Backup Script

BACKUP_DIR="/var/backups/kanari"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="kanari_backup_$DATE.tar.gz"

echo "Starting Kanari backup..."

# Create backup directory
mkdir -p $BACKUP_DIR

# Stop service
systemctl stop kanari-validator

# Create backup
tar -czf $BACKUP_DIR/$BACKUP_FILE \
    /home/kanari/.kanari/kanari.yaml \
    /home/kanari/.kanari/kanari.keystore \
    /var/lib/kanari

# Start service
systemctl start kanari-validator

# Remove old backups (keep only last 7 days)
find $BACKUP_DIR -name "kanari_backup_*.tar.gz" -mtime +7 -delete

echo "Backup completed: $BACKUP_DIR/$BACKUP_FILE"
EOF

    sudo chmod +x /usr/local/bin/kanari-backup.sh
    print_success "Backup script created"
}

# Print completion message
print_completion() {
    print_success "Kanari Validator Node setup completed!"
    echo
    echo "Next steps:"
    echo "1. Start the validator service:"
    echo "   sudo systemctl start kanari-validator"
    echo
    echo "2. Check service status:"
    echo "   sudo systemctl status kanari-validator"
    echo
    echo "3. View logs:"
    echo "   sudo journalctl -u kanari-validator -f"
    echo
    echo "4. Check health:"
    echo "   kanari-monitor.sh health"
    echo
    echo "5. Create backup:"
    echo "   sudo kanari-backup.sh"
    echo
    echo "Important files:"
    echo "- Config: /home/$KANARI_USER/.kanari/kanari.yaml"
    echo "- Keystore: /home/$KANARI_USER/.kanari/kanari.keystore"
    echo "- Data: $KANARI_DATA_DIR"
    echo "- Logs: $KANARI_LOG_DIR"
    echo
    print_warning "Make sure to backup your keystore file!"
}

# Main function
main() {
    echo "Kanari L2 Validator Node Setup"
    echo "=============================="
    echo
    
    check_root
    check_requirements
    install_dependencies
    install_rust
    install_kanari
    create_user
    setup_directories
    initialize_kanari
    create_service
    setup_firewall
    create_monitoring
    create_backup
    print_completion
}

# Run main function
main "$@"
