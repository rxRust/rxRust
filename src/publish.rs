use crate::observer::Observer;

pub trait Publish<Item, Err>: Observer<Item, Err> {
}
