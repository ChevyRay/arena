use crate::ArenaId;
use std::ops::Deref;

#[derive(Debug)]
pub struct CompactArena<T> {
    values: Vec<T>,
    slot_indices: Vec<usize>,
    slots: Vec<Slot>,
    first_free_slot: Option<usize>,
    next_uid: u64,
}

#[derive(Debug)]
enum Slot {
    Used { uid: u64, value_index: usize },
    Free { next_free_slot: Option<usize> },
}

impl<T> CompactArena<T> {
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            slot_indices: Vec::new(),
            slots: Vec::new(),
            first_free_slot: None,
            next_uid: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            slot_indices: Vec::with_capacity(capacity),
            slots: Vec::with_capacity(capacity),
            first_free_slot: None,
            next_uid: 0,
        }
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn free_slot_count(&self) -> usize {
        self.slot_count() - self.len()
    }

    pub fn insert(&mut self, value: T) -> ArenaId {
        self.insert_with(|_| value)
    }

    pub fn insert_with<F>(&mut self, create: F) -> ArenaId
    where
        F: FnOnce(ArenaId) -> T,
    {
        let slot_index = match self.first_free_slot.take() {
            Some(index) => {
                self.first_free_slot = match &self.slots[index] {
                    Slot::Free { next_free_slot } => *next_free_slot,
                    _ => panic!("used slot in free list"),
                };
                self.slots[index] = Slot::Used {
                    value_index: self.values.len(),
                    uid: self.next_uid,
                };
                index
            }
            None => {
                self.slots.push(Slot::Used {
                    uid: 0,
                    value_index: self.values.len(),
                });
                self.slots.len() - 1
            }
        };
        let id = ArenaId {
            index: slot_index,
            version: self.next_uid,
        };
        self.next_uid += 1;
        self.values.push(create(id));
        self.slot_indices.push(slot_index);
        id
    }

    pub fn remove(&mut self, id: ArenaId) -> Option<T> {
        // get the index of the value
        let value_index = match self.slots.get(id.index) {
            Some(Slot::Used { uid, value_index }) if *uid == id.version => *value_index,
            _ => return None,
        };

        // free up the value's slot
        self.slots[id.index] = Slot::Free {
            next_free_slot: self.first_free_slot.replace(id.index),
        };

        // move the last slot's position into the removed value's position
        let last_index = *self.slot_indices.last().unwrap();
        self.slots[last_index] = match self.slots[last_index] {
            Slot::Used { uid, .. } => Slot::Used { uid, value_index },
            _ => unreachable!(),
        };

        // pop + swap out the removed value
        let value = self.values.swap_remove(value_index);
        self.slot_indices.swap_remove(value_index);

        Some(value)
    }

    pub fn clear(&mut self) {
        if self.is_empty() {
            return;
        }
        for &i in &self.slot_indices {
            self.slots[i] = Slot::Free {
                next_free_slot: self.first_free_slot.replace(i),
            };
        }
        self.values.clear();
        self.slot_indices.clear();
    }
}

impl<T> Deref for CompactArena<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.values.as_slice()
    }
}
