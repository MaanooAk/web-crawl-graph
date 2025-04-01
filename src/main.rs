use std::{
    collections::{HashMap, HashSet},
    env,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use rand::prelude::*;
use rand::rngs::ThreadRng;

mod http;
mod parser;

use http::{FetchResult, Page, fetch_body};
use parser::*;

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

        dbg!(&self.domains);
    }
}

fn handle_url<T: Parser>(state: &Arc<Mutex<State>>, parser: &T, url: &str) {
    
    let response: FetchResult = fetch_body(url);
    
    let mut state = state.lock().unwrap();
    state.streaming -= 1;
    if let FetchResult::Success(page) = response {
        let links = parser.links(&page);
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
        state.mapping.insert(page.url.clone(), links);
    }
}

fn select_link(state: &Arc<Mutex<State>>) -> Option<String> {
    let mut state = state.lock().unwrap();

    if state.links_pending.is_empty() {
        None
    } else {
        state.streaming += 1;

        let len = state.links_pending.len();
        let index = rand::rng().random_range(0..len);
        state.links_pending.remove(index).into()

        // state.links_pending.pop().unwrap().into()
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let state = Arc::new(Mutex::new(State::new()));

    let start_url = if args.len() > 1 {
        String::from(&args[1])
    } else {
        String::from("https://www.rust-lang.org/")
    };

    {
        let mut state = state.lock().unwrap();

        state.links_found.insert(start_url.clone());
        state.links_pending.push(start_url.clone());
    }

    let mut handles = vec![];
    for i in 0..=100 {
        let state = Arc::clone(&state);

        let parser = StaticParser::new();

        let handle = thread::spawn(move || {
            let id = format!("{:?}", std::thread::current().id());

            loop {
                let Some(url) = select_link(&state) else {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                };

                println!("{}: GET {}", id, url);

                // let state = state.lock().unwrap();
                handle_url(&state, &parser, &url);

                let state = state.lock().unwrap();
                // println!(
                //     "{} ({}) {}...",
                //     state.links_found.len(),
                //     state.links_pending.len(),
                //     state.streaming
                // );
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
            println!("received Ctrl+C!");
            
            let mut state = state.lock().unwrap();
            state.streaming -= 1000;

        }).expect("ctrlc set handler");
    }

    for handle in handles {
        handle.join().unwrap();
    }

    print_state(&state);
}

fn print_state(state: &Arc<Mutex<State>>) {
    let state = state.lock().unwrap();

    println!("digraph G {{");
    for (source, targets) in &state.mapping {
        for target in targets {

            println!("\t\"{}\" -> \"{}\"", &source, &target);
        }
    }
    println!("}}");

}