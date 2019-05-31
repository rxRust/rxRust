/// In rx_rs, every extension has two version method. One version is use when no runtime error
/// will be propagated. This version receive an normal closure. The other is use when when will
/// propagating runtime error, named `err_with_xxx`, and receive an closure that return an
/// `Result` type, to detect if an runtime error occur.
///
/// In the inner of the extension, rx_rs not direct call the closure，but via the `NextWithErr` or
/// `NextWithoutErr` to call `call_and_consume_err` to execute the closure. 
/// rx_rs unify the behavior of the two version through `NextWithErr` and `NextWithoutErr`.
use crate::Subscription;

pub trait NextObserver<T, R> {
  type Err;
  fn call_and_consume_err<'a, S>(&self, v: &T, subscription: &S) -> Option<R>
  where
    S: Subscription<'a, Err = Self::Err>;
}

pub struct NextWhitoutError<N>(pub N);

impl<T, R, N> NextObserver<T, R> for NextWhitoutError<N>
where
  N: Fn(&T) -> R,
{
  type Err = ();
  fn call_and_consume_err<'a, S>(&self, v: &T, _subscription: &S) -> Option<R>
  where
    S: Subscription<'a, Err = Self::Err>,
  {
    Some(self.0(v))
  }
}


pub struct NextWithError<NE>(pub NE);
impl<T, R, N, E> NextObserver<T, R> for NextWithError<N>
where
  N: Fn(&T) -> Result<R, E>,
{
  type Err = E;
  fn call_and_consume_err<'a, S>(&self, v: &T, subscription: &S) -> Option<R>
  where
    S: Subscription<'a, Err = Self::Err>,
  {
    match self.0(v) {
      Ok(b) => Some(b),
      Err(err) => {
        subscription.throw_error(&err);
        None
      }
    }
  }
}
