pub struct Subscription<'a>(pub Box<FnMut() + 'a>);

impl<'a> Subscription<'a> {
  pub fn unsubscribe(mut self) {
    (self.0)()
  }
}