//! ZAP wire benchmarks — encode + decode of representative messages.
//!
//! Compared against a JSON baseline to quantify the binary win. Runs
//! on a single core; multiply by core count for a server estimate.

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use hanzo_zap::schema::{keys_snapshot, keys_subscribe};

fn bench_keys_subscribe_encode(c: &mut Criterion) {
    let mut g = c.benchmark_group("encode/keys_subscribe");
    g.throughput(Throughput::Elements(1));
    g.bench_function("zap", |b| {
        b.iter(|| {
            let bytes = keys_subscribe::Writer {
                application: black_box("liquidity".to_string()),
                cursor: black_box(42),
            }
            .to_bytes();
            black_box(bytes);
        })
    });
    g.bench_function("json", |b| {
        b.iter(|| {
            let s = format!(
                r#"{{"application":"{}","cursor":{}}}"#,
                black_box("liquidity"),
                black_box(42u64)
            );
            black_box(s.into_bytes());
        })
    });
    g.finish();
}

fn bench_keys_subscribe_decode(c: &mut Criterion) {
    let bytes = keys_subscribe::Writer {
        application: "liquidity".to_string(),
        cursor: 42,
    }
    .to_bytes();
    let json = br#"{"application":"liquidity","cursor":42}"#;

    let mut g = c.benchmark_group("decode/keys_subscribe");
    g.throughput(Throughput::Elements(1));
    g.bench_function("zap", |b| {
        b.iter(|| {
            let r = keys_subscribe::Reader::from_slice(black_box(&bytes)).unwrap();
            black_box((r.application(), r.cursor()));
        })
    });
    g.bench_function("json_naive_scan", |b| {
        // Apples-to-apples: a hand-rolled minimal JSON-ish scan that's
        // still allocating a String for the payload. Real JSON parsers
        // (serde) are slower; we're being generous to the baseline.
        b.iter(|| {
            let s = std::str::from_utf8(black_box(json)).unwrap();
            let app_start = s.find("\"application\":\"").unwrap() + 15;
            let app_end = s[app_start..].find('"').unwrap() + app_start;
            let app = &s[app_start..app_end];
            let cur_start = s.find("\"cursor\":").unwrap() + 9;
            let cur_end = s[cur_start..]
                .find(|c: char| !c.is_ascii_digit())
                .map(|i| cur_start + i)
                .unwrap_or(s.len());
            let cur: u64 = s[cur_start..cur_end].parse().unwrap();
            black_box((app, cur));
        })
    });
    g.finish();
}

fn bench_keys_snapshot_encode_with_payload(c: &mut Criterion) {
    let jwks = include_bytes!("./fixtures_jwks.json");
    let mut g = c.benchmark_group("encode/keys_snapshot_2k_jwks");
    g.throughput(Throughput::Bytes(jwks.len() as u64));
    g.bench_function("zap", |b| {
        b.iter(|| {
            let bytes = keys_snapshot::Writer {
                application: "liquidity".to_string(),
                seq: 1,
                jwks_json: jwks.to_vec(),
            }
            .to_bytes();
            black_box(bytes);
        })
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_keys_subscribe_encode,
    bench_keys_subscribe_decode,
    bench_keys_snapshot_encode_with_payload
);
criterion_main!(benches);
