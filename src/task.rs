use crate::{
    data_store::DataStore,
    fetch::Fetch,
    fetch::HttpFetch,
    link::{self, filter_url, process_url, UrlParts},
    parser::Parser,
    url_frontier::Queue,
};
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Dependencies {
    pub url_frontier: Arc<RwLock<dyn Queue + Send + Sync>>,
    pub data_store: Arc<RwLock<dyn DataStore + Send + Sync>>,
}

pub async fn run_task(
    deps: Arc<Dependencies>,
    client: HttpFetch,
    url_parts: Arc<Result<UrlParts, link::Error>>,
    mut is_initial_crawl: bool,
) {
    let Dependencies {
        url_frontier,
        data_store,
    } = &*deps;

    loop {
        let mut url_frontier_write = url_frontier.write().await;
        let url = url_frontier_write.dequeue().await;
        let current_url = match url {
            Some(val) => val,
            None => return,
        };

        let data_store_read = data_store.read().await;
        if data_store_read.has_visited(&current_url) {
            continue;
        }
        drop(data_store_read);

        let content = match client.get(&current_url).await {
            Ok(val) => val,
            Err(e) => {
                warn!("Error requesting URL {} - {}", current_url, e);
                continue;
            }
        };

        let mut data_store_write = data_store.write().await;
        data_store_write.add(current_url.clone(), None);
        data_store_write.visited(&current_url);

        let urls_found = Parser::new(content).all_links();

        info!("Visited URL: {}", current_url);
        for url in urls_found {
            let url = process_url(url, &current_url);
            info!("Found URL: {}", url);

            data_store_write.add(current_url.clone(), Some(url.clone()));

            if let Some(url) = filter_url(url, url_parts.clone()) {
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
}

#[cfg(test)]
mod task_tests {
    use crate::data_store::{DataStore, DataStoreEntry};
    use crate::fetch::{Fetch, HttpFetch};
    use crate::link::url_parts;
    use crate::task::{run_task, Dependencies};
    use crate::url_frontier::{Dequeue, Enqueue, Queue};
    use async_trait::async_trait;
    use mockall::{mock, predicate, Sequence};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    mock!(
        #[derive(Debug)]
        Store {}

        impl DataStore for Store {
            fn add(&mut self, key: String, value: Option<String>);
            fn visited(&mut self, key: &str);
            fn has_visited(&self, key: &str) -> bool;
            fn exists(&self, key: &str) -> bool;
            fn get<'a>(&'a self, key: &str) -> Option<&'a DataStoreEntry>;
        }
    );

    mock!(
        URLFrontier {}

        impl Queue for URLFrontier {}

        impl Enqueue for URLFrontier {
            fn enqueue(&mut self, value: String);
        }

        #[async_trait]
        impl Dequeue for URLFrontier {
            async fn dequeue(&mut self) -> Option<String>;
        }
    );

    #[tokio::test]
    async fn task_runs_correctly() {
        let listener = std::net::TcpListener::bind("localhost:45367").unwrap();
        let mock_server = MockServer::builder().listener(listener).start().await;

        let port = "45367";
        let main_url = format!("http://localhost:{}", port);
        let hrefs = make_hrefs(&main_url);
        let about_url = hrefs.first().unwrap();
        let contact_url = hrefs.get(1).unwrap();
        let anchors = make_anchors(hrefs.to_vec());

        // First request (/)
        let response = ResponseTemplate::new(200).set_body_string(anchors);
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(response.clone())
            .up_to_n_times(1)
            .expect(1..)
            .mount(&mock_server)
            .await;

        // Second request (/about)
        let response = ResponseTemplate::new(200).set_body_string("".to_owned());
        Mock::given(method("GET"))
            .and(path("/about"))
            .respond_with(response.clone())
            .up_to_n_times(1)
            .expect(1..)
            .mount(&mock_server)
            .await;

        // Third request (/contact)
        let response = ResponseTemplate::new(200).set_body_string("".to_owned());
        Mock::given(method("GET"))
            .and(path("/contact"))
            .respond_with(response.clone())
            .up_to_n_times(1)
            .expect(1..)
            .mount(&mock_server)
            .await;

        let mut url_frontier_mock = MockURLFrontier::new();
        let mut data_store_mock = MockStore::new();
        let mut sequence = Sequence::new();

        // /
        url_frontier_mock
            .expect_dequeue()
            .times(1)
            .return_const(Some(main_url.clone()))
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_has_visited()
            .once()
            .with(predicate::eq(main_url.clone()))
            .return_const(false)
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_add()
            .once()
            .with(predicate::eq(main_url.clone()), predicate::eq(None))
            .returning(|_, _| {})
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_visited()
            .once()
            .with(predicate::eq(main_url.clone()))
            .returning(|_| {})
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_add()
            .once()
            .with(
                predicate::eq(main_url.clone()),
                predicate::eq(Some(about_url.clone())),
            )
            .returning(|_, _| {})
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_has_visited()
            .once()
            .with(predicate::eq(about_url.clone()))
            .returning(|_| false)
            .in_sequence(&mut sequence);

        url_frontier_mock
            .expect_enqueue()
            .with(predicate::eq(about_url.clone()))
            .times(1)
            .returning(|_| {})
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_add()
            .once()
            .with(
                predicate::eq(main_url.clone()),
                predicate::eq(Some(contact_url.clone())),
            )
            .returning(|_, _| {})
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_has_visited()
            .once()
            .with(predicate::eq(contact_url.clone()))
            .returning(|_| false)
            .in_sequence(&mut sequence);

        url_frontier_mock
            .expect_enqueue()
            .with(predicate::eq(contact_url.clone()))
            .once()
            .returning(|_| {})
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_add()
            .once()
            .with(
                predicate::eq(main_url.clone()),
                predicate::eq(Some("http://google.com".to_owned())),
            )
            .returning(|_, _| {})
            .in_sequence(&mut sequence);

        url_frontier_mock
            .expect_enqueue()
            .with(predicate::eq("http://google.com".to_owned()))
            .times(0)
            .returning(|_| {});

        // /about
        url_frontier_mock
            .expect_dequeue()
            .times(1)
            .return_const(Some(about_url.clone()))
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_has_visited()
            .once()
            .with(predicate::eq(about_url.clone()))
            .returning(|_| false)
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_add()
            .once()
            .with(predicate::eq(about_url.clone()), predicate::eq(None))
            .returning(|_, _| {})
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_visited()
            .once()
            .with(predicate::eq(about_url.clone()))
            .returning(|_| {})
            .in_sequence(&mut sequence);

        // /contact
        url_frontier_mock
            .expect_dequeue()
            .times(1)
            .return_const(Some(contact_url.clone()))
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_has_visited()
            .once()
            .with(predicate::eq(contact_url.clone()))
            .returning(|_| false)
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_add()
            .once()
            .with(predicate::eq(contact_url.clone()), predicate::eq(None))
            .returning(|_, _| {})
            .in_sequence(&mut sequence);

        data_store_mock
            .expect_visited()
            .once()
            .with(predicate::eq(contact_url.clone()))
            .returning(|_| {})
            .in_sequence(&mut sequence);

        url_frontier_mock
            .expect_dequeue()
            .times(1)
            .returning(|| None)
            .in_sequence(&mut sequence);

        let client: HttpFetch = Fetch::new();
        let url_parts = Arc::new(url_parts(&main_url));
        let url_frontier = Arc::new(RwLock::new(url_frontier_mock));
        let data_store = Arc::new(RwLock::new(data_store_mock));
        let deps = Arc::new(Dependencies {
            url_frontier,
            data_store,
        });
        let is_initial_crawl = true;

        run_task(deps, client, url_parts, is_initial_crawl).await;
    }
}
