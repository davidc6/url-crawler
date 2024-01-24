# URL Crawler

## Context

This is a simple CLI application that can crawl a single URL / domain, extract all links and store only the non-external ones. It is written in [Rust 1.75.0](https://www.rust-lang.org/) and makes use of [Tokio async runtime](https://tokio.rs/) for asynchronicity/concurrency. 

**Please note**: to make this project production ready, take care of all the edge cases and confirm scope etc will take a bit more time. I am aware of many trade-offs and future improvements that can be made to it and have documented these below. 

## Instructions

`cargo` has to be installed in order to run the application - https://doc.rust-lang.org/cargo/getting-started/installation.html

To run the app from the terminal, cd into the root of the project and then run - `cargo run -- --url <url_to_crawl>`.

To run tests - `cargo test`

Additional cli options can be provided:

- `--workers_n <number_of_workers_to_create>` (defaults to 1)
- `--delay <delay_in_seconds>` (to delay requests to the host, defaults to 2)
- `--print <bool>` (whether data store should be printed at the end of the crawl, default to false)

## Components

- URL Frontier - a very simple implementation of a component that manages URLs. The component makes use of crossbeams `SeqQueue` which is a thread-safe queue.
- Data store - a simple in-memory data store that uses a HashMap to track downloaded and visited URLs
- Link - links/urls maker and filter
- Fetch - Http client abstraction
- Parser - Content parser and links extractor

## Basic flow

1. URL Frontier gets a seed url
2. N number of tasks (green threads) get created
3. Each task gets a pointer to
    - URL frontier, to populate it with new URLs
    - Data store, to track visited and downloaded URLs
4. URL Frontier pops a url and it is checked for visited status
5. Data from URL gets downloaded
6. URL gets marked as visiting in the data store
7. Content gets parsed and links extracted
8. Urls/links get filtered based on the initial / seed URL
9. Each new URL gets updates / added to the data store (if not already in it)
10. Each new, unvisited url gets added to the URL Frontier to be crawled later

## Potential future improvements / trade-offs (in no particular order)

- User input validation and initial URL validation
- Instead of using a simple HashMap as data store use a combination of in-memory and disk databases (and Docker compose to bring all the components up)
- Store webpage content and compare in the future crawls to avoid fetching stale data/pages
- For JS only sites a different technique is needed, i.e. a webdriver
- Store date/time when a URL was visited and compare whether it potentially can be stale
- Instead of storing URLs, to save space, store URL checksums
- In order to expand crawling to other domain, separate queues can be introduced inside of the URL frontier component
- If a request to a url fails, a retry mechanism can be implemented (or URL can be enqueued and tried again)
- A graph data structure (instead of a queue) that links all pages in order to construct a sitemap can be introduced
- Politeness factor / delay can also be set to a float type (to work on milliseconds level)
- Check for "https://" when reading CLI commands and if doesn't exist, prepend
- Add test coverage tooling setup
- Data collector instead of printing info at the end of the crawl
- Differentiate between different link types (images, text, etc.)
- Mocking dependencies and checking for number of calls to each method and add a couple of separate integrations tests to `execute()`
- Handling of fragment urls (i.e. /about#section1) 
- Better handling of domain specific politeness / delay
- Provide an abstraction to write output to a file
- Environment dependent logger
- Have a go at wiring up [libfaketime](https://github.com/wolfcw/libfaketime) or [fluxcapacitor](https://github.com/majek/fluxcapacitor#fluxcapacitor) to mock system time for tests

## Version 0.2.0 changelog

- Task/thread logic moved into a separate module to better/easier unit testing
- Integration test moved into a separate directory to make it separate from the rest of binary
- Logging is now without timestamp (to allow for cleaner testing for now)
- Store and URLFrontier are now passed in as trait objects so we are coding to interface and not implementation
- Store and URLFrontier traits are mocked for testing purposes
- URLFrontier split into two traits (sync and async) in order to enable mocking
- Improved tests (main.rs)
- task.rs contains a sequential steps test that makes sure that the sequence of calls is correct
- Address parsing now works ok with localhost instead of 127.0.0.1 (previously it could not parse TLD)
