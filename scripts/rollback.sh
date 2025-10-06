#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/rollback.sh <user>@<hostname>"
    exit 1
fi

ssh $1 <<'EOS'
sudo systemctl stop lsd
sudo cp -aL /home/lsd/backups/lsd.latest /home/lsd/lsd
sudo cp -aL /home/lsd/backups/db.latest.sqlite /home/lsd/db.sqlite
sudo setcap 'cap_net_bind_service=+ep' /home/lsd/lsd
sudo systemctl start lsd
EOS
