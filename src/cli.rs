use solr_post::solr_index;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
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
    };

    let on_finish = || {
        println!("\nFinished indexing.");
    };

    solr_index(on_start, on_next, on_finish).await;
}
