#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/deploy.sh <user>@<hostname>"
    exit 1
fi

rsync --rsync-path="sudo rsync" -Pavz target/aarch64-unknown-linux-gnu/release/lsd $1:/home/lsd/lsd.next
rsync --rsync-path="sudo rsync" -Pavz --delete frontend/static/ $1:/home/lsd/static/

ssh $1 <<'EOS'
set -euxo pipefail
sudo systemctl stop lsd
sudo sqlite3 /home/lsd/db.sqlite "PRAGMA wal_checkpoint(TRUNCATE);"

sudo cp -a /home/lsd/lsd /home/lsd/backups/lsd.bak
sudo cp -a /home/lsd/db.sqlite /home/lsd/backups/db.bak.sqlite

sudo mv /home/lsd/lsd.next /home/lsd/lsd
sudo setcap 'cap_net_bind_service=+ep' /home/lsd/lsd

sudo systemctl start lsd
sleep 1
sudo systemctl is-active --quiet lsd
EOS
