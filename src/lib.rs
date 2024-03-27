use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use futures::StreamExt;
use log::info;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use regex::Regex;
use wax::{Glob, WalkEntry, WalkError};

/// trait for implementing a callback when the indexing starts
/// the value is the total number of files to index
pub trait StartCallback {
    fn on_start(&mut self, value: u64);
}

/// trait for implementing a callback when the indexing progresses
/// the value is the current progress
pub trait NextCallback {
    fn on_next(&mut self, value: u64);
}

/// trait for implementing a callback when the indexing finishes
/// this is useful for any tasks that need to be done after the indexing is complete
pub trait FinishCallback {
    fn on_finish(&mut self);
}

pub struct PostConfig<S: StartCallback, N: NextCallback, F: FinishCallback> {
    pub concurrency: usize,
    pub host: String,
    pub port: u16,
    pub collection: String,
    pub directory_path: PathBuf,
    pub file_extensions: Vec<String>,
    pub update_url: Option<String>,
    pub on_start: Option<S>,
    pub on_next: Option<N>,
    pub on_finish: Option<F>,
}

#[allow(clippy::redundant_clone)]
pub async fn solr_post<S: StartCallback, N: NextCallback + std::marker::Copy, F: FinishCallback>(
    config: PostConfig<S, N, F>,
) -> usize {
    let file_extensions_joined = config.file_extensions.join(",");
    let glob_expression = format!("**/*.{{{}}}", file_extensions_joined);
    let glob = Glob::new(glob_expression.as_str()).unwrap();
    let files: Vec<Result<WalkEntry, WalkError>> = glob.walk(config.directory_path).collect();
    let files_to_index_set: HashSet<String>;
    let client = reqwest::Client::new();

    // build the solr post url from the config. If the update_url is set, use that, otherwise build the url
    let solr_collection_update_endpoint = match &config.update_url {
        Some(url) => url.clone(),
        None => format!(
            "http://{0}:{1}/solr/{2}/update/extract",
            config.host, config.port, config.collection
        ),
    };

    // scope for the MutexGuard accross async/await
    // see: https://rust-lang.github.io/rust-clippy/master/index.html#await_holding_lock
    {
        // files to index
        let files_to_index = Arc::new(RwLock::new(HashSet::<String>::new()));

        // this clone is just so the main thread can hold onto a reference, to then print out later
        let files_to_index_ref = files_to_index.clone();

        // use regex to find the string "mimir_solr_noindex" in the file
        let noindex_re = Regex::new(r"solr_noindex").unwrap();

        // Scan for .html files that need indexing and store them in a vector
        files.par_iter().for_each(|file| match file {
            Ok(entry) => {
                let path = entry.path();
                let path_str = path.to_str().unwrap();

                // read the file content
                let mut file = File::open(path_str).unwrap();
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();

                if !noindex_re.is_match(&contents) {
                    let mut files_to_index_set = files_to_index.write().expect("rwlock poisoned");
                    files_to_index_set.insert(path_str.to_string());
                }
            }
            Err(e) => println!("error: {:?}", e),
        });

        let rw_lock_files_set = files_to_index_ref.read().expect("rwlock poisoned");
        files_to_index_set = rw_lock_files_set.clone();
    } // MutexGuard is dropped here

    let total_files_to_index = files_to_index_set.len();

    let mut posts = futures::stream::iter(files_to_index_set.into_iter().map(|file| async {
        // get the absolute path of file
        let file_path = Path::new(&file);
        let file_path_absolute = file_path.canonicalize().unwrap();

        // url encode the file path string
        let file_path_encoded = urlencoding::encode(file_path_absolute.to_str().unwrap());

        // read the file into a String
        let mut file = File::open(file).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        // format the solr post url using file_path_encoded as the resource.name & literal.id
        let solr_post_url = format!(
            "{0}?resource.name={1}&literal.id={1}",
            solr_collection_update_endpoint, file_path_encoded
        );

        // use reqwest::Client to post the file to solr using the Apache Tika update/extract handler
        client
            .post(solr_post_url)
            .header(reqwest::header::CONTENT_TYPE, "text/html")
            .body(contents)
            .send()
            .await
    }))
    .buffer_unordered(config.concurrency);

    info!("indexing {} files", total_files_to_index);
    let mut indexed_count = 0;

    if let Some(mut on_start) = config.on_start {
        // call the on_start callback with the total number of files to index
        on_start.on_start(total_files_to_index as u64);
    }

    // loop through the stream of futures solr POST requests and increment the progress bar
    while let Some(res) = posts.next().await {
        match res {
            Ok(_) => {
                indexed_count += 1;

                if let Some(mut on_next) = config.on_next {
                    // call the on_next callback with the current progress
                    on_next.on_next(indexed_count as u64);
                }
            }
            Err(e) => {
                eprintln!("{}", e)
            }
        }
    }

    // send GET request to solr to commit the changes
    client
        .get("http://localhost:8983/solr/portal/update?commit=true")
        .send()
        .await
        .unwrap();

    // output time
    info!("indexing complete");

    if let Some(mut on_finish) = config.on_finish {
        // call the finish callback
        on_finish.on_finish();
    }

    total_files_to_index
}
