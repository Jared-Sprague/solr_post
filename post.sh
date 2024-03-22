#!/bin/bash

directory=/home/jsprague/projects/mimir-cli/components/load/zola/zola-project/public
# directory=/home/jsprague/temp/public_small

# Count the number of *.html files locaed in $directory
total_count=$(find $directory -type f -name "*.html" | wc -l)
count=0

echo "Indexing $total_count files"

# Iterate over all the *.html files in $directory recursively and echo the filename
find $directory -type f -name "*.html" | while read filename; do
    # send all stdout and stderr to /dev/null
    /home/jsprague/projects/mimir-cli/components/load/solr/.solr-dist/bin/post -c portal $filename >/dev/null 2>&1

    # increment the count
    count=$((count + 1))

    # print on a single line the count/total_count and the percent complete
    echo -ne "($count/$total_count) $(bc <<<"scale=2; $count / $total_count * 100")%\r"
done
