#![cfg_attr(feature = "nightly", feature(fn_traits, unboxed_closures))]
//! Reactive extensions library for Rust: a library for
//! [Reactive Programming](http://reactivex.io/) using
//! [Observable](crate::observable::Observable), to make
//! it easier to compose asynchronous or callback-based code.
#[cfg_attr(not(target_arch = "wasm32"), doc = include_str!("../README.md"))]
#[cfg(test)]
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn __rxrust_wasm_init() { console_error_panic_hook::set_once(); }

// Main modules (formerly v1)
pub mod context;
pub mod factory;
pub mod observable;
pub mod observer;
pub mod ops;
pub mod prelude;
pub mod rc;
pub mod scheduler;
pub mod subject;
pub mod subscription;

// Re-export the prelude module
pub use prelude::*;

pub use crate::scheduler::{Duration, Instant};
#[cfg(feature = "scheduler")]
pub use crate::scheduler::{LocalScheduler, SharedScheduler};

// Bring external markdown files (README + guide/**/*.md) into Cargo-driven
// doctests. This module is only compiled for rustdoc doctest builds.

#[cfg(all(doctest, not(target_arch = "wasm32")))]
mod __markdown_doctests {
  // Split into per-file modules so doctest failures point at a meaningful
  // module name (instead of everything collapsing into `__markdown_doctests`).
  mod readme {
    #![doc = include_str!("../README.md")]
  }

  mod guide_summary {
    #![doc = include_str!("../guide/SUMMARY.md")]
  }

  mod guide_introduction {
    #![doc = include_str!("../guide/introduction.md")]
  }

  mod guide_getting_started {
    #![doc = include_str!("../guide/getting_started.md")]
  }

  mod guide_core_concepts {
    #![doc = include_str!("../guide/core_concepts.md")]
  }

  mod guide_core_concepts_context {
    #![doc = include_str!("../guide/core_concepts/context.md")]
  }

  mod guide_core_concepts_scheduler {
    #![doc = include_str!("../guide/core_concepts/scheduler.md")]
  }

  mod guide_core_concepts_type_erasure {
    #![doc = include_str!("../guide/core_concepts/type_erasure.md")]
  }

  mod guide_async_interop {
    #![doc = include_str!("../guide/async_interop.md")]
  }

  mod guide_cookbook {
    #![doc = include_str!("../guide/cookbook.md")]
  }

  mod guide_cookbook_gui {
    #![doc = include_str!("../guide/cookbook/gui.md")]
  }

  mod guide_cookbook_wasm {
    #![doc = include_str!("../guide/cookbook/wasm.md")]
  }

  mod guide_cookbook_auto_save {
    #![doc = include_str!("../guide/cookbook/auto_save.md")]
  }

  mod guide_cookbook_heartbeat {
    #![doc = include_str!("../guide/cookbook/heartbeat.md")]
  }

  mod guide_cookbook_state_store {
    #![doc = include_str!("../guide/cookbook/state_store.md")]
  }

  mod guide_operators {
    #![doc = include_str!("../guide/operators.md")]
  }

  mod guide_advanced {
    #![doc = include_str!("../guide/advanced.md")]
  }

  mod guide_advanced_architecture_deep_dive {
    #![doc = include_str!("../guide/advanced/architecture_deep_dive.md")]
  }

  mod guide_advanced_custom_scheduling {
    #![doc = include_str!("../guide/advanced/custom_scheduling.md")]
  }

  mod guide_advanced_custom_operators {
    #![doc = include_str!("../guide/advanced/custom_operators.md")]
  }

  mod guide_contributing {
    #![doc = include_str!("../guide/contributing.md")]
  }
}
