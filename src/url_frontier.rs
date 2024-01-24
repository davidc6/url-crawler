use async_trait::async_trait;
use crossbeam_queue::SegQueue;
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;

pub trait Queue: Enqueue + Dequeue {}

#[async_trait]
pub trait Dequeue {
    async fn dequeue(&mut self) -> Option<String>;
}

pub trait Enqueue {
    fn enqueue(&mut self, value: String);
}

#[derive(Default)]
pub struct URLFrontier {
    queue: Arc<SegQueue<String>>,
    delay_s: Option<u64>,
}

impl Queue for URLFrontier {}

#[async_trait]
impl Dequeue for URLFrontier {
    async fn dequeue(&mut self) -> Option<String> {
        if let Some(delay_s) = self.delay_s {
            sleep(Duration::from_secs(delay_s)).await;
        }
        self.queue.pop()
    }
}

impl Enqueue for URLFrontier {
    fn enqueue(&mut self, value: String) {
        self.queue.push(value)
    }
}

#[derive(Default)]
pub struct URLFrontierBuilder {
    queue: SegQueue<String>,
    delay_s: Option<u64>,
}

impl URLFrontierBuilder {
    pub fn new() -> URLFrontierBuilder {
        URLFrontierBuilder {
            queue: SegQueue::new(),
            delay_s: None,
        }
    }

    pub fn value(self, value: String) -> URLFrontierBuilder {
        self.queue.push(value);
        self
    }

    pub fn delay_s(mut self, delay_s: u64) -> URLFrontierBuilder {
        if delay_s > 0 {
            self.delay_s = Some(delay_s)
        }
        self
    }

    pub fn build(self) -> URLFrontier {
        URLFrontier {
            queue: self.queue.into(),
            delay_s: self.delay_s,
        }
    }
}

#[cfg(test)]
mod url_frontier_tests {
    use super::Dequeue;
    use super::Enqueue;
    use super::URLFrontierBuilder;

    #[test]
    fn url_frontier_builder_builds_url_frontier() {
        let url_frontier = URLFrontierBuilder::new()
            .delay_s(1)
            .value("one".to_string())
            .build();

        assert!(url_frontier.delay_s == Some(1));
        assert!(url_frontier.queue.pop() == Some("one".to_owned()));
    }

    #[tokio::test]
    async fn url_frontier_dequeues_value() {
        let mut url_frontier = URLFrontierBuilder::new()
            .delay_s(0)
            .value("one".to_string())
            .build();

        let val = url_frontier.dequeue().await;

        assert_eq!(val, Some("one".to_owned()));
    }

    #[tokio::test]
    async fn url_frontier_dequeues_none_if_there_are_no_values_in_the_queue() {
        let mut url_frontier = URLFrontierBuilder::new().delay_s(0).build();

        let val = url_frontier.dequeue().await;

        assert_eq!(val, None);
    }

    #[tokio::test]
    async fn url_frontier_enqueues_value() {
        let mut url_frontier = URLFrontierBuilder::new().delay_s(0).build();

        url_frontier.enqueue("two".to_owned());
        let val = url_frontier.dequeue().await;

        assert_eq!(val, Some("two".to_owned()));
        assert_eq!(url_frontier.dequeue().await, None);
    }
}
