use crate::prelude::*;

#[derive(Clone)]
pub struct PairwiseOp<S> {
  pub(crate) source: S,
}

impl<Item, Err, O, S> Observable<(Item, Item), Err, O> for PairwiseOp<S>
where
  S: Observable<Item, Err, PairwiseObserver<O, Item>>,
  O: Observer<(Item, Item), Err>,
  Item: Clone,
{
  type Unsub = S::Unsub;

  fn actual_subscribe(self, observer: O) -> Self::Unsub {
    self
      .source
      .actual_subscribe(PairwiseObserver { observer, pair: (None, None) })
  }
}

impl<Item, Err, S> ObservableExt<(Item, Item), Err> for PairwiseOp<S> where
  S: ObservableExt<Item, Err>
{
}

#[derive(Clone)]
pub struct PairwiseObserver<O, Item> {
  observer: O,
  pair: (Option<Item>, Option<Item>),
}

impl<O, Item, Err> Observer<Item, Err> for PairwiseObserver<O, Item>
where
  O: Observer<(Item, Item), Err>,
  Item: Clone,
{
  fn next(&mut self, value: Item) {
    let (_x, y) = std::mem::take(&mut self.pair);
    self.pair = (y, Some(value));

    if let (Some(a), Some(b)) = &self.pair {
      self.observer.next((a.clone(), b.clone()));
    }
  }

  #[inline]
  fn complete(self) {
    self.observer.complete();
  }

  #[inline]
  fn error(self, err: Err) {
    self.observer.error(err)
  }

  #[inline]
  fn is_finished(&self) -> bool {
    self.observer.is_finished()
  }
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
