struct Url<'a> {
    path: String,
    domain: &'a str,
}

// pub fn domain_of(path: &str) -> &str {
//     println!("{path} = {}", domain_of1(path));
//     return domain_of1(path);
// }

pub fn domain_of(path: &str) -> &str {
    let path = path
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    let domain = match path.find(|c| c == '/' || c == '?' || c == '#') {
        Some(index) => path.split_at(index).0,
        _ => path,
    };

    let mut words = domain.rsplit(".");
    let Some(last) = words.next() else {
        return domain;
    };
    let Some(pre) = words.next() else {
        return domain;
    };
    if pre.len() > 3 {
        return &domain[(domain.len() - last.len() - pre.len() - 1)..];
    }
    let Some(name) = words.next() else {
        return domain;
    };
    return &domain[(domain.len() - last.len() - pre.len() - name.len() - 2)..];
}

// pub fn domain_of(path: &str) -> &str {
//     let path = path
//         .trim_start_matches("https://")
//         .trim_start_matches("http://");
//     if let Some((domain, _)) = path.split_once("/") {
//         domain
//     } else {
//         path
//     }
// }
