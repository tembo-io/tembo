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

# Install /temback
curl -L https://github.com/tembo-io/temback/releases/download/v0.1.0/temback-v0.1.0-linux-amd64.tar.gz \
    | tar -C "${dst}" --strip-components=1 -zxf - temback-v0.1.0-linux-amd64/temback
