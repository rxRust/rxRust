use std::cell::RefCell;
use std::rc::Rc;

pub fn unwrap_rc_ref_cell<T>(pointer: Rc<RefCell<T>>, expect: &str) -> T {
  Rc::try_unwrap(pointer).ok().expect(expect).into_inner()
}
