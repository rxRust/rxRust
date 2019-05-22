/// In rx_rs, every extension has two version method. One version is use when no runtime error
/// will be propagated. This version receive an normal closure. The other is use when when will
/// propagating run time error, named `err_with_xxx`, and receive an closure that return an
/// `Result` type, to detect if an runtime error occur.
///
/// In the inner of the extension, rx_rs not direct call the closure，but via the `NextWithErr` or
/// `NextWithoutErr` to call `call_with_err` to execute the closure. `NextWithErr` and `NextWithoutErr`
/// rx_rs unify the behavior of the two version through `NextWithErr` and `NextWithoutErr`.
pub trait NextWithErr<Item, Output, Err> {
  fn call_with_err(&mut self, v: Item) -> Result<Output, Err>;
}

pub trait NextWithoutErr<Item, Output> {
  fn call_with_err<Err>(&mut self, v: Item) -> Result<Output, Err>;
}

impl<T, U, F> NextWithoutErr<T, U> for F
where
  F: Fn(T) -> U,
{
  fn call_with_err<E>(&mut self, v: T) -> Result<U, E> {
    Ok(self(v))
  }
}

impl<T, U, E, F> NextWithErr<T, U, E> for F
where
  F: Fn(T) -> Result<U, E>,
{
  fn call_with_err(&mut self, v: T) -> Result<U, E> {
    self(v)
  }
}

pub enum OnNextClousre<N, NE> {
  Next(N),
  NextWithErr(NE),
}

impl<N, NE> OnNextClousre<N, NE> {
  pub fn call_with_err<T, U, E>(&self, v: T) -> Result<U, E>
  where
    N: Fn(T) -> U,
    NE: Fn(T) -> Result<U, E>,
  {
    match self {
      OnNextClousre::Next(n) => Ok(n(v)),
      OnNextClousre::NextWithErr(ne) => ne(v),
    }
  }
}