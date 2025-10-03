#!/bin/bash
set -euxo pipefail
if [ $# -ne 1 ]; then
    echo "Usage: scripts/deploy.sh <user>@<hostname>"
    exit 1
fi

rsync --rsync-path="sudo rsync" -Pavz target/aarch64-unknown-linux-gnu/release/lsd $1:/home/lsd/
ssh $1 <<'EOS'
sudo setcap 'cap_net_bind_service=+ep' /home/lsd/lsd
sudo systemctl restart lsd
EOS
