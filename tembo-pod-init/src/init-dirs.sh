#!/bin/sh

cp -p --recursive --update /var/lib/postgresql/data/* /var/lib/postgresql/init/
rm --recursive --force /var/lib/postgresql/init/lost+found
