#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/deploy.sh <user>@<hostname>"
    exit 1
fi

# Backup
ssh $1 <<'EOS'
TS=$(date +'%Y-%m-%d_%H-%M-%S')
sudo cp -a /home/lsd/lsd /home/lsd/backups/lsd.$TS
sudo ln -sfn /home/lsd/backups/lsd.$TS /home/lsd/backups/lsd.latest
sudo cp -a /home/lsd/db.sqlite /home/lsd/backups/db.$TS.sqlite
sudo ln -sfn /home/lsd/backups/db.$TS.sqlite /home/lsd/backups/db.latest.sqlite
EOS

# Deploy and restart
rsync --rsync-path="sudo rsync" -Pavz target/aarch64-unknown-linux-gnu/release/lsd $1:/home/lsd/
ssh $1 <<'EOS'
sudo setcap 'cap_net_bind_service=+ep' /home/lsd/lsd
sudo systemctl restart lsd
sudo systemctl is-active --quiet lsd
EOS
