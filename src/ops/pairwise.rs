use crate::{impl_local_shared_both, prelude::*};

#[derive(Clone)]
pub struct PairwiseOp<S> {
  pub(crate) source: S,
}

impl<S: Observable> Observable for PairwiseOp<S> {
  type Item = (S::Item, S::Item);
  type Err = S::Err;
}

impl_local_shared_both! {
  impl<S> PairwiseOp<S>;
  type Unsub = S::Unsub;
  macro method($self: ident, $observer: ident, $ctx: ident) {
    $self
    .source
    .actual_subscribe(PairwiseObserver{
      observer: $observer,
      pair: (None, None),
    })
  }
  where
    S: @ctx::Observable,
    S::Item: Clone
      @ctx::shared_only(+ Send + Sync + 'static)
      @ctx::local_only(+ 'o)
}

#[derive(Clone)]
pub struct PairwiseObserver<O, Item> {
  observer: O,
  pair: (Option<Item>, Option<Item>),
}

impl<O, Item, Err> Observer for PairwiseObserver<O, Item>
where
  O: Observer<Item = (Item, Item), Err = Err>,
  Item: Clone,
{
  type Item = Item;
  type Err = Err;

  fn next(&mut self, value: Self::Item) {
    let (_x, y) = std::mem::take(&mut self.pair);
    self.pair = (y, Some(value));

    if let (Some(a), Some(b)) = &self.pair {
      self.observer.next((a.clone(), b.clone()));
    }
  }

  fn complete(&mut self) { self.observer.complete(); }

  fn error(&mut self, err: Self::Err) { self.observer.error(err) }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn smoke() {
    let expected = vec![
      (0, 1),
      (1, 2),
      (2, 3),
      (3, 4),
      (4, 5),
      (5, 6),
      (6, 7),
      (7, 8),
      (8, 9),
    ];
    let mut actual = vec![];
    observable::from_iter(0..10)
      .pairwise()
      .subscribe(|pair| actual.push(pair));

    assert_eq!(expected, actual);
  }
}
