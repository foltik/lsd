#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/deploy.sh <user>@<hostname>"
    exit 1
fi

ssh $1 <<'EOS'
# update
sudo yum update -y

# create a user
if ! id lsd &>/dev/null; then
    sudo adduser lsd
fi

# setup service
sudo tee /etc/systemd/system/lsd.service <<EOF >/dev/null
[Unit]
Description=LSD
After=network.target

[Service]
Type=simple
User=lsd
WorkingDirectory=/home/lsd
ExecStart=/home/lsd/lsd /home/lsd/config/prod.toml
Restart=always

[Install]
WantedBy=multi-user.target
EOF
sudo systemctl daemon-reload
sudo systemctl enable lsd
sudo systemctl restart lsd
EOS
