use addr::parse_domain_name;
use std::sync::Arc;
use url::{ParseError, Url};

#[derive(Debug, PartialEq)]
pub enum Error {
    ParseError(ParseError),
    Other(String),
}

impl std::convert::From<addr::error::Error<'_>> for Error {
    fn from(err: addr::error::Error) -> Self {
        Error::Other(err.to_string())
    }
}

impl std::convert::From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Error::ParseError(err)
    }
}

#[derive(PartialEq, PartialOrd, Debug, Clone)]
pub struct UrlParts {
    sub_domain: String,
    root_domain: String,
}

pub fn url_parts(url: &str) -> Result<UrlParts, Error> {
    let parsed_url = Url::parse(url)?;

    if let Some(host) = parsed_url.host_str() {
        let domain = parse_domain_name(host)?;

        let sub_domain = if let Some(sub_domain) = domain.prefix() {
            sub_domain.to_owned()
        } else {
            "www".to_owned()
        };

        let root_domain = if let Some(root_domain) = domain.root() {
            root_domain.to_owned()
        } else {
            "".to_owned()
        };

        return Ok(UrlParts {
            sub_domain,
            root_domain,
        });
    }

    Ok(UrlParts {
        sub_domain: "".to_owned(),
        root_domain: "".to_owned(),
    })
}

pub fn process_url(url: String, original_url: &str) -> String {
    if Url::parse(&url) == Err(ParseError::RelativeUrlWithoutBase) {
        let original_url = Url::parse(original_url).unwrap();
        let absolute_url = original_url.join(&url);
        absolute_url.unwrap().as_str().to_owned()
    } else {
        url
    }
}

pub fn filter_url(url: String, original_url_parts: Arc<Result<UrlParts, Error>>) -> Option<String> {
    let current_url_parts = url_parts(&url);
    let original_url_parts = original_url_parts.clone();
    if current_url_parts == *original_url_parts {
        Some(url.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod link_tests {
    use std::sync::Arc;

    use super::Error;
    use url::ParseError;

    use super::url_parts;
    use crate::link::{filter_url, process_url, UrlParts};

    #[test]
    fn url_parts_constructs_url_with_www_correctly() {
        let result = url_parts("https://www.github.com");

        let expected = Ok(UrlParts {
            sub_domain: "www".to_owned(),
            root_domain: "github.com".to_owned(),
        });

        assert_eq!(result, expected);
    }

    #[test]
    fn url_parts_constructs_url_without_www_correctly() {
        let result = url_parts("https://github.com");

        let expected = Ok(UrlParts {
            sub_domain: "www".to_owned(),
            root_domain: "github.com".to_owned(),
        });

        assert_eq!(result, expected);
    }

    #[test]
    fn url_parts_url_constructs_url_without_a_host_correctly() {
        let result = url_parts("urlunix:/run/foo.socket");

        let expected = Ok(UrlParts {
            sub_domain: "".to_owned(),
            root_domain: "".to_owned(),
        });

        assert_eq!(result, expected);
    }

    #[test]
    fn url_parts_returns_error_if_invalid_url() {
        let result = url_parts("@.,~");

        let expected = Err(Error::ParseError(ParseError::RelativeUrlWithoutBase));

        assert_eq!(result, expected);
    }

    #[test]
    fn process_url_converts_relative_urls_to_absolute() {
        let link = "/users".to_owned();
        let expected = "https://github.com/users".to_owned();

        let actual = process_url(link, "https://github.com");

        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_url_filters_out_external_domain_urls() {
        let link = "https://www.stackoverflow.com".to_owned();
        let original_url_parts = UrlParts {
            sub_domain: "www".to_owned(),
            root_domain: "google.com".to_owned(),
        };

        let actual = filter_url(link, Arc::new(Ok(original_url_parts)));
        let expected = None;

        assert_eq!(actual, expected);
    }

    #[test]
    fn filter_url_keeps_internal_domain_url() {
        let link = "https://google.com".to_owned();
        let original_url_parts = UrlParts {
            sub_domain: "www".to_owned(),
            root_domain: "google.com".to_owned(),
        };

        let actual = filter_url(link, Arc::new(Ok(original_url_parts)));
        let expected = Some("https://google.com".to_owned());

        assert_eq!(actual, expected);
    }
}
