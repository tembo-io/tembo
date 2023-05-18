#!/bin/bash
file='extensions.txt'
lines=$(cat $file)
for line in $lines
do
	echo $line
	trunk install $line --pg-config /usr/lib/postgresql/15/bin/pg_config
	psql -c "create extension \"$line\";"
done
