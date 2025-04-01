#[derive(Debug)]

pub struct Page {
    pub url: String,
    pub domain: String,
    pub body: String,
}

pub enum FetchResult {
    Success(Page),
    // Retry,
    Fail,
}

use std::time::Duration;

use FetchResult::*;
use reqwest::header;

use crate::url::domain_of;

pub fn fetch_body(client: &reqwest::blocking::Client, url: &str) -> FetchResult {
    //! Http request a url and return the body text of the response

    let mut url = String::from(url);
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        url.insert_str(0, "https://");
    }

    // let client = reqwest::blocking::Client::new();
    let request = client.get(&url).timeout(Duration::from_secs(5));

    let res = match request.send() {
        Ok(res) => res,
        Err(_err) => {
            // panic!("{url}: {_err}");
            // TODO check the error
            return Fail;
        }
    };

    let Some(content_type) = res.headers().get(header::CONTENT_TYPE) else {
        return Fail;
    };
    let Ok(content_type) = content_type.to_str() else {
        return Fail;
    };
    if !content_type.starts_with("text/html") {
        // println!("skip {content_type}");
        return Fail;
    }

    let final_url = String::from(res.url().as_str());
    let domain = String::from(domain_of(res.url().domain().unwrap()));
    let Ok(body) = res.text() else {
        return Fail;
    };

    Success(Page {
        url: final_url,
        domain,
        body,
    })
}
