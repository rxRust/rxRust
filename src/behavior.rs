pub trait Behavior {
    /// The type of the item tracked by the behavior
    type Item<'a> where Self: 'a;

    /// Get the value contained currently in the behavior
    ///
    /// Example:
    /// ```
    /// use rxrust::prelude::*;
    /// let mut behavior = SharedBehaviorSubject::new(0);
    /// behavior.clone()
    ///     .into_shared()
    ///     .subscribe(|value| println!("{value}"));
    /// behavior.next(7);
    /// println!("{}", behavior.peek())
    ///
    /// // print log:
    /// // 0
    /// // 7
    /// // 7
    ///
    /// ```
    fn peek(&self) -> Self::Item<'_>;
}