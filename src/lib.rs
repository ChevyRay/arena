use std::cmp::Ordering;
use std::ops::Deref;

#[derive(Debug)]
pub struct Arena<T> {
    values: Vec<T>,
    slots: Vec<Slot>,
    next_id: u64,
    first_free: Option<usize>,
}

impl<T> Arena<T> {
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            slots: Vec::new(),
            next_id: 0,
            first_free: None,
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            slots: Vec::with_capacity(capacity),
            next_id: 0,
            first_free: None,
        }
    }

    #[inline]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    #[inline]
    pub fn free_slot_count(&self) -> usize {
        self.slot_count() - self.len()
    }
}

impl<T> Deref for Arena<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.values.as_slice()
    }
}

#[derive(Debug)]
struct Slot {
    value_slot: usize,
    state: State,
}

#[derive(Debug)]
enum State {
    Used { id: u64, value: usize },
    Free { next_free: Option<usize> },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, Hash)]
pub struct ArenaId {
    id: u64,
    index: usize,
}

impl PartialOrd for ArenaId {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (self.id, self.index).partial_cmp(&(other.id, other.index))
    }
}
