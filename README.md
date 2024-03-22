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
Usage: solr-post -c <collection> [-h <host>] [-p <port>] -d <directory> -g <glob-pattern>

Post files to a solr collection

Options:
  -c, --collection  the solr collection to post to
  -h, --host        the host of the solr server defaults to localhost
  -p, --port        the port of the solr server defaults to 8983
  -d, --directory   the directory to search for files to post
  -g, --glob-pattern
                    the glob pattern to use to find files to post e.g.
                    "**/*.html"
  --help            display usage information
```
