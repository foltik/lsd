#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/rollback.sh <user>@<hostname>"
    exit 1
fi

#ssh $1 <<'EOS'
ssh -i ~/.ssh/id_ed25519_lsd_root ec2-user@beta.lightandsound.design <<'EOS'
sudo systemctl stop lsd
sudo sqlite3 /home/lsd/db.sqlite "SELECT COUNT(*) FROM sqlite_master;" # flush the WAL
sudo cp -aLv /home/lsd/backups/lsd.latest /home/lsd/lsd
sudo cp -aLv /home/lsd/backups/db.latest.sqlite /home/lsd/db.sqlite
sudo setcap 'cap_net_bind_service=+ep' /home/lsd/lsd
sudo systemctl start lsd
EOS
