use scraper::{Html, Selector};

pub struct Parser {
    html_parsed: Html,
}

impl Parser {
    pub fn new(html_doc_str: String) -> Self {
        Parser {
            html_parsed: Html::parse_document(&html_doc_str),
        }
    }

    pub fn all_links(self) -> Vec<String> {
        let mut vec: Vec<String> = vec![];
        let selector = Selector::parse("a").unwrap();

        for element in self.html_parsed.select(&selector) {
            let parsed = element.value().attr("href");
            if let Some(el) = parsed {
                vec.push(el.to_owned());
            }
        }

        vec
    }
}

#[cfg(test)]
mod parser_tests {
    use super::Parser;

    #[test]
    fn all_links_extracts_existing_links_from_html_string() {
        let parser = Parser::new(
            "<p>One</p><a href=\"/link\"a>Link</a><p>Two</p><a href=\"/link-two\">Link 2</a>"
                .to_owned(),
        );
        let mut links = parser.all_links();

        assert!(links.len() == 2);
        assert_eq!(links.pop(), Some("/link-two".to_owned()));
        assert_eq!(links.pop(), Some("/link".to_owned()));
        assert_eq!(links.pop(), None);
    }
}
