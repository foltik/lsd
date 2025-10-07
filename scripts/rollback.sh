#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/rollback.sh <user>@<hostname>"
    exit 1
fi

#ssh $1 <<'EOS'
ssh -i ~/.ssh/id_ed25519_lsd_root ec2-user@beta.lightandsound.design <<'EOS'
set -euxo pipefail
sudo systemctl stop lsd
sudo sqlite3 /home/lsd/db.sqlite "PRAGMA wal_checkpoint(TRUNCATE);"

TS=$(date +'%Y-%m-%d_%H-%M-%S')
sudo cp /home/lsd/db.sqlite /home/lsd/backups/db.rollback.$TS.sqlite

sudo cp -aL /home/lsd/backups/lsd.latest /home/lsd/lsd
sudo cp -aL /home/lsd/backups/db.latest.sqlite /home/lsd/db.sqlite

sudo systemctl start lsd
sleep 1
sudo systemctl is-active --quiet lsd
EOS
