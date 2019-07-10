use crate::prelude::*;

pub trait Multicast<'a> {
  type Item;
  type Err;
  fn multicast(self) -> Subject<'a, Self::Item, Self::Err>;
}

impl<'a, S> Multicast<'a> for S
where
  S: ImplSubscribable<'a> + 'a,
{
  type Item = S::Item;
  type Err = S::Err;

  fn multicast(self) -> Subject<'a, Self::Item, Self::Err> {
    Subject::from_stream(self)
  }
}
