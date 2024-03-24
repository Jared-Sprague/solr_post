use argh::FromArgs;
use solr_post::{solr_post, PostConfig};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

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
        }
    }
}

#[tokio::main]
async fn main() {
    let args: SolrPostArgs = argh::from_env();
    let total_files_to_index = Arc::new(Mutex::new(0));

    let on_start = |total_files: u64| {
        let total_files_to_index = Arc::clone(&total_files_to_index);

        let mut total_files_to_index = total_files_to_index.lock().unwrap();

        // initialize the total_files_to_index to totla_files
        *total_files_to_index = total_files;

        // total_files_to_index = total_files;
        println!("Start indexing {} files", total_files_to_index);
    };

    let on_next = |indexed_count| {
        let total_files_to_index = Arc::clone(&total_files_to_index);

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

    solr_post(args.into(), on_start, on_next, on_finish).await;
}
