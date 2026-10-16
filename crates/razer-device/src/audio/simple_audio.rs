//! Current simple_service AudioServiceWin records and libc++ hash-table order.
//! Recovered from PE RVAs 3a170/3b820/3be88/1f746; no OS or vendor DLL.
use anyhow::ensure;
use serde::Serialize;
use std::num::Wrapping as W;

#[derive(Clone, Debug, Serialize)]
pub struct SimpleAudioEndpoint {
    pub id: String,
    pub name: String,
    #[serde(rename = "containerId")]
    pub container_id: String,
    #[serde(rename = "type")]
    pub endpoint_type: &'static str,
}

const K0: W<u64> = W(0xc3a5c85c97cb3127);
const K1: W<u64> = W(0xb492b66fbe98f273);
const K2: W<u64> = W(0x9ae16a3b2f90404f);
const K3: W<u64> = W(0xc949d7c7509e6557);
const KM: W<u64> = W(0x9ddfea08eb382d69);
fn shift(x: W<u64>) -> W<u64> {
    x ^ (x >> 47)
}
fn rotate(x: W<u64>, n: u32) -> W<u64> {
    W(x.0.rotate_right(n))
}
fn fetch(s: &[u8], at: usize) -> W<u64> {
    W(u64::from_le_bytes(s[at..at + 8].try_into().unwrap()))
}
fn hash_pair(u: W<u64>, v: W<u64>) -> W<u64> {
    let a = shift((u ^ v) * KM);
    shift((v ^ a) * KM) * KM
}
fn weak(s: &[u8], at: usize, mut a: W<u64>, mut b: W<u64>) -> (W<u64>, W<u64>) {
    let w = fetch(s, at);
    let x = fetch(s, at + 8);
    let y = fetch(s, at + 16);
    let z = fetch(s, at + 24);
    a += w;
    b = rotate(b + a + z, 21);
    let c = a;
    a += x + y;
    b += rotate(a, 44);
    (a + z, b + c)
}

/// Exact 64-bit hash variant in the current PE, including the old short-string
/// formulas. It differs from newer CityHash versions for lengths <= 64.
pub fn source_hash(s: &[u8]) -> u64 {
    let n = s.len();
    let len = W(n as u64);
    if n <= 16 {
        if n > 8 {
            let b = fetch(s, n - 8);
            return (hash_pair(fetch(s, 0), rotate(b + len, n as u32)) ^ b).0;
        }
        if n >= 4 {
            let a = W(u32::from_le_bytes(s[..4].try_into().unwrap()) as u64);
            let b = W(u32::from_le_bytes(s[n - 4..].try_into().unwrap()) as u64);
            return hash_pair(len + (a << 3), b).0;
        }
        if n > 0 {
            let y = W(s[0] as u64 | ((s[n >> 1] as u64) << 8));
            let z = len + W((s[n - 1] as u64) << 2);
            return (shift(y * K2 ^ z * K3) * K2).0;
        }
        return K2.0;
    }
    if n <= 32 {
        let a = fetch(s, 0) * K1;
        let b = fetch(s, 8);
        let c = fetch(s, n - 8) * K2;
        let d = fetch(s, n - 16) * K0;
        return hash_pair(
            rotate(a - b, 43) + rotate(c, 30) + d,
            a + rotate(b ^ K3, 20) - c + len,
        )
        .0;
    }
    if n <= 64 {
        let mut a = fetch(s, 0) + (len + fetch(s, n - 16)) * K0;
        let mut b = rotate(a + fetch(s, 24), 52);
        let mut c = rotate(a, 37);
        a += fetch(s, 8);
        c += rotate(a, 7);
        a += fetch(s, 16);
        let vf = a + fetch(s, 24);
        let vs = b + rotate(a, 31) + c;
        a = fetch(s, 16) + fetch(s, n - 32);
        b = rotate(a + fetch(s, n - 8), 52);
        c = rotate(a, 37);
        a += fetch(s, n - 24);
        c += rotate(a, 7);
        a += fetch(s, n - 16);
        let wf = a + fetch(s, n - 8);
        let ws = b + rotate(a, 31) + c;
        let r = shift((vf + ws) * K2 + (wf + vs) * K0);
        return (shift(r * K0 + vs) * K2).0;
    }
    let mut x = fetch(s, n - 40);
    let mut y = fetch(s, n - 16) + fetch(s, n - 56);
    let mut z = hash_pair(fetch(s, n - 48) + len, fetch(s, n - 24));
    let mut v = weak(s, n - 64, len, z);
    let mut w = weak(s, n - 32, y + K1, x);
    x = x * K1 + fetch(s, 0);
    let mut remaining = (n - 1) & !63;
    let mut at = 0;
    loop {
        x = rotate(x + y + v.0 + fetch(s, at + 8), 37) * K1;
        y = rotate(y + v.1 + fetch(s, at + 48), 42) * K1;
        x ^= w.1;
        y += v.0 + fetch(s, at + 40);
        z = rotate(z + w.0, 33) * K1;
        v = weak(s, at, v.1 * K1, x + w.0);
        w = weak(s, at + 32, z + w.1, y + fetch(s, at + 16));
        std::mem::swap(&mut z, &mut x);
        at += 64;
        remaining -= 64;
        if remaining == 0 {
            break;
        }
    }
    hash_pair(
        hash_pair(v.0, w.0) + shift(y) * K1 + z,
        hash_pair(v.1, w.1) + x,
    )
    .0
}

fn next_prime(mut n: usize) -> usize {
    if n <= 2 {
        return 2;
    }
    if n % 2 == 0 {
        n += 1;
    }
    while (3..)
        .step_by(2)
        .take_while(|d| *d <= n / *d)
        .any(|d| n % d == 0)
    {
        n += 2;
    }
    n
}

/// Emulates the recovered table's bucket growth, insertion and rehash linked
/// list, rather than sorting names/IDs or using Rust's randomized HashMap.
pub fn source_order(entries: Vec<SimpleAudioEndpoint>) -> anyhow::Result<Vec<SimpleAudioEndpoint>> {
    ensure!(
        entries.len() <= 65536,
        "audio endpoint count exceeds replacement bound"
    );
    let mut records: Vec<SimpleAudioEndpoint> = Vec::new();
    let mut order: Vec<usize> = Vec::new();
    let mut hashes: Vec<u64> = Vec::new();
    let mut buckets = 0usize;
    for entry in entries {
        // Original insertion returns false for an existing ID. Its caller
        // fails initialization instead of accepting an ambiguous duplicate.
        ensure!(
            !records.iter().any(|old| old.id == entry.id),
            "duplicate audio endpoint ID"
        );
        if records.len() + 1 > buckets {
            let requested = (2 * buckets) | usize::from(buckets < 3 || !buckets.is_power_of_two());
            buckets = if requested == 1 {
                2
            } else if requested.is_power_of_two() {
                requested
            } else {
                next_prime(requested)
            };
            // Original rehash keeps a first group in place, moving subsequent
            // groups into that bucket immediately after its previous node.
            let mut seen = vec![None; buckets];
            if let Some(&first) = order.first() {
                let mut current = (hashes[first] % buckets as u64) as usize;
                seen[current] = Some(usize::MAX);
                let mut pos = 1;
                while pos < order.len() {
                    let id = order[pos];
                    let bucket = (hashes[id] % buckets as u64) as usize;
                    if bucket == current {
                        pos += 1;
                        continue;
                    }
                    if let Some(previous) = seen[bucket] {
                        let retained_previous = order[pos - 1];
                        order.remove(pos);
                        let insert = if previous == usize::MAX {
                            0
                        } else {
                            order.iter().position(|id| *id == previous).unwrap() + 1
                        };
                        order.insert(insert, id);
                        pos = order
                            .iter()
                            .position(|id| *id == retained_previous)
                            .unwrap()
                            + 1;
                    } else {
                        seen[bucket] = Some(order[pos - 1]);
                        current = bucket;
                        pos += 1;
                    }
                }
            }
        }
        let hash = source_hash(entry.id.as_bytes());
        let bucket = (hash % buckets as u64) as usize;
        let pos = order
            .iter()
            .position(|id| (hashes[*id] % buckets as u64) as usize == bucket)
            .unwrap_or(0);
        order.insert(pos, records.len());
        hashes.push(hash);
        records.push(entry);
    }
    let mut records = records.into_iter().map(Some).collect::<Vec<_>>();
    Ok(order
        .into_iter()
        .map(|id| records[id].take().unwrap())
        .collect())
}
