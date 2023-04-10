#!/bin/bash
#
# This script is instead of 'trunk install'
for extension in $(ls /extensions); do
  for file in $(tar -tvf /extensions/${extension} \
		| rev | cut -d" " -f1 | rev | grep -E 'sql|so|control' | cut -d"/" -f2); do
		echo "Installing ${file}...";
		if echo ${file} | grep "\.so"; then
			echo "Installing $(pg_config --pkglibdir)/${file}";
			tar -axf /extensions/${extension} trunk-output/${file} -O > $(pg_config --pkglibdir)/${file}
		else
			echo "Installing $(pg_config --sharedir)/extension/${file}";
			tar -axf /extensions/${extension} trunk-output/${file} -O > $(pg_config --sharedir)/extension/${file}
		fi
  done;
done;
