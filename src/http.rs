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

use reqwest::header;
use FetchResult::*;

pub fn fetch_body(url: &str) -> FetchResult {
    //! Http request a url and return the body text of the response

    let res = match reqwest::blocking::get(url) {
        Ok(res) => res,
        Err(_) => {
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
    let domain = String::from(res.url().domain().unwrap());
    let Ok(body) = res.text() else {
        return Fail;
    };

    Success(Page {
        url: final_url,
        domain, body,
    })
}
