use std::{
    collections::{HashMap, HashSet},
    env,
    fs::File,
    io::{BufWriter, Write},
    num::NonZero,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use clap::command;
use rand::prelude::*;

mod http;
mod parser;
mod url;

use http::{FetchResult, Page, fetch_body};
use parser::*;
use reqwest::blocking::Client;
use url::domain_of;

#[derive(Debug)]
struct State {
    mapping: HashMap<String, HashSet<String>>,

    links_found: HashSet<String>,
    links_pending: Vec<String>,
    streaming: i32,

    domains: HashMap<String, i32>,
    // rand: ThreadRng,
}

impl State {
    fn new() -> Self {
        Self {
            mapping: HashMap::new(),
            links_found: HashSet::new(),
            links_pending: Vec::new(),
            streaming: 0,
            domains: HashMap::new(),
            // rand: rand::rng(),
        }
    }

    fn new_page(&mut self, page: &Page) {
        let count = self.domains.entry(page.domain.to_owned()).or_default();
        *count += 1;

        // dbg!(&self.domains);
    }
}

fn handle_url<T: Parser>(state: &Arc<Mutex<State>>, parser: &T, client: &Client, url: &str) {
    let response: FetchResult = fetch_body(client, url);

    if let FetchResult::Success(page) = response {
        let links = parser.links(&page);
        let mut domains = HashSet::new();
        for i in links {
            domains.insert(String::from(domain_of(&i)));
        }
        let links = domains;

        // critical

        let mut state = state.lock().unwrap();
        state.streaming -= 1;
        state.new_page(&page);

        for link in &links {
            if state.links_found.contains(link) {
                continue;
            } else {
                state.links_found.insert(link.clone());
                state.links_pending.push(link.clone());
            }
        }
        // dbg!(&links);
        state.mapping.insert(domain_of(&page.url).into(), links);
    } else {
        // critical

        let mut state = state.lock().unwrap();
        state.streaming -= 1;
    }
}

enum Next<T> {
    Some(T),
    Wait,
    End,
}

fn select_link(state: &Arc<Mutex<State>>) -> Next<String> {
    // critical

    let mut state = state.lock().unwrap();

    if state.links_pending.is_empty() {
        if state.streaming <= 0 {
            Next::End
        } else {
            Next::Wait
        }
    } else {
        state.streaming += 1;

        let len = state.links_pending.len();
        let index = rand::rng().random_range(0..len);
        let url = state.links_pending.remove(index);

        Next::Some(url)
    }
}

use clap::arg;

fn main() {
    let commands = command!()
        .arg(
            arg!(
                [url] "The url to start from"
            )
            .default_value("rust-lang.org"),
        )
        .arg(
            arg!(
                -j --threads [threads] "The number of threads"
            )
            .default_value("10"),
        )
        .get_matches();

    let start_url = commands.get_one::<String>("url").unwrap();
    let threads = commands
        .get_one::<String>("threads")
        .unwrap()
        .parse::<NonZero<u32>>()
        .expect("number of threads must a positive number");

    let state = Arc::new(Mutex::new(State::new()));

    {
        let mut state = state.lock().unwrap();

        let start_url = String::from(domain_of(&start_url));
        state.links_found.insert(start_url.clone());
        state.links_pending.push(start_url.clone());
    }

    let mut handles = vec![];
    for id in 0..threads.get() {
        let state = Arc::clone(&state);

        let parser = StaticParser::new();

        let handle = thread::spawn(move || {
            // let id = format!("{:?}", std::thread::current().id());

            let client = Client::new();

            loop {
                let url = match select_link(&state) {
                    Next::Some(url) => url,
                    Next::Wait => {
                        thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    Next::End => {
                        break;
                    }
                };

                eprintln!("{}: GET {}", id, url);

                // let state = state.lock().unwrap();
                handle_url(&state, &parser, &client, &url);

                let state = state.lock().unwrap();
                eprintln!(
                    "{} ({}) {}... = {}",
                    state.links_found.len(),
                    state.links_pending.len(),
                    state.streaming,
                    state.mapping.len()
                );
                if state.streaming < 0 {
                    break;
                }
            }
        });
        handles.push(handle);
    }

    {
        let state = Arc::clone(&state);
        ctrlc::set_handler(move || {
            eprintln!("received Ctrl+C!");

            let mut state = state.lock().unwrap();
            state.streaming -= 1000;
        })
        .expect("ctrlc set handler");
    }

    for handle in handles {
        if let Err(_err) = handle.join() {
            // TODO handle
        }
    }

    {
        let mut filename = String::new();
        filename.push_str(start_url);
        filename.push_str("-graph.dot");

        eprintln!("Exporting to {filename}");
        export_dot_file(&state, &filename);
    }
}

fn export_dot_file(state: &Arc<Mutex<State>>, filename: &str) {
    let state = state.lock().unwrap();

    let mut writer = BufWriter::new(File::create(filename).expect("Could open file"));

    writeln!(&mut writer, "digraph G {{").unwrap();
    for (source, targets) in &state.mapping {
        for target in targets {
            writeln!(&mut writer, "\t\"{}\" -> \"{}\"", &source, &target).unwrap();
        }
    }
    writeln!(&mut writer, "}}").unwrap();
    
}
