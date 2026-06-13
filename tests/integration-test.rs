#[cfg(test)]
mod integration_tests {
    use assert_cmd::Command;
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
    async fn binary_runs_as_expected() {
        // --- arrange
        let test_url = "http://localhost:45367";
        let listener = std::net::TcpListener::bind("localhost:45367").unwrap();
        let mock_server = MockServer::builder().listener(listener).start().await;
        let hrefs = make_hrefs(test_url);
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

        // --- act
        let mut cmd = Command::cargo_bin("url-crawler").unwrap();

        // --- assert
        let assert = cmd.args(["--url", &test_url]).assert();

        let expected = "[INFO  url_crawler::task] Visited URL: http://localhost:45367\n\
    [INFO  url_crawler::task] Found URL: http://localhost:45367/about\n\
    [INFO  url_crawler::task] Found URL: http://localhost:45367/contact\n\
    [INFO  url_crawler::task] Found URL: http://google.com\n\
    [INFO  url_crawler::task] --------------------------------------------\n\
    [INFO  url_crawler::task] Visited URL: http://localhost:45367/about\n\
    [INFO  url_crawler::task] --------------------------------------------\n\
    [INFO  url_crawler::task] Visited URL: http://localhost:45367/contact\n\
    [INFO  url_crawler::task] --------------------------------------------\n\
    [INFO  url_crawler] Worker completed\n\
    [INFO  url_crawler] Done!\n";

        assert.success().code(0).stderr(expected);
    }
}
