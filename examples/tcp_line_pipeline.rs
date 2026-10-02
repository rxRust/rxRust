//! Example: TCP line pipeline with pull-based backpressure
//!
//! A tiny metrics collector. A client writes `name=value` lines over TCP; the
//! server turns the socket into an observable with `from_stream`, so bytes
//! are only read as fast as the pipeline consumes them, then parses, drops
//! malformed lines, batches with `buffer_count`, and emits per-batch averages.
//!
//! Run with `cargo run --example tcp_line_pipeline`.

// The example needs tokio, which is not available on wasm32, so the whole
// program lives behind a target guard and wasm gets an empty `main`.
#[cfg(not(target_arch = "wasm32"))]
mod native {
  use std::convert::Infallible;

  use futures::stream;
  use rxrust::prelude::*;
  use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
  };

  /// The script the client sends; the second line is deliberately malformed.
  pub const SCRIPT: &[&str] =
    &["cpu=10", "bad line", "cpu=20", "mem=5", "mem=7", "cpu=30", "mem=9"];

  /// Parses `name=value` into `(name, value)`, returning `None` for junk.
  pub fn parse(line: String) -> Option<(String, f64)> {
    let (name, value) = line.split_once('=')?;
    Some((name.trim().to_string(), value.trim().parse().ok()?))
  }

  /// Average of a batch of parsed metrics.
  pub fn average(batch: &[(String, f64)]) -> f64 {
    batch.iter().map(|(_, v)| v).sum::<f64>() / batch.len() as f64
  }

  /// Turns one accepted socket into a `Stream` of lines, read on demand.
  fn lines_of(socket: TcpStream) -> impl futures::Stream<Item = String> + Send {
    stream::unfold(BufReader::new(socket).lines(), |mut lines| async move {
      match lines.next_line().await {
        Ok(Some(line)) => Some((line, lines)),
        _ => None,
      }
    })
  }

  /// Runs a server and a client, returning the batch averages the pipeline
  /// produced.
  pub async fn run() -> Vec<f64> {
    let listener = TcpListener::bind("127.0.0.1:0")
      .await
      .expect("bind");
    let addr = listener.local_addr().expect("addr");

    // The client: writes the script and closes the connection.
    let client = tokio::spawn(async move {
      let mut socket = TcpStream::connect(addr).await.expect("connect");
      for line in SCRIPT {
        socket
          .write_all(format!("{line}\n").as_bytes())
          .await
          .expect("write");
      }
      socket.shutdown().await.expect("shutdown");
    });

    let (socket, _) = listener.accept().await.expect("accept");

    // The server: socket -> lines -> metrics -> batches -> averages.
    let averages = Shared::from_stream(lines_of(socket))
      .filter_map(parse)
      .buffer_count(3)
      .map(|batch: Vec<(String, f64)>| average(&batch))
      .tap(|avg| println!("batch average: {avg:.2}"))
      .collect::<Vec<f64>>()
      .into_future()
      .await
      .expect("pipeline")
      .unwrap_or_else(|never: Infallible| match never {});

    client.await.expect("client task");
    averages
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[tokio::test]
    async fn pipeline_batches_parsed_metrics() {
      let averages = run().await;

      // Expected from the same script: 6 valid metrics in batches of 3.
      let valid: Vec<(String, f64)> = SCRIPT
        .iter()
        .filter_map(|l| parse(l.to_string()))
        .collect();
      let expected: Vec<f64> = valid.chunks(3).map(average).collect();

      assert_eq!(averages, expected);
      assert_eq!(averages.len(), 2);
    }
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
  let averages = native::run().await;
  println!("done: {averages:?}");
}

#[cfg(target_arch = "wasm32")]
fn main() {}
