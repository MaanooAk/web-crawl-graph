use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::BufWriter,
    num::NonZero,
    sync::{Arc, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

use ::rand as the_rand;
use ::rand::prelude::*;
use clap::command;

mod gui;
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
    order: Vec<String>,

    links_found: HashSet<String>,
    links_found_order: Vec<String>,

    links_pending: Vec<String>,
    streaming: i32,
    // domains: HashMap<String, i32>,
    // rand: ThreadRng,
}

impl State {
    fn new(start: &str) -> Self {
        let mut state = Self {
            mapping: HashMap::new(),
            order: Vec::new(),
            links_found: HashSet::new(),
            links_found_order: Vec::new(),
            links_pending: Vec::new(),
            streaming: 0,
            // domains: HashMap::new(),
            // rand: rand::rng(),
        };

        state.links_found.insert(String::from(start));
        state.links_found_order.push(String::from(start));
        state.links_pending.push(String::from(start));

        state
    }

    fn new_page(&mut self, page: &Page, links: HashSet<String>) {
        if self.mapping.contains_key(&page.domain) {
            return;
        }

        for link in &links {
            if self.links_found.contains(link) {
                continue;
            } else {
                self.links_found.insert(link.clone());
                self.links_found_order.push(link.clone());
                self.links_pending.push(link.clone());
            }
        }

        if !self.links_found.contains(&page.domain) {
            // TODO create alias
            self.links_found.insert(page.domain.clone());
            self.links_found_order.push(page.domain.clone());
        }

        self.order.push(page.domain.clone());
        self.mapping.insert(page.domain.clone(), links);
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
        domains.remove(&page.domain); // TODO check
        let links = domains;

        // critical

        let mut state = state.lock().unwrap();
        state.streaming -= 1;
        state.new_page(&page, links);
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
        let index = the_rand::rng().random_range(0..len);
        let url = state.links_pending.remove(index);

        Next::Some(url)
    }
}

use clap::arg;

use macroquad::prelude::*;

#[macroquad::main("Crawler")]
async fn main() {
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

    let state = Arc::new(Mutex::new(State::new(start_url)));

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

                eprintln!("{: >4}: GET {}", id, url);

                // let state = state.lock().unwrap();
                handle_url(&state, &parser, &client, &url);

                let state = state.lock().unwrap();
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

    {
        let state = Arc::clone(&state);

        let mut nodes = Vec::<Node>::new();
        let mut connections = Vec::<Connection>::new();
        let mut last_state_order_len = 0;
        let mut visible_nodes = 0;

        let mut has_ended = false;
        let mut last_update = Instant::now();
        loop {
            let h = screen_height();
            let w = screen_width();

            clear_background(BLACK);

            let size = f32::min(10.0, 10.0 * 300.0 / visible_nodes as f32);

            for i in &connections {
                let source = nodes.get(i.source).unwrap();
                let target = nodes.get(i.target).unwrap();

                if !source.visible || !target.visible {
                    continue;
                }

                draw_line(
                    w * 0.5 + source.x * size,
                    h * 0.5 + source.y * size,
                    w * 0.5 + target.x * size,
                    h * 0.5 + target.y * size,
                    2.0,
                    Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.2,
                    },
                );
            }
            for i in &nodes {
                if !i.visible {
                    continue;
                }

                draw_circle(w * 0.5 + i.x * size, h * 0.5 + i.y * size, size, WHITE);
            }

            // ===

            let movement = simulate_physics(&mut nodes, &connections);
            if movement <= visible_nodes as f32 * 1.0 {
                for i in &mut nodes {
                    if !i.visible {
                        i.visible = true;
                        visible_nodes += 1;
                        break;
                    }
                }
            }

            draw_text(
                format!("no {}/{} mo {}", visible_nodes, nodes.len(), movement).as_str(),
                20.0,
                20.0,
                30.0,
                GRAY,
            );

            next_frame().await;

            if has_ended {
                continue;
            }
            if last_update.elapsed() < Duration::from_millis(100) {
                continue;
            }
            last_update = Instant::now();

            let state = state.lock().unwrap();
            eprintln!(
                "{} ({}) {}... = {}",
                state.links_found.len(),
                state.links_pending.len(),
                state.streaming,
                state.mapping.len()
            );

            let ended = (state.links_pending.is_empty() && state.streaming == 0)
                || state.streaming == -1000;
            if ended {
                has_ended = true;

                // for handle in handles {
                //     handle.join();
                // }

                {
                    let mut filename = String::new();
                    filename.push_str(start_url);
                    filename.push_str("-graph.dot");

                    eprintln!("Exporting to {filename}");
                    export_dot_file(&state, &filename);
                }
            }

            // if nodes.len() > 100 {
            //     continue;
            // }

            while nodes.len() < state.links_found_order.len() {
                let name = &state.links_found_order[nodes.len()];

                let node = Node {
                    name: name.clone(),
                    x: 0.0 + rand::gen_range(-100.0, 100.0),
                    y: 0.0 + rand::gen_range(-100.0, 100.0),
                    visible: false,
                };
                nodes.push(node.clone());
            }

            for index in last_state_order_len..state.order.len() {
                let source_name = &state.order[index];
                let source_index = nodes.iter().position(|i| &i.name == source_name).unwrap();

                for target_name in state.mapping.get(source_name).unwrap() {
                    let target_index = nodes.iter().position(|i| &i.name == target_name).unwrap();

                    if target_index == source_index {
                        continue;
                    }
                    connections.push(Connection {
                        source: source_index,
                        target: target_index,
                    });
                }
            }
            last_state_order_len = state.order.len();
        }
    }
}

#[derive(Clone)]
struct Node {
    name: String,
    x: f32,
    y: f32,
    visible: bool,
}

#[derive(Clone)]
struct Connection {
    source: usize,
    target: usize,
}

use macroquad::math::Vec2;

fn simulate_physics(nodes: &mut Vec<Node>, connections: &Vec<Connection>) -> f32 {
    let mut movement = 0.0;

    let step = 1.0 / 60.0;

    for con in connections {
        let n1 = &nodes[con.source];
        if !n1.visible {
            continue;
        }
        let n2 = &nodes[con.target];
        if !n2.visible {
            continue;
        }
        let mut v1 = Vec2::new(n1.x, n1.y);
        let mut v2 = Vec2::new(n2.x, n2.y);
        let dis = v1.distance(v2);
        let d = v1 - v2;

        if dis <= 5.0 {
            continue;
        }

        let scale = (dis / 5.0).powf(2.0);
        v1 += d.normalize() * -1.0 * step * scale;
        v2 += d.normalize() * 1.0 * step * scale;
        movement += (step * scale).powf(2.0);

        nodes[con.source].y = v1.y;
        nodes[con.source].x = v1.x;
        nodes[con.target].x = v2.x;
        nodes[con.target].y = v2.y;
    }

    for i1 in 0..nodes.len() {
        let n1 = &nodes[i1];
        if !n1.visible {
            continue;
        }
        let mut v1 = Vec2::new(n1.x, n1.y);

        for i2 in i1 + 1..nodes.len() {
            debug_assert_ne!(i1, i2);

            let n2 = &nodes[i2];
            if !n2.visible {
                continue;
            }

            let mut v2 = Vec2::new(n2.x, n2.y);
            let dis = v1.distance(v2);
            let d = v1 - v2;

            if dis < 2.5 {
                let scale = (2.5 - dis) / 2.0;
                v1 += d.normalize() * 1.0 * scale;
                v2 += d.normalize() * -1.0 * scale;
                movement += (scale).powf(2.0);
            } else {
                // let c12 = connections.iter().find(|c| {
                //     (c.source == i1 && c.target == i2) || (c.source == i2 && c.target == i1)
                // });

                // if c12.is_some() {
                //     if dis > 5.0 {
                //         let scale = dis / 5.0;
                //         v1 += d.normalize() * -1.0 * step * scale;
                //         v2 += d.normalize() * 1.0 * step * scale;
                //         movement += step * scale;
                //     }
                // }
                // } else {
                if dis < 20.0 {
                    let scale = 80.0 / dis;
                    v1 += d.normalize() * 1.0 * step * scale;
                    v2 += d.normalize() * -1.0 * step * scale;
                    movement += (step * scale).powf(2.0);
                }
                // }
            }

            nodes[i2].x = v2.x;
            nodes[i2].y = v2.y;
        }
        nodes[i1].x = v1.x;
        nodes[i1].y = v1.y;
    }
    if !nodes.is_empty() {
        nodes[0].x = 0.0;
        nodes[0].y = 0.0;
    }
    return movement;
}

use std::io::Write;

fn export_dot_file(state: &MutexGuard<'_, State>, filename: &str) {
    let mut writer = BufWriter::new(File::create(filename).expect("Could open file"));

    writeln!(&mut writer, "digraph G {{").unwrap();
    for source in &state.order {
        // for (source, targets) in &state.mapping {
        let targets = state.mapping.get(source).unwrap();
        for target in targets {
            writeln!(&mut writer, "\t\"{}\" -> \"{}\"", &source, &target).unwrap();
        }
    }
    writeln!(&mut writer, "}}").unwrap();
}
