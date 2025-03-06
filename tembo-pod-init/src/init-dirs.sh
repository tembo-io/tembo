#!/bin/sh

# Only copy sharedir if it is empty.
sharedir="$(pg_config --sharedir)"
mkdir -p "${sharedir}"
if [ -d /tmp/pg_sharedir ] && [ -z "$(ls -A "${sharedir}")" ]; then
    cp -Rp /tmp/pg_sharedir/. "${sharedir}";
fi

# Always copy the contents of pkglibdir
pkglibdir="$(pg_config --pkglibdir)"
mkdir -p "${pkglibdir}"
if [ -d /tmp/pg_pkglibdir ]; then
    cp -Rp /tmp/pg_pkglibdir/. "${pkglibdir}";
fi

if [ -d /var/lib/postgresql/data/lost+found ]; then
    rmdir /var/lib/postgresql/data/lost+found;
fi
