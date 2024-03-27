# solr_post

This is a simple CLI and library for posting files in a directory to a Solr collection. It is ment as a faster Rust based alternative to the [Solr Post Tool](https://solr.apache.org/guide/8_5/post-tool.html)

## Library usage

The library provides a function called `solr_post()` which you pass a `PostConfig` struct as well as progress callback functions for monitoring or logging the progress.

# CLI usage

There is also an included binary that you can use on the command line by running `cargo install solr_post`

Example usage:

```
solr-post -c my_collection -d /var/www/html -g **/*.html
```

Current options:

```
Usage: solr-post -c <collection> [-h <host>] [-p <port>] [--url <url>] -d <directory> [-f <file-extensions>] [--concurrency <concurrency>] [-e <exclude-regex>] [-i <include-regex>]

Post files to a solr collection

Options:
  -c, --collection  the solr collection to post to
  -h, --host        the host of the solr server defaults to localhost
  -p, --port        the port of the solr server defaults to 8983
  --url             base Solr update URL e.g.
                    http://localhost:8983/solr/my_collection/update if this is
                    set, the collection, host, and port are ignored
  -d, --directory   the directory to search for files to post
  -f, --file-extensions
                    the file extensions to post defaults to
                    xml,json,jsonl,csv,pdf,doc,docx,ppt,pptx,xls,xlsx,odt,odp,ods,ott,otp,ots,rtf,htm,html,txt,log
                    e.g. "html,txt,json"
  --concurrency     concurrency level defauls to 8 the number of concurrent
                    requests to make to the solr server
  -e, --exclude-regex
                    exclude files who's content contains this regex pattern e.g.
                    "no_index". only files files who's content does not contains
                    this pattern will be indexed. this is case insensitive. if
                    both exclude_regex and include_regex are set, exclude_regex
                    will takes precedence.
  -i, --include-regex
                    include only files who's content contains this regex pattern
                    e.g. "index_me". only files files who's content contains
                    this pattern will be indexed. this is case insensitive. if
                    both exclude_regex and include_regex are set, exclude_regex
                    will takes precedence.
  --help            display usage information
```
