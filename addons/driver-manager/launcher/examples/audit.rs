use std::collections::{BTreeSet, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

use swofty_formats::gvdb::Gvdb;
use swofty_formats::hash::{fnv1a64, fnv1a64_from};

#[derive(Default)]
struct Id(u64);

impl Hasher for Id {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {
        unreachable!()
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }
}

type Keys = HashSet<u64, BuildHasherDefault<Id>>;

fn scan(keys: &Keys, base: u64, names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|n| keys.contains(&fnv1a64_from(base, n)))
        .cloned()
        .collect()
}

fn containers(keys: &Keys, base: u64, candidates: &[String], leaves: &[String]) -> Vec<String> {
    let threads = 20;
    let chunk = candidates.len().div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = candidates
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .filter(|c| {
                            let h = fnv1a64_from(base, format!("{c}."));
                            leaves.iter().any(|l| keys.contains(&fnv1a64_from(h, l)))
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 6 {
        eprintln!("usage: audit <db> <idents.txt> <all.tsv> <extra names.txt> <row prefix>...");
        std::process::exit(2);
    }
    let db = Gvdb::parse(&std::fs::read(&args[1]).unwrap()).unwrap();
    let keys: Keys = db.records.iter().map(|r| r.key).collect();
    let mut idents: BTreeSet<String> = std::fs::read_to_string(&args[2])
        .unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let extra: Vec<String> = std::fs::read_to_string(&args[4])
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut segs = BTreeSet::new();
    let mut leaves = BTreeSet::new();
    for line in std::fs::read_to_string(&args[3])
        .unwrap()
        .lines()
        .chain(extra.iter().map(|e| e.as_str()))
    {
        let path = line.split('\t').nth(1).unwrap_or(line);
        let parts: Vec<&str> = path.split('.').collect();
        leaves.insert(parts[parts.len() - 1].to_owned());
        for p in &parts[..parts.len() - 1] {
            let base = match p.rsplit_once('_') {
                Some((b, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => b,
                _ => p,
            };
            segs.insert(base.to_owned());
        }
    }
    for i in &idents {
        if let Some((a, _)) = i.split_once('.') {
            segs.insert(a.to_owned());
        }
    }
    for l in &leaves {
        idents.insert(l.clone());
    }
    let idents: Vec<String> = idents.into_iter().collect();
    let mut short: Vec<String> = leaves.iter().cloned().collect();
    short.extend(idents.iter().filter(|i| i.contains('.')).cloned());
    for e in &extra {
        let parts: Vec<&str> = e.split('.').collect();
        if parts.len() >= 2 {
            short.push(parts[parts.len() - 2..].join("."));
        }
    }
    short.sort();
    short.dedup();
    let mut cands: Vec<String> = Vec::new();
    for s in &segs {
        cands.push(s.clone());
        if s.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
            for i in 0..8 {
                cands.push(format!("{s}_{i}"));
            }
        }
    }
    eprintln!(
        "{} idents, {} short leaves, {} container candidates",
        idents.len(),
        short.len(),
        cands.len()
    );
    for prefix in &args[5..] {
        let mut found: BTreeSet<String> = BTreeSet::new();
        let mut queue = vec![String::new()];
        let mut seen = HashSet::new();
        while let Some(sub) = queue.pop() {
            if !seen.insert(sub.clone()) || sub.matches('.').count() > 3 {
                continue;
            }
            let base = fnv1a64(format!("{prefix}.{sub}"));
            for n in scan(&keys, base, &idents) {
                found.insert(format!("{sub}{n}"));
            }
            for c in containers(&keys, base, &cands, &short) {
                queue.push(format!("{sub}{c}."));
            }
        }
        let list: Vec<&String> = found.iter().collect();
        println!(
            "{prefix}\t{}\t{}",
            list.len(),
            serde_json::to_string(&list).unwrap()
        );
    }
}
