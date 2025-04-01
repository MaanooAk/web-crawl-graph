use regex::Regex;
use std::{cmp, collections::HashSet};

use crate::http::Page;

pub trait Parser {
    fn links(&self, page: &Page) -> HashSet<String>;
}

pub struct StaticParser {
    regex: Regex,
    exts: Vec<&'static str>,
}

impl StaticParser {
    pub fn new() -> Self {
        StaticParser {
            regex: Regex::new(r#"<a[^>]*href="([^"\n]*)""#).unwrap(),
            exts: vec!["html", "htm", "php", "jsp", "jspx", "asp", "aspx"],
        }
    }
}

impl Parser for StaticParser {
    fn links(&self, page: &Page) -> HashSet<String> {
        //! Simple regex search

        let mut set = HashSet::new();

        for capture in self.regex.captures_iter(&page.body) {
            let (_, [href]) = capture.extract();

            if href.starts_with("mailto:") || href.contains("{{") {
                continue;
            }

            if let Some(ext) = extension(href) {
                // println!("{ext} of {href}");
                if !self.exts.contains(&ext) {
                    // println!("skip extension {} of {}", ext, href);
                    continue;
                }
            }

            set.insert(absolute_link(&page.url, href));
        }
        set
    }
}

pub fn extension(path: &str) -> Option<&str> {
    let path = path
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    let len = path.len();
    let index1 = path.find("?");
    let index2 = path.find("#");
    let tranc_index = cmp::min(index1.unwrap_or(len), index2.unwrap_or(len));

    let path = if tranc_index == len {
        path
    } else {
        &path[..tranc_index]
    };

    let Some((_, path)) = path.rsplit_once("/") else {
        return None;
    };
    if let Some((_, ext)) = path.rsplit_once(".") {
        ext.into()
    } else {
        None
    }
}

fn absolute_link(base: &str, link: &str) -> String {
    if link.starts_with("http://") || link.starts_with("https://") {
        String::from(link)
    } else {
        let mut absolute = if base.ends_with("/") {
            String::from(base)
        } else {
            if let Some((path, _)) = base.rsplit_once("/") {
                let mut absolute = String::from(path);
                absolute.push_str("/");
                absolute
            } else {
                panic!("{}", base);
            }
        };
        // TODO handle "".."" prefix
        absolute.push_str(link.trim_start_matches("/"));
        absolute
    }
}
