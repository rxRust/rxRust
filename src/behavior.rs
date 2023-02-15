use crate::observer::Observer;

pub trait Behavior<Item, Err>: Observer<Item, Err> {
  /// Get the value contained currently in the behavior
  ///
  /// Example:
  /// ```
  /// use rxrust::prelude::*;
  /// let mut behavior = BehaviorSubject::<i32, Subject::<_, _>>::new(0);
  /// behavior.clone()
  ///     .subscribe(|value| println!("{value}"));
  /// behavior.next(7);
  /// println!("{}", behavior.peek());
  ///
  /// // print log:
  /// // 0
  /// // 7
  /// // 7
  ///
  /// ```
  fn peek(&self) -> Item;

  /// Update the behavior's value based on its last one
  ///
  /// Example:
  /// ```
  /// use rxrust::prelude::*;
  /// let mut behavior = BehaviorSubject::<i32, Subject::<_, _>>::new(0);
  /// behavior.clone()
  ///     .subscribe(|value| println!("{value}"));
  /// for i in 0..3 {
  ///     behavior.next_by(|value| value + 1);
  /// }
  ///
  /// // print log:
  /// // 0
  /// // 1
  /// // 2
  ///
  /// ```
  fn next_by(&mut self, f: impl FnOnce(Item) -> Item) {
    let data = f(self.peek());
    self.next(data);
  }
}
