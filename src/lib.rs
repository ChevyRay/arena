use std::cmp::Ordering;
use std::mem::replace;
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

    #[inline]
    pub fn insert(&mut self, value: T) -> ArenaId {
        self.insert_with(|_| value)
    }

    pub fn insert_with<F>(&mut self, create: F) -> ArenaId
    where
        F: FnOnce(ArenaId) -> T,
    {
        let index = match self.first_free.take() {
            // if there is a free slot available, assign the value to it
            Some(index) => {
                let slot = &mut self.slots[index];
                match slot.state.clone() {
                    State::Free { next_free } => {
                        self.first_free = next_free;
                        slot.value_slot = index;
                        slot.state = State::Used {
                            id: self.next_id,
                            value: self.values.len(),
                        };
                        index
                    }
                    _ => panic!("expected free slot"),
                }
            }

            // if there is no free slot available, assign the value to a new one
            None => {
                let index = self.slots.len();
                self.slots.push(Slot {
                    value_slot: index,
                    state: State::Used {
                        id: self.next_id,
                        value: self.values.len(),
                    },
                });
                index
            }
        };
        let id = ArenaId {
            id: self.next_id,
            index,
        };
        self.next_id += 1;
        self.values.push(create(id));
        id
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

#[derive(Debug, Clone)]
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
