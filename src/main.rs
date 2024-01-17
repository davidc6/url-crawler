use clap::Parser as ClapParser;
use env_logger::Env;
use log::{info, warn};
use std::{io::Error, sync::Arc};
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use url_crawler::{
    data_store::{DataStore, Store},
    fetch::{Fetch, HttpFetch},
    link::{filter_url, process_url, url_parts},
    parser::Parser,
    url_frontier::{URLFrontier, URLFrontierBuilder, URLFrontierable},
};

struct Dependencies {
    url_frontier: Arc<RwLock<URLFrontier>>,
    data_store: Arc<RwLock<Store>>,
}

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

async fn execute(cli_args: Args, dependencies: Dependencies) -> Result<Arc<RwLock<Store>>, Error> {
    let Args { url, workers_n, .. } = cli_args;
    let Dependencies {
        url_frontier,
        data_store,
    } = dependencies;

    let original_url_parts = Arc::new(url_parts(&url));
    let mut tasks = JoinSet::new();

    for _ in 0..workers_n {
        let url_frontier = url_frontier.clone();
        let data_store = data_store.clone();
        let urls_parts = original_url_parts.clone();

        let client: HttpFetch = Fetch::new(); // a HTTP client per worker
        let mut is_initial_crawl = true; // if multi-threaded, threads won't quit after the initial single URL crawl

        let task = tokio::spawn(async move {
            loop {
                let mut url_frontier_write = url_frontier.write().await;
                let url = url_frontier_write.dequeue().await;
                let current_url = match url {
                    Some(val) => val,
                    None => return,
                };

                let mut data_store_write = data_store.write().await;
                if data_store_write.has_visited(&current_url) {
                    continue;
                }

                info!("Visiting URL: {}", current_url);

                let content = match client.get(&current_url).await {
                    Ok(val) => val,
                    Err(e) => {
                        warn!("Error requesting URL {} - {}", current_url, e);
                        continue;
                    }
                };

                data_store_write.add(current_url.clone(), None);
                data_store_write.visited(&current_url);

                let urls_found = Parser::new(content).all_links();
                for url in urls_found {
                    let url = process_url(url, &current_url);
                    info!("Found URL: {}", url);

                    data_store_write.add(current_url.clone(), Some(url.clone()));

                    if let Some(url) = filter_url(url, urls_parts.clone()) {
                        if !data_store_write.has_visited(&url) {
                            url_frontier_write.enqueue(url);
                        }
                    };
                }

                info!("--------------------------------------------");

                if is_initial_crawl {
                    is_initial_crawl = false;
                    continue;
                }
            }
        });

        tasks.spawn(task);
    }

    while let Some(_res) = tasks.join_next().await {
        info!("Worker completed");
    }

    Ok(data_store)
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cli_args = Args::parse();
    let should_print_results = cli_args.print;

    let url_frontier = Arc::new(RwLock::new(
        URLFrontierBuilder::new()
            .value(cli_args.url.to_owned())
            .delay_s(cli_args.delay)
            .build(),
    ));
    let data_store = Arc::new(RwLock::new(DataStore::new()));
    let dependencies = Dependencies {
        url_frontier,
        data_store,
    };

    match execute(cli_args, dependencies).await {
        Ok(val) => {
            info!("Done!");

            if should_print_results {
                let data_store_read = val.read().await;
                println!("{:?}", *data_store_read);
            }
        }
        Err(e) => {
            warn!("There's been an error: {}", e);
        }
    };
}

#[cfg(test)]
mod tests {
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

    use crate::{execute, Args, Dependencies};

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

    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    async fn test_execute() {
        //  --- arrange
        let mock_server = MockServer::start().await;
        let mock_server_uri = mock_server.uri();
        let hrefs = make_hrefs(&mock_server_uri);
        let anchors = make_anchors(hrefs.to_vec());

        // mock http requests
        let response = ResponseTemplate::new(200).set_body_string(anchors);
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(response)
            .mount(&mock_server)
            .await;

        let cli_args = Args {
            url: mock_server.uri().to_owned(),
            workers_n: 1,
            delay: 0,
            print: false,
        };

        // dependencies
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

        // --- act
        let actual = execute(cli_args, dependencies).await;
        let data_store = actual.unwrap();
        let actual = data_store.read().await;

        // --- assert
        let mut expected = Store::new();
        for url in make_hrefs(&mock_server_uri) {
            expected.add(mock_server_uri.to_owned(), Some(url.clone()));
            expected.visited(&mock_server_uri);
        }

        expected.add(hrefs[0].clone(), None);
        expected.visited(&hrefs[0]);

        expected.add(hrefs[1].clone(), None);
        expected.visited(&hrefs[1]);

        assert_eq!(expected, *actual);
    }
}
