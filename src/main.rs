use clap::Parser as ClapParser;
use env_logger::Env;
use log::{info, warn};
use std::{io::Error, sync::Arc};
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use url_crawler::{
    data_store::{DataStore, Store},
    fetch::{Fetch, HttpFetch},
    link::url_parts,
    task::run_task,
    task::Dependencies,
    url_frontier::URLFrontierBuilder,
};

#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// URL to crawl
    #[arg(short, long)]
    url: String,

    /// Number of worker threads
    #[arg(short, long, default_value_t = 1)]
    workers_n: u8,

    /// Politeness delay (in seconds) between requests
    #[arg(short, long, default_value_t = 2)]
    delay: u64,

    /// Print data store at the end of the crawl (boolean value)
    #[arg(short, long)]
    print: bool,
}

async fn execute(
    cli_args: Args,
    dependencies: Dependencies,
) -> Result<Arc<RwLock<dyn DataStore + Send>>, Error> {
    let Args { url, workers_n, .. } = cli_args;
    let Dependencies {
        url_frontier,
        data_store,
    } = dependencies;
    let original_url_parts = Arc::new(url_parts(&url));
    let mut tasks = JoinSet::new();

    let dependencies = Arc::new(Dependencies {
        url_frontier,
        data_store: data_store.clone(),
    });

    for _n in 0..workers_n {
        let client: HttpFetch = Fetch::new(); // a HTTP client per worker
        let is_initial_crawl = true; // if multi-threaded, threads won't quit after the initial single URL crawl

        let task = tokio::spawn(run_task(
            dependencies.clone(),
            client,
            original_url_parts.clone(),
            is_initial_crawl,
        ));

        tasks.spawn(task);
    }

    while let Some(_res) = tasks.join_next().await {
        info!("Worker completed");
    }

    Ok(data_store)
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    let cli_args = Args::parse();
    let should_print_results = cli_args.print;

    let url_frontier = Arc::new(RwLock::new(
        URLFrontierBuilder::new()
            .value(cli_args.url.to_owned())
            .delay_s(cli_args.delay)
            .build(),
    ));
    let data_store = Arc::new(RwLock::new(Store::new()));
    let dependencies = Dependencies {
        url_frontier,
        data_store,
    };

    match execute(cli_args, dependencies).await {
        Ok(val) => {
            info!("Done!");

            if should_print_results {
                let data_store_read = val.read().await;
                println!("{:?}", data_store_read);
            }
        }
        Err(e) => {
            warn!("There's been an error: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{execute, Args, Dependencies};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use url_crawler::{
        data_store::{DataStore, Store},
        url_frontier::URLFrontierBuilder,
    };
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    fn make_hrefs(base_uri: &str) -> Vec<String> {
        let url1 = format!("{}/about", &base_uri);
        let url2 = format!("{}/contact", &base_uri);
        let url3 = "http://google.com".to_owned();

        vec![url1, url2, url3]
    }

    fn make_anchors(urls: Vec<String>) -> String {
        urls.into_iter()
            .map(|url| format!("<a href=\"{}\"</a>", url))
            .collect::<Vec<_>>()
            .join("")
    }

    #[tokio::test]
    async fn execute_populates_data_store_as_expected() {
        //  --- arrange
        let mock_server = MockServer::start().await;
        let port = mock_server.address().port();
        let test_url = format!("http://localhost:{}", port);
        let hrefs = make_hrefs(&test_url);
        let anchors = make_anchors(hrefs.to_vec());

        let response = ResponseTemplate::new(200).set_body_string(anchors);
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(response.clone())
            .up_to_n_times(1)
            .expect(1..)
            .mount(&mock_server)
            .await;

        let response = ResponseTemplate::new(200).set_body_string("");
        Mock::given(method("GET"))
            .and(path("/about"))
            .respond_with(response.clone())
            .up_to_n_times(1)
            .expect(1..)
            .mount(&mock_server)
            .await;

        let response = ResponseTemplate::new(200).set_body_string("");
        Mock::given(method("GET"))
            .and(path("/contact"))
            .respond_with(response.clone())
            .up_to_n_times(1)
            .expect(1..)
            .mount(&mock_server)
            .await;

        let cli_args = Args {
            url: test_url.to_owned(),
            workers_n: 1,
            delay: 0,
            print: false,
        };

        let url_frontier = Arc::new(RwLock::new(
            URLFrontierBuilder::new()
                .value(cli_args.url.to_owned())
                .delay_s(cli_args.delay)
                .build(),
        ));
        let data_store = Arc::new(RwLock::new(Store::new()));

        let mut data_store_raw = Store::new();
        for url in make_hrefs(&test_url) {
            data_store_raw.add(test_url.to_owned(), Some(url.clone()));
            data_store_raw.visited(&test_url);
        }

        data_store_raw.add(hrefs[0].clone(), None);
        data_store_raw.visited(&hrefs[0]);

        data_store_raw.add(hrefs[1].clone(), None);
        data_store_raw.visited(&hrefs[1]);

        let expected_data_store = Arc::new(RwLock::new(data_store_raw));

        let dependencies = Dependencies {
            url_frontier,
            data_store: data_store.clone(),
        };

        // --- act
        let _ = execute(cli_args, dependencies).await;

        // --- assert
        let actual = data_store.read().await;
        let expected = expected_data_store.read().await;

        assert_eq!(*expected, *actual);
    }
}
