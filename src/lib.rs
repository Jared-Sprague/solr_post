use std::{
    collections::HashSet,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use base64::prelude::*;
use futures::StreamExt;
use log::info;
use mime_guess::from_path;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use regex::Regex;
use reqwest::{header, Client};
use wax::{Glob, WalkEntry, WalkError};

/// Configuration for posting files to Solr server
pub struct PostConfig {
    /// the number of concurrent requests to make to the solr server defaults to 8
    pub concurrency: usize,

    /// the host of the solr server defaults to localhost
    pub host: String,

    /// the port of the solr server defaults to 8983
    pub port: u16,

    /// the solr collection to post to collection1
    pub collection: String,

    /// the directory to search for files to post defaults to current directory
    pub directory_path: PathBuf,

    /// the file extensions to post defaults to xml,json,jsonl,csv,pdf,doc,docx,ppt,pptx,xls,xlsx,odt,odp,ods,ott,otp,ots,rtf,htm,html,txt,log
    pub file_extensions: Vec<String>,

    /// base Solr update URL this will override host, port, and collection e.g. "http://localhost:8983/solr/my_collection/update"
    pub update_url: Option<String>,

    /// exclude files who's content contains this regex pattern, this takes precedence over include_regex e.g. "no_index"
    pub exclued_regex: Option<Regex>,

    /// include only files who's content contains this regex pattern e.g. "index_me"
    pub include_regex: Option<Regex>,

    /// basic auth user credentials e.g. "user:pass"
    pub basic_auth_creds: Option<String>,
}

// defaults for PostConfig
impl Default for PostConfig {
    fn default() -> Self {
        PostConfig {
            concurrency: 8,
            host: String::from("localhost"),
            port: 8983,
            collection: String::from("collection1"),
            directory_path: PathBuf::from("./"),
            file_extensions: vec![
                String::from("xml"),
                String::from("json"),
                String::from("jsonl"),
                String::from("csv"),
                String::from("pdf"),
                String::from("doc"),
                String::from("docx"),
                String::from("ppt"),
                String::from("pptx"),
                String::from("xls"),
                String::from("xlsx"),
                String::from("odt"),
                String::from("odp"),
                String::from("ods"),
                String::from("ott"),
                String::from("otp"),
                String::from("ots"),
                String::from("rtf"),
                String::from("htm"),
                String::from("html"),
                String::from("txt"),
                String::from("log"),
            ],
            update_url: None,
            exclued_regex: None,
            include_regex: None,
            basic_auth_creds: None,
        }
    }
}

/// Post files to Solr server concurrently based on the configuration
/// optionally you can provide callbacks for on_start, on_next, and on_finish
/// on_start will be called with the total number of files to index
/// on_next will be called with the number of files indexed for tracking progress
/// on_finish will be called when the indexing is complete
/// returns the total number of files indexed
#[allow(clippy::redundant_clone)]
pub async fn solr_post(
    config: PostConfig,
    mut on_start: Option<Box<dyn FnMut(u64)>>,
    mut on_next: Option<Box<dyn FnMut(u64)>>,
    mut on_finish: Option<Box<dyn FnMut()>>,
) -> usize {
    let file_extensions_joined = config.file_extensions.join(",");
    let glob_expression = format!("**/*.{{{}}}", file_extensions_joined);
    let glob = Glob::new(glob_expression.as_str()).unwrap();
    let files: Vec<Result<WalkEntry, WalkError>> = glob.walk(config.directory_path).collect();
    let files_to_index_set: HashSet<String>;
    let mut default_headers = header::HeaderMap::new();

    // insert basic auth header if basic_auth_creds is set
    if let Some(creds) = &config.basic_auth_creds {
        // encode the username and password to base64
        let auth_value = BASE64_STANDARD.encode(creds);
        default_headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(&format!("Basic {}", auth_value)).unwrap(),
        );
    }

    // build the client with default_headers
    let client = Client::builder()
        .default_headers(default_headers)
        .build()
        .unwrap();

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

        // Scan for .html files that need indexing and store them in a vector
        files.par_iter().for_each(|file| match file {
            Ok(entry) => {
                let path = entry.path();
                let path_str = path.to_str().unwrap();

                // read the file content
                let mut file = File::open(path_str).unwrap();
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();

                // exclude and include rules. Note if exclude takes precedence over include

                if let Some(exclude_regex) = config.exclued_regex.as_ref() {
                    if exclude_regex.is_match(&contents) {
                        // this file should be excluded, skip it and continue to the next file
                        return;
                    }
                }

                if let Some(include_regex) = config.include_regex.as_ref() {
                    if !include_regex.is_match(&contents) {
                        // this file should not be included, skip it and continue to the next file
                        return;
                    }
                }

                let mut files_to_index_set = files_to_index.write().expect("rwlock poisoned");
                files_to_index_set.insert(path_str.to_string());
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

        // guess the mime type of the file from the file path e.g. "text/html"
        let mime_type = from_path(&file_path_absolute).first_or_octet_stream();

        // post the file to solr using the Apache Tika update/extract handler
        (
            client
                .post(solr_post_url)
                .header(header::CONTENT_TYPE, mime_type.to_string())
                .body(contents)
                .send()
                .await,
            file_path_absolute,
        )
    }))
    .buffer_unordered(config.concurrency);

    info!("indexing {} files", total_files_to_index);
    let mut indexed_count = 0;

    if let Some(ref mut on_start) = on_start {
        // call the start callback with the total_files_to_index
        on_start(total_files_to_index as u64);
    }

    // loop through the stream of futures solr POST requests and increment the progress bar
    while let Some((res, file_path)) = posts.next().await {
        match res {
            Ok(response) => {
                if response.status().is_success() {
                    info!("indexed: {}", file_path.to_str().unwrap());
                } else {
                    eprintln!(
                        "POST {} {}\nIs collection correct?\nfailed to index file: {}",
                        response.url(),
                        response.status(),
                        file_path.to_str().unwrap(),
                    );
                }

                indexed_count += 1;

                if let Some(ref mut on_next) = on_next {
                    // call the progress callback with the indexed_count
                    on_next(indexed_count as u64);
                }
            }
            Err(e) => {
                eprintln!("{}\nIs Solr server running and collection available?", e)
            }
        }
    }

    // send GET request to solr to commit the changes
    let response = client
        .get("http://localhost:8983/solr/portal/update?commit=true")
        .send()
        .await;

    // check if the commit was successful
    match response {
        Ok(response) => {
            if response.status().is_success() {
                info!("commit successful");
            } else {
                info!("commit failed");
            }
        }
        Err(e) => {
            eprintln!("{}\nIs Solr server running and collection available?", e);
        }
    }

    // output time
    info!("indexing complete");

    if let Some(ref mut on_finish) = on_finish {
        // call the finish callback
        on_finish();
    }

    total_files_to_index
}
