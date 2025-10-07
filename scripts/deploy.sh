#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/deploy.sh <user>@<hostname>"
    exit 1
fi

rsync --rsync-path="sudo rsync" -Pavz target/aarch64-unknown-linux-gnu/release/lsd $1:/home/lsd/lsd.next

ssh $1 <<'EOS'
sudo systemctl stop lsd
sudo sqlite3 /home/lsd/db.sqlite "SELECT COUNT(*) FROM sqlite_master;" # flush the WAL

TS=$(date +'%Y-%m-%d_%H-%M-%S')
sudo cp -av /home/lsd/lsd /home/lsd/backups/lsd.$TS
sudo cp -av /home/lsd/db.sqlite /home/lsd/backups/db.$TS.sqlite
sudo ln -sfv /home/lsd/backups/lsd.$TS /home/lsd/backups/lsd.latest
sudo ln -sfv /home/lsd/backups/db.$TS.sqlite /home/lsd/backups/db.latest.sqlite

mv -v /home/lsd/lsd.next /home/lsd/lsd
sudo setcap 'cap_net_bind_service=+ep' /home/lsd/lsd

sudo systemctl start lsd
sudo systemctl is-active --quiet lsd
EOS
