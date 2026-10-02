//! Example: debounced search with cancellation of stale requests
//!
//! Keystrokes arrive on a `Subject`. The pipeline waits for a quiet period
//! (`debounce`), ignores repeated queries (`distinct_until_changed`), and
//! runs the search with `switch_map`, which cancels an in-flight search the
//! moment a newer query arrives.
//!
//! Run with `cargo run --example debounced_search`.

// The example needs tokio, which is not available on wasm32, so the whole
// program lives behind a target guard and wasm gets an empty `main`.
#[cfg(not(target_arch = "wasm32"))]
mod native {
  use std::{cell::RefCell, convert::Infallible, rc::Rc, time::Duration};

  use rxrust::prelude::*;

  /// What the example observed.
  #[derive(Debug, Default, PartialEq, Eq)]
  pub struct Outcome {
    /// Queries for which a search was started
    pub searches_started: Vec<String>,
    /// Result sets that reached the subscriber, tagged by query
    pub results_delivered: Vec<(String, Vec<String>)>,
  }

  /// A slow, fake search backend.
  async fn search(query: String, started: Rc<RefCell<Vec<String>>>) -> (String, Vec<String>) {
    started.borrow_mut().push(query.clone());
    tokio::time::sleep(Duration::from_millis(100)).await;
    let hits = ["rxrust", "rust", "reactive"]
      .iter()
      .filter(|h| h.starts_with(&query))
      .map(|h| h.to_string())
      .collect();
    (query, hits)
  }

  pub async fn run() -> Outcome {
    let started = Rc::new(RefCell::new(Vec::new()));
    let delivered = Rc::new(RefCell::new(Vec::new()));
    let mut keystrokes = Local::subject::<String, Infallible>();

    let started_for_search = started.clone();
    let delivered_sink = delivered.clone();
    let _sub = keystrokes
      .clone()
      .debounce(Duration::from_millis(30))
      .distinct_until_changed()
      .tap(|q| println!("query after quiet period: {q:?}"))
      .switch_map(move |q| Local::from_future(search(q, started_for_search.clone())))
      .subscribe(move |(query, hits)| {
        println!("results for {query:?}: {hits:?}");
        delivered_sink.borrow_mut().push((query, hits));
      });

    // Type "rxrust" quickly: only the final query survives the debounce.
    for prefix in ["r", "rx", "rxr", "rxru", "rxrust"] {
      keystrokes.next(prefix.to_string());
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // Wait past the debounce so the search for "rxrust" starts, then change
    // the query while that search is still running: switch_map abandons it.
    tokio::time::sleep(Duration::from_millis(50)).await;
    keystrokes.next("rust".to_string());
    tokio::time::sleep(Duration::from_millis(250)).await;

    Outcome {
      searches_started: started.borrow().clone(),
      results_delivered: delivered.borrow().clone(),
    }
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[tokio::test(flavor = "local")]
    async fn debounce_collapses_typing_and_switch_map_cancels_stale_search() {
      let outcome = run().await;

      // One search per settled query
      assert_eq!(outcome.searches_started, vec!["rxrust", "rust"]);
      // Only the latest search delivered results; the first was cancelled
      assert_eq!(outcome.results_delivered, vec![("rust".to_string(), vec!["rust".to_string()])]);
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main(flavor = "local")]
async fn main() {
  let outcome = native::run().await;
  println!("{outcome:#?}");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
