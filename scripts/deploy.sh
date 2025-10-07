#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/deploy.sh <user>@<hostname>"
    exit 1
fi

rsync --rsync-path="sudo rsync" -Pavz target/aarch64-unknown-linux-gnu/release/lsd $1:/home/lsd/lsd.next

ssh $1 <<'EOS'
set -euxo pipefail
sudo systemctl stop lsd
sudo sqlite3 /home/lsd/db.sqlite "PRAGMA wal_checkpoint(TRUNCATE);"

TS=$(date +'%Y-%m-%d_%H-%M-%S')
sudo cp -a /home/lsd/lsd /home/lsd/backups/lsd.$TS
sudo cp -a /home/lsd/db.sqlite /home/lsd/backups/db.$TS.sqlite
sudo ln -sf /home/lsd/backups/lsd.$TS /home/lsd/backups/lsd.latest
sudo ln -sf /home/lsd/backups/db.$TS.sqlite /home/lsd/backups/db.latest.sqlite

sudo mv /home/lsd/lsd.next /home/lsd/lsd
sudo setcap 'cap_net_bind_service=+ep' /home/lsd/lsd

sudo systemctl start lsd
sleep 1
sudo systemctl is-active --quiet lsd
EOS
