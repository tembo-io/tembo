#!/bin/bash

set -e

src=/var/lib/postgresql/data
dst=/var/lib/postgresql/init

if [ -x /tmp/sync-volume.sh ]; then
    /tmp/sync-volume.sh "$dst"
else
    cp -p --recursive --update "$src/"* "$dst/"
    rm --recursive --force "$dst/lost+found"
fi
