/// `impl_local_shared_both` is a macro helper to implement both the
/// `LocalObservable` and `SharedObservable` at once. You can use
/// `@ctx` to unify the same meaning but different types in local and shared
/// implementation. See (`local`)[local] and ('shared')[shared] mod to learn
/// what type you can use.
/// Define an method macro use to write the method code of the `LocalObservable`
///  and `SharedObservable`. This macro should named with `method` and always
/// accept three arguments are `$self` `$observer` and `$ctx`. `$self` and
/// `$observer` are the arguments of the `actual_subscribe` method. `$ctx` is
/// either `impl_local` mod or `impl_shared`.
///
/// Here is the schema of the macro.
/// ```ignore
/// impl_local_shared_both! {
///   // First part is tell the macro what type you want implement for and the
///   // generics use to implement it.
///   impl$(<..>)? T$(<..>)?;
///   // This line specify the associtated type `Unsub`.
///   type Unsub = $ty;
///   // The macro use to implement the body of the method.
///   macro method($self: ident, $observer: ident, $ctx: ident) {
///     // write your `actual_subscribe` implementation in this block.
///   }
///   // if your implementation has some bounds, please provide a where clause
///   where $ident:$ty,
/// }
/// ```
///
/// Here is a real example which is the implementation of `ObservableFn`.
///
/// ```ignore
/// impl_local_shared_both! {
///   impl<F, Item, Err> ObservableFn<F, Item, Err>;
///   type Unsub = SingleSubscription;
///   macro method($self: ident, $observer: ident, $ctx: ident) {
///     ($self.0)(&mut $observer);
///     SingleSubscription::default()
///   }
///   where F: FnOnce(&mut dyn Observer<Item = Item, Err = Err>)
/// }
/// ```
#[macro_export]
macro_rules! impl_local_shared_both {
  (
    @replace$(+$i:literal)*, $ctx: ident,
    [$($before:tt)*] macro method($($args:tt)*) $body:tt $($t:tt)*
  ) => {
    impl_local_shared_both!{
      @replace$(+$i)*, $ctx,
      [
        $($before)*
        macro method($($args)*) $body
      ]
      $($t)*
    }
  };
  (
    @replace$(+$i:literal)*, impl_local,
    [$($before:tt)*] @ctx::Observable<$($ie:ident=$b:ty),+> $($t:tt)*
  ) => {
    impl_local_shared_both!(
      @replace$(+$i)*, impl_local,
      [$($before)* LocalObservable<'o, $($ie=$b),+> ] $($t)*
    );
  };
  (
    @replace$(+$i:literal)*, impl_local,
    [$($before:tt)*] @ctx::Observable $($t:tt)*
  ) => {
    impl_local_shared_both!(
      @replace$(+$i)*, impl_local,
      [$($before)* LocalObservable<'o> ] $($t)*
    );
  };
  (
    @replace$(+$i:literal)*, impl_shared,
    [$($before:tt)*] @ctx::Observable $($t:tt)*
  ) => {
    impl_local_shared_both!(
      @replace$(+$i)*, impl_shared,
      [$($before)* SharedObservable ] $($t)*
    );
  };
  (
    @replace$(+$i:literal)*, impl_local,
    [$($before:tt)*] @ctx::shared_only($($b:tt)*) $($t:tt)*
  ) => {
    impl_local_shared_both!(
      @replace$(+$i)*, impl_local,
      [$($before)* ] $($t)*
    );
  };
  (
    @replace$(+$i:literal)*, impl_shared,
    [$($before:tt)*] @ctx::shared_only($($b:tt)*) $($t:tt)*
  ) => {
    impl_local_shared_both!(
      @replace$(+$i)*, impl_shared,
      [$($before)* ] $($b)* $($t)*
    );
  };
  (
    @replace$(+$i:literal)*, impl_local,
    [$($before:tt)*] @ctx::local_only($($b:tt)*) $($t:tt)*
  ) => {
    impl_local_shared_both!(
      @replace$(+$i)*, impl_local,
      [$($before)* ] $($b)* $($t)*
    );
  };
  (
    @replace$(+$i:literal)*, impl_shared,
    [$($before:tt)*] @ctx::local_only($($b:tt)*) $($t:tt)*
  ) => {
    impl_local_shared_both!(
      @replace$(+$i)*, impl_shared,
      [$($before)* ] $($t)*
    );
  };
  // replace @ctx
  (
    @replace$(+$i:literal)*, $ctx: ident,
    [$($before:tt)*] @ctx $($t:tt)*
  ) => {
    impl_local_shared_both!{
      @replace$(+$i)*, $ctx,
      [$($before)* $ctx ] $($t)*
    }
  };

  // fix args invalid token.
  (
    @replace$(+$i:literal)*, $ctx: ident,
    [$($before:tt)*] ($arg1: ty, $($args: ty),+) $($t:tt)*
  ) => {
    impl_local_shared_both!{
      @replace$(+$i)*, $ctx,
      [
        $($before)*
        ($arg1, $($args)+)
      ]
      $($t)*
    }
  };
  // next token that group in () [] or {}.
  (
    @replace$(+$i:literal)*, $ctx: ident,
    [$($before:tt)*] ($($caller:tt)+) $($t:tt)*
  ) => {
    impl_local_shared_both!{
      @replace$(+$i)*, $ctx,
      [
        $($before)*
        (
          impl_local_shared_both!{
            @replace$(+$i)*+1, $ctx,
            []$($caller)+
          }
        )
      ]
      $($t)*
    }
  };
  (
    @replace$(+$i:literal)*, $ctx: ident,
    [$($before:tt)*] [$($caller:tt)+] $($t:tt)*
  ) => {
    impl_local_shared_both!{
      @replace$(+$i)*, $ctx,
      [
        $($before)*
        [
          impl_local_shared_both!{
            @replace$(+$i)*+1, $ctx,
            []$($caller)+
          }
        ]
      ]
      $($t)*
    };
  };
  (
    @replace$(+$i:literal)*, $ctx: ident,
    [$($before:tt)*] {$($caller:tt)+} $($t:tt)*
  ) => {
    impl_local_shared_both!{
      @replace$(+$i)*, $ctx,
      [
        $($before)*
        {
          impl_local_shared_both!{
            @replace$(+$i)*+1, $ctx,
            []$($caller)+
          }
        }
      ]
      $($t)*
      }
  };

// next token.
(
  @replace$(+$i:literal)*, $ctx: ident,
  [$($before:tt)*] $caller:tt $($t:tt)*
) => {
   impl_local_shared_both!{
     @replace$(+$i)*, $ctx,
     [$($before)* $caller ] $($t)*
   }
};
(
  @replace$(+$i:literal)+, $ctx: ident,
  [$($before:tt)*]
) => {
  $($before)*
};
  // done! all @ token is replaced and this is the top call.
  (
    @replace, $ctx: ident,
    [$($before:tt)*]
  ) => {
    impl_local_shared_both!(@$ctx, $($before)*);
  };
  (
    @impl_local,
    impl$(<$($ilf: lifetime,)* $($ig: ident),*>)?
      $($t: ident)::+ $(<$($lf: lifetime,)* $($g: ty),*>)?;
    type Unsub = $u: ty;
    macro method($($args:tt)*) $body:tt
    $(where $($b:tt)+)?
  ) => {
    impl<'o, $($($ilf,)* $($ig),*)?> LocalObservable<'o>
      for $($t)::+ $(<$($lf,)* $($g,)*>)?
    $(where $($b)+)?
    {
      type Unsub = $u;

      #[allow(unused_mut)]
      fn actual_subscribe<O>(self, mut _observer: O) -> Self::Unsub
      where
        O: Observer<Item = Self::Item, Err = Self::Err> + 'o,
      {
        macro_rules! method{
          ($($args)*) => {$body}
        }
        method!(self, _observer, impl_local)
      }
    }
  };
  (
    @impl_shared,
    impl$(<$($ilf: lifetime,)* $($ig: ident),*>)?
      $($t: ident)::+ $(<$($lf: lifetime,)* $($g: ty),*>)?;
    type Unsub = $u: ty;
    macro method($($args:tt)*) $body:tt
    $(where $($b:tt)+)?
  )=>{
    impl$(<$($ilf,)* $($ig),*>)? SharedObservable
      for $($t)::+ $(<$($lf,)* $($g,)*>)?
    $(where $($b)+)?
    {
      type Unsub = $u;

      #[allow(unused_mut)]
      fn actual_subscribe<O>(self, mut _observer: O) -> Self::Unsub
      where
        O: Observer<Item = Self::Item, Err = Self::Err> + Send + Sync + 'static,
      {
        macro_rules! method{
          ($($args)*) => {$body}
        }
        method!(self, _observer, impl_shared)
      }
    }
  };
  // enter replace
  ($($t:tt)*) => {
    impl_local_shared_both!(@replace, impl_local, [] $($t)*);
    #[cfg(not(all(target_arch = "wasm32")))]
    impl_local_shared_both!(@replace, impl_shared, [] $($t)*);
  };
}
pub mod impl_local {
  use crate::prelude::*;
  // macro builtin replace.
  pub type Observable = ();
  #[allow(dead_code, non_upper_case_globals)]
  const shared_only: () = ();
  #[allow(dead_code, non_upper_case_globals)]
  const local_only: () = ();

  pub type RcMultiSubscription = LocalSubscription;
  pub type Rc<T> = MutRc<T>;
  pub type Brc<T, Item, Err> = BufferedMutRc<T, Item, Err>;
  pub trait Scheduler: LocalScheduler {}
  impl<T: LocalScheduler> Scheduler for T {}

  #[inline]
  pub fn actual_subscribe<'a, S, O>(s: S, o: O) -> S::Unsub
  where
    S: LocalObservable<'a>,
    O: Observer<Item = S::Item, Err = S::Err> + 'a,
  {
    s.actual_subscribe(o)
  }
}

#[cfg(not(all(target_arch = "wasm32")))]
pub mod impl_shared {
  use crate::prelude::*;
  // macro builtin replace.
  pub type Observable = ();
  #[allow(dead_code, non_upper_case_globals)]
  const shared_only: () = ();
  #[allow(dead_code, non_upper_case_globals)]
  const local_only: () = ();

  pub type RcMultiSubscription = SharedSubscription;
  pub type Rc<T> = MutArc<T>;
  pub type Brc<T, Item, Err> = BufferedMutArc<T, Item, Err>;
  pub trait Scheduler: SharedScheduler {}
  impl<T: SharedScheduler> Scheduler for T {}

  #[inline]
  pub fn actual_subscribe<S, O>(s: S, o: O) -> S::Unsub
  where
    S: SharedObservable,
    O: Observer<Item = S::Item, Err = S::Err> + Send + Sync + 'static,
  {
    s.actual_subscribe(o)
  }
}
