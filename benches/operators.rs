//! Micro-benchmarks for the operator core, run with `cargo bench`.
//!
//! `bencher` prints libtest-style `bench: N ns/iter` lines on stable, which
//! CI feeds to github-action-benchmark to track regressions per commit.

use std::{
  convert::Infallible,
  sync::{Arc, Mutex},
};

use bencher::{Bencher, benchmark_group, benchmark_main, black_box};
use rxrust::prelude::*;

const N: i64 = 1_000;

fn local_map_filter(b: &mut Bencher) {
  b.iter(|| {
    let mut sum = 0i64;
    Local::from_iter(0..N)
      .map(|x| x * 2)
      .filter(|x| x % 3 == 0)
      .subscribe(|x| sum += x);
    black_box(sum)
  });
}

fn shared_map_filter(b: &mut Bencher) {
  b.iter(|| {
    let sum = Arc::new(Mutex::new(0i64));
    let sink = sum.clone();
    Shared::from_iter(0..N)
      .map(|x| x * 2)
      .filter(|x| x % 3 == 0)
      .subscribe(move |x| *sink.lock().unwrap() += x);
    black_box(*sum.lock().unwrap())
  });
}

fn local_scan_take(b: &mut Bencher) {
  b.iter(|| {
    let mut last = 0i64;
    Local::from_iter(0..N)
      .scan(0i64, |acc, x| acc + x)
      .take(500)
      .subscribe(|x| last = x);
    black_box(last)
  });
}

fn local_flat_map_small_inners(b: &mut Bencher) {
  b.iter(|| {
    let mut count = 0usize;
    Local::from_iter(0..100i64)
      .flat_map(|x| Local::from_iter(x..x + 10))
      .subscribe(|_| count += 1);
    black_box(count)
  });
}

fn subject_broadcast_ten_subscribers(b: &mut Bencher) {
  let mut subject = Local::subject::<i64, Infallible>();
  let counters: Vec<_> = (0..10)
    .map(|_| {
      let counter = std::rc::Rc::new(std::cell::Cell::new(0i64));
      let sink = counter.clone();
      subject
        .clone()
        .subscribe(move |x| sink.set(sink.get() + x));
      counter
    })
    .collect();
  b.iter(|| {
    for i in 0..N {
      subject.next(i);
    }
    black_box(counters[0].get())
  });
}

fn local_merge_two_sources(b: &mut Bencher) {
  b.iter(|| {
    let mut count = 0usize;
    Local::from_iter(0..N)
      .merge(Local::from_iter(0..N))
      .subscribe(|_| count += 1);
    black_box(count)
  });
}

fn local_collect_to_vec(b: &mut Bencher) {
  b.iter(|| {
    let mut len = 0usize;
    Local::from_iter(0..N)
      .to_vec()
      .subscribe(|v| len = v.len());
    black_box(len)
  });
}

benchmark_group!(
  operators,
  local_map_filter,
  shared_map_filter,
  local_scan_take,
  local_flat_map_small_inners,
  subject_broadcast_ten_subscribers,
  local_merge_two_sources,
  local_collect_to_vec
);
benchmark_main!(operators);
