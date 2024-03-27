# solr_post

This is a simple library and CLI for posting files in a directory to a Solr collection to be indexed. It is ment as a much faster (up to 10x) Rust based alternative to the java based [Solr Post Tool](https://solr.apache.org/guide/8_5/post-tool.html) that is included by default with Solr. It also includes additional features that are not included with the default Solr Post Tool, such as the ability
to filter files with include/exclude regex patterns.

## Library

The library provides a function called `solr_post()` which you pass a `PostConfig` struct as well as progress callback functions for monitoring or logging the progress.

### Basic Example

```
use solr_post::{PostConfig, solr_post};
std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Configure
    let config = PostConfig {
        host: String::from("localhost"),
        port: 8983,
        collection: String::from("my_collection"),
        directory_path: PathBuf::from("/var/www/html"),
        ..Default::default()
    };

    // Make the Solr post request
    solr_post(config, None, None, None).await;
}
```

In this example we will index files located in /var/www/html recursively to collection "my_collection" on the Solr server running at localhost:8983.

### Example using progress callbacks

```
use solr_post::{solr_post, PostConfig};
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};

#[tokio::main]
async fn main() {
    static TOTAL_FILES_TO_INDEX: OnceLock<Mutex<u64>> = OnceLock::new();

    TOTAL_FILES_TO_INDEX.get_or_init(|| Mutex::new(0u64));

    let on_start = move |total_files: u64| {
        // Retrieve the total_files_to_index from the static variable
        let total_files_to_index = TOTAL_FILES_TO_INDEX.get().unwrap();

        // Lock the mutex to update the value
        let mut total_files_to_index = total_files_to_index.lock().unwrap();

        // Initialize the total_files_to_index to total_files
        *total_files_to_index = total_files;

        println!(
            "Start indexing {} files",
            total_files_to_index
        );
    };

    // log the progress as a percent complete
    let on_next = |indexed_count: u64| {
        let total_files_to_index = TOTAL_FILES_TO_INDEX.get().unwrap();

        let total_files_to_index = total_files_to_index.lock().unwrap();

        // get the percent complete as a float
        let percetn_complete = (indexed_count as f64 / *total_files_to_index as f64) * 100.0;

        // print the precent complete to presicion of 2 decimal places
        print!(
            "{}/{} indexed {:.2}%\r",
            indexed_count, *total_files_to_index, percetn_complete
        );
        io::stdout().flush().unwrap(); // Flush the output buffer
    };

    let on_finish = || {
        println!("\nFinished indexing.");
    };

    // Configure
    let config = PostConfig {
        host: String::from("localhost"),
        port: 8983,
        collection: String::from("my_collection"),
        directory_path: std::path::PathBuf::from("/var/www/html"),
        file_extensions: vec![
            String::from("html"),
            String::from("txt"),
            String::from("pdf"),
        ],
        ..Default::default()
    };

    solr_post(
        config,
        Some(Box::new(on_start)),
        Some(Box::new(on_next)),
        Some(Box::new(on_finish)),
    )
    .await;
}
```

In this example we will index only html, txt, and pdf files located in /var/www/html recursively to collection "my_collection" on the Solr server running at localhost:8983. we also will provide callbacks to output progress messages. Running this will result in output that looks like:

```
Start indexing 152 files
152/152 indexed 100.00%
Finished indexing.
```

# CLI usage

There is also an included binary that you can use on the command line by running `cargo install solr_post`

```
Usage: solr-post -c <collection> [-h <host>] [-p <port>] [--url <url>] [-u <user>] -d <directory> [-f <file-extensions>] [--concurrency <concurrency>] [-e <exclude-regex>] [-i <include-regex>]

Post files to a solr collection

Options:
  -c, --collection  the solr collection to post to
  -h, --host        the host of the solr server defaults to localhost
  -p, --port        the port of the solr server defaults to 8983
  --url             base Solr update URL e.g.
                    http://localhost:8983/solr/my_collection/update if this is
                    set, the collection, host, and port are ignored
  -u, --user        basic auth user credentials e.g. "username:password"
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

## Example

```
solr-post -c my_collection -d /var/www/html -f html,txt,pdf
```
