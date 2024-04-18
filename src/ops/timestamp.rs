use super::map::MapOp;
use crate::prelude::Instant;

// Right now, timestamp is implemented as a map operation, so this is
// a simple typedef rather than a new implementation
pub type TimestampOp<S, Item> = MapOp<S, fn(Item) -> (Item, Instant), Item>;
