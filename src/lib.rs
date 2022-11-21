use std::cmp::Ordering;
use std::ops::Deref;

#[derive(Debug)]
pub struct Arena<T> {
    values: Vec<T>,
    slots: Vec<Slot>,
    next_version: u64,
    first_free: Option<usize>,
}

impl<T> Arena<T> {
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            slots: Vec::new(),
            next_version: 0,
            first_free: None,
        }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            slots: Vec::with_capacity(capacity),
            next_version: 0,
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
                            version: self.next_version,
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
                        version: self.next_version,
                        value: self.values.len(),
                    },
                });
                index
            }
        };
        let id = ArenaId {
            version: self.next_version,
            index,
        };
        self.next_version += 1;
        self.values.push(create(id));
        id
    }

    pub fn remove(&mut self, id: ArenaId) -> Option<T> {
        let removed_value = {
            let slot = self.slots.get_mut(id.index)?;

            // get the slot of the removed value
            let value = match &slot.state {
                State::Used { version, value } if *version == id.version => *value,
                _ => return None,
            };

            // free up the value's slot
            slot.state = State::Free {
                next_free: self.first_free.replace(id.index),
            };

            value
        };

        // the last value has moved into the removed value's slot, so we need to move its value_slot as well
        self.slots[removed_value].value_slot = self.slots[self.values.len() - 1].value_slot;

        // pop + swap out the removed value
        Some(self.values.swap_remove(removed_value))
    }

    pub fn clear(&mut self) {
        if self.is_empty() {
            return;
        }
        for i in 0..self.values.len() {
            let slot = self.slots[i].value_slot;
            self.slots[slot].state = State::Free {
                next_free: self.first_free.replace(slot),
            };
        }
        self.values.clear();
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
    Used { version: u64, value: usize },
    Free { next_free: Option<usize> },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, Hash)]
pub struct ArenaId {
    version: u64,
    index: usize,
}

impl PartialOrd for ArenaId {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (self.version, self.index).partial_cmp(&(other.version, other.index))
    }
}
