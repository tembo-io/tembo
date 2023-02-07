#!/bin/bash
#
# Build a .deb for the Postgres and Ubuntu version.
#

PACKAGE_VERSION=${PACKAGE_VERSION:-0.0.0}
OUTPUT_DIR=${OUTPUT_DIR:-"."}
NAME=${NAME:-"coredb-extension"}
PACKAGE_NAME=${PACKAGE_NAME:-"coredb-extension"}

if [[ $(uname -a) == *"aarch64"* ]]; then
    ARCH="arm64"
else
    ARCH="amd64"
fi

PGVERSION=$(pg_config | grep "VERSION")

if [[ $PGVERSION == *"15."* ]]; then
    PGVERSION="15"
elif [[ $PGVERSION == *"14."* ]]; then
    PGVERSION="14"
elif [[ $PGVERSION == *"13."* ]]; then
    PGVERSION="13"
else
    echo "Unknown PostgreSQL version detected: ${PGVERSION}"
    exit 1
fi

TARGET="target/release/${PACKAGE_NAME}-pg${PGVERSION}"
UBUNTU_VERSION=$(lsb_release -a | grep Release | awk '{ print $2 }')

ls -R ${TARGET}

mkdir -p ${TARGET}/DEBIAN

cat <<EOF > ${TARGET}/DEBIAN/control
Package: postgresql-${NAME}-${PGVERSION}
Version: ${PACKAGE_VERSION}
Section: database
Priority: optional
Architecture: ${ARCH}
Depends: postgresql-${PGVERSION}, postgresql-server-dev-${PGVERSION}
Maintainer: CoreDB <admin+${NAME}@coredb.io>
Homepage: https://coredb.io
Description: The extension is written in Rust using tcdi/pgx
EOF

cat ${TARGET}/DEBIAN/control

PACKAGE=postgresql-${NAME}-${PGVERSION}_${PACKAGE_VERSION}-ubuntu${UBUNTU_VERSION}-${ARCH}.deb

# Build the debian package
dpkg-deb --build ${TARGET} $OUTPUT_DIR/${PACKAGE}
