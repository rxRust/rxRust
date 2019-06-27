/// Subscription returns from `Observable.subscribe(Subscriber)` to allow
///  unsubscribing.
pub trait Subscription<'a> {
  type Err;
  /// the action you have designed to accept any error notification from the
  ///  Observable
  fn on_error<E>(&mut self, err: E) -> &mut Self
  where
    E: Fn(&Self::Err) + 'a;
  /// the action you have designed to accept a completion notification from the
  ///  Observable
  fn on_complete<C>(&mut self, complete: C) -> &mut Self
  where
    C: Fn() + 'a;

  /// This allows deregistering an stream before it has finished receiving all
  ///  events (i.e. before onCompleted is called).
  fn unsubscribe(self);
}
