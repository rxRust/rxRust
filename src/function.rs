use std::borrow::Borrow;
use std::sync::Arc;

#[repr(transparent)]
pub struct RxFnWrapper<T>(T);

impl<F> RxFnWrapper<F> {
  #[inline(always)]
  pub fn new(f: F) -> Self { Self(f) }
}

pub trait RxFn<Args> {
  type Output;
  extern "rust-call" fn call(&self, args: Args) -> Self::Output;
}

impl<F, Args> RxFn<Args> for RxFnWrapper<F>
where
  F: Fn<Args>,
{
  type Output = F::Output;
  #[inline(always)]
  extern "rust-call" fn call(&self, args: Args) -> Self::Output { self.0.call(args) }
}

impl<F, Args> RxFn<Args> for Arc<F>
where
  F: RxFn<Args>,
{
  type Output = F::Output;
  #[inline(always)]
  extern "rust-call" fn call(&self, args: Args) -> Self::Output {
    let f: &F = self.borrow();
    f.call(args)
  }
}

#[test]
#[should_panic]
fn test() {
  let f = RxFnWrapper(|_x: &'static str| panic!("boooom!"));
  f.call(("",));
}
