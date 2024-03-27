use argh::FromArgs;
use regex::Regex;
use solr_post::{solr_post, PostConfig};
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};

#[derive(FromArgs)]
/// Post files to a solr collection
struct SolrPostArgs {
    /// the solr collection to post to
    #[argh(option, short = 'c')]
    collection: String,

    /// the host of the solr server defaults to localhost
    #[argh(option, short = 'h', default = "String::from(\"localhost\")")]
    host: String,

    /// the port of the solr server defaults to 8983
    #[argh(option, short = 'p', default = "8983")]
    port: u16,

    /// base Solr update URL
    /// e.g. http://localhost:8983/solr/my_collection/update
    /// if this is set, the collection, host, and port are ignored
    #[argh(option)]
    url: Option<String>,

    /// basic auth user credentials
    /// e.g. "username:password"
    #[argh(option, short = 'u')]
    user: Option<String>,

    /// the directory to search for files to post
    #[argh(option, short = 'd')]
    directory: String,

    /// the file extensions to post defaults to xml,json,jsonl,csv,pdf,doc,docx,ppt,pptx,xls,xlsx,odt,odp,ods,ott,otp,ots,rtf,htm,html,txt,log
    /// e.g. "html,txt,json"
    #[argh(
        option,
        short = 'f',
        default = "String::from(\"xml,json,jsonl,csv,pdf,doc,docx,ppt,pptx,xls,xlsx,odt,odp,ods,ott,otp,ots,rtf,htm,html,txt,log\")"
    )]
    file_extensions: String,

    /// concurrency level defauls to 8
    /// the number of concurrent requests to make to the solr server
    #[argh(option, default = "8")]
    concurrency: usize,

    /// exclude files who's content contains this regex pattern
    /// e.g. "no_index".
    /// only files files who's content does not contains this pattern will be indexed.
    /// this is case insensitive.
    /// if both exclude_regex and include_regex are set, exclude_regex will takes precedence.
    #[argh(option, short = 'e')]
    exclude_regex: Option<String>,

    /// include only files who's content contains this regex pattern
    /// e.g. "index_me".
    /// only files files who's content contains this pattern will be indexed.
    /// this is case insensitive.
    /// if both exclude_regex and include_regex are set, exclude_regex will takes precedence.
    #[argh(option, short = 'i')]
    include_regex: Option<String>,
}

// implement into for SOlrPostArgs to convert it to PostConfig
impl From<SolrPostArgs> for PostConfig {
    fn from(val: SolrPostArgs) -> Self {
        PostConfig {
            collection: val.collection,
            host: val.host,
            port: val.port,
            directory_path: val.directory.into(),
            file_extensions: val
                .file_extensions
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            update_url: val.url,
            concurrency: val.concurrency,

            // create regex objects from the exclude and include regex strings ignore case
            exclued_regex: val
                .exclude_regex
                .map(|s| Regex::new(&format!("(?i){}", s)).unwrap()),
            include_regex: val
                .include_regex
                .map(|s| Regex::new(&format!("(?i){}", s)).unwrap()),

            basic_auth_creds: val.user,
        }
    }
}

#[tokio::main]
async fn main() {
    let args: SolrPostArgs = argh::from_env();

    // make sure that total_files_to_index lives for the entire duration of the program
    // Make total_files_to_index 'static' to ensure it lives for the entire program duration
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
            "Start indexing {} files with concurrency {}",
            total_files_to_index, args.concurrency
        );
    };

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

    solr_post(
        args.into(),
        Some(Box::new(on_start)),
        Some(Box::new(on_next)),
        Some(Box::new(on_finish)),
    )
    .await;
}
