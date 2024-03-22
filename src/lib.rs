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

const CONNCURENCY: usize = 8;

pub struct PostConfig {
    pub collection: String,
    pub directory_path: PathBuf,
    pub glob_pattern: String,
}

#[allow(clippy::redundant_clone)]
pub async fn solr_index(
    config: PostConfig,
    mut on_start: impl FnMut(u64),
    mut on_next: impl FnMut(u64),
    mut on_finish: impl FnMut(),
) -> usize {
    // let directory_path = "../mimir-cli/components/load/zola/zola-project/public/";
    // let directory_path = "public_small/";

    // TODO: make the glob a parameter
    // Glob .html files
    let glob = Glob::new(config.glob_pattern.as_str()).unwrap();
    let files: Vec<Result<WalkEntry, WalkError>> = glob.walk(config.directory_path).collect();
    let files_to_index_set: HashSet<String>;
    let client = reqwest::Client::new();

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
            "http://localhost:8983/solr/portal/update/extract?resource.name={0}&literal.id={0}",
            file_path_encoded
        );

        // use reqwest::Client to post the file to solr using the Apache Tika update/extract handler
        client
            .post(solr_post_url)
            .header(reqwest::header::CONTENT_TYPE, "text/html")
            .body(contents)
            .send()
            .await
    }))
    .buffer_unordered(CONNCURENCY);

    info!("indexing {} files", total_files_to_index);
    let mut indexed_count = 0;

    on_start(total_files_to_index as u64);

    // loop through the stream of futures solr POST requests and increment the progress bar
    while let Some(res) = posts.next().await {
        match res {
            Ok(_) => {
                indexed_count += 1;
                // call the progress callback
                on_next(indexed_count);
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

    // call the finish callback
    on_finish();

    total_files_to_index
}
