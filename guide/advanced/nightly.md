# Nightly (Experimental)

rxRust targets **stable Rust** by default.

This page documents a small set of **nightly-only experimental capabilities**
used to explore advanced type-system use cases.

## Lifetime-dependent mapped outputs (why stable `map` is limited)

rxRust's `ObservableType` uses a GAT (`type Item<'a>`) so an observable can emit
values that *borrow* from a scope in a controlled way (this is essential for
patterns like `subject_mut_ref` / `multicast_mut_ref`).

However, on **stable Rust** there is an important limitation when defining
operators like `map`:

- The stable-friendly signature is typically written as
  `F: for<'a> FnMut(S::Item<'a>) -> Out`.
- This makes `Out` a *single fixed type*, which means the mapped output type
  cannot depend on the input lifetime `'a`.

So mappings that conceptually look like `&'a T -> &'a U` (or `&'a mut T -> &'a U`)
cannot be expressed through the stable `map` API, because it would require a
type family `Out<'a>`.

## Experimental support in `map` (nightly only)

rxRust provides an **experimental** implementation of `map` behind a Cargo
feature flag:

- Feature: `nightly`
- Requires a nightly compiler

Enable it like this:

```toml
[dependencies]
rxrust = { version = "1.0.0-rc.0", features = ["nightly"] }
```

And build/tests with nightly:

```bash
cargo +nightly test --features nightly
```

With the nightly feature enabled, `map` can be used in patterns where the output
borrows from the input (example uses `subject_mut_ref`):

```rust,ignore
use rxrust::prelude::*;
use std::convert::Infallible;

fn as_ref<'a>(v: &'a mut i32) -> &'a i32 { v }

let subject = Local::subject_mut_ref::<i32, Infallible>();
subject.map(as_ref).subscribe(|v: &i32| {
    // v is tied to the emission's lifetime
    assert!(*v >= 0 || *v < 0);
});
```

## Why we don't roll this out to every operator

We intentionally keep this behavior **limited to `map`** for now:

- It relies on nightly-only features and subtle type inference behavior.
- Duplicating this across many operators increases maintenance cost
  significantly.
- When the relevant capabilities become stable, we can support lifetime-dependent
  output types across more operators with a single, maintainable design.
