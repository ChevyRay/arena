use std::cmp::Ordering;
use std::ops::{Deref, Index, IndexMut};

#[test]
fn test() {
    let mut arena = Arena::new();

    let d = arena.insert('D');
    let c = arena.insert('C');
    let b = arena.insert('B');
    let e = arena.insert('E');
    let a = arena.insert('A');

    println!("{:?}", arena.iter().collect::<Vec<_>>());

    println!("{}", arena[a]);
    println!("{}", arena[b]);
    println!("{}", arena[c]);
    println!("{}", arena[d]);
    println!("{}", arena[e]);

    arena.sort();

    println!("{:?}", arena.iter().collect::<Vec<_>>());

    println!("{}", arena[a]);
    println!("{}", arena[b]);
    println!("{}", arena[c]);
    println!("{}", arena[d]);
    println!("{}", arena[e]);
}

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
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
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
    pub fn as_slice(&self) -> &[T] {
        self.values.as_slice()
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.values.as_mut_slice()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.values.as_mut_ptr()
    }

    #[inline]
    pub fn get(&self, id: ArenaId) -> Option<&T> {
        match &self.slots.get(id.index)?.state {
            State::Used { version, value } if *version == id.version => Some(&self.values[*value]),
            _ => None,
        }
    }

    #[inline]
    pub fn get_mut(&mut self, id: ArenaId) -> Option<&mut T> {
        match &self.slots.get(id.index)?.state {
            State::Used { version, value } if *version == id.version => {
                Some(&mut self.values[*value])
            }
            _ => None,
        }
    }

    #[inline]
    pub fn contains(&self, id: ArenaId) -> bool {
        self.get(id).is_some()
    }

    #[inline]
    pub fn id_at(&self, index: usize) -> Option<ArenaId> {
        let slot = self.slots.get(index)?.value_slot;
        match &self.slots[slot].state {
            State::Used { version, .. } => Some(ArenaId {
                version: *version,
                index,
            }),
            _ => None,
        }
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
        let last_slot = self.slots[self.values.len() - 1].value_slot;
        match &mut self.slots[last_slot].state {
            State::Used { value, .. } => *value = id.index,
            _ => panic!("invalid value_slot"),
        }

        // pop + swap out the removed value
        Some(self.values.swap_remove(removed_value))
    }

    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        let value = self.values.pop()?;
        let slot = self.slots[self.values.len()].value_slot;
        self.slots[slot].state = State::Free {
            next_free: self.first_free.replace(slot),
        };
        Some(value)
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

    #[inline]
    pub fn swap(&mut self, i: usize, j: usize) {
        self.values.swap(i, j);
        let slot_i = self.slots[i].value_slot;
        let slot_j = self.slots[j].value_slot;
        match &mut self.slots[slot_i] {
            Slot {
                value_slot,
                state: State::Used { value, .. },
            } => {
                *value_slot = slot_j;
                *value = j;
            }
            _ => panic!("invalid slot"),
        };
        match &mut self.slots[slot_j] {
            Slot {
                value_slot,
                state: State::Used { value, .. },
            } => {
                *value_slot = slot_i;
                *value = i;
            }
            _ => panic!("invalid slot"),
        };
    }

    fn quicksort<F: FnMut(&T, &T) -> Ordering>(
        &mut self,
        low: usize,
        high: usize,
        compare: &mut F,
    ) {
        if low + 1 >= high.wrapping_add(1) {
            return;
        }
        let p = {
            let (mut i, mut j) = (low, low);
            while i <= high {
                if compare(&self.values[i], &self.values[high]) == Ordering::Greater {
                    i += 1;
                } else {
                    self.swap(i, j);
                    i += 1;
                    j += 1;
                }
            }
            j - 1
        };
        self.quicksort(low, p.wrapping_sub(1), compare);
        self.quicksort(p + 1, high, compare);
    }

    #[inline]
    pub fn sort_by<F: FnMut(&T, &T) -> Ordering>(&mut self, mut compare: F) {
        if self.len() > 1 {
            self.quicksort(0, self.len() - 1, &mut compare);
        }
    }

    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.values.iter_mut()
    }

    #[inline]
    pub fn pairs(&self) -> Pairs<'_, T> {
        Pairs {
            iter: self.values.iter().enumerate(),
            slots: &self.slots,
        }
    }

    #[inline]
    pub fn pairs_mut(&mut self) -> PairsMut<'_, T> {
        PairsMut {
            iter: self.values.iter_mut().enumerate(),
            slots: &self.slots,
        }
    }

    #[inline]
    pub fn ids(&self) -> Ids<'_> {
        Ids {
            iter: self.slots[..self.len()].iter().enumerate(),
        }
    }
}

impl<T: Clone> Arena<T> {
    #[inline]
    pub fn extend_from_slice(&mut self, slice: &[T]) {
        self.values.reserve(slice.len());
        self.extend(slice.iter().cloned());
    }
}

impl<T: Ord> Arena<T> {
    #[inline]
    pub fn sort(&mut self) {
        self.sort_by(|a, b| a.cmp(b));
    }
}

impl<T> Deref for Arena<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.values.as_slice()
    }
}

impl<T> Default for Arena<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Index<ArenaId> for Arena<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: ArenaId) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T> IndexMut<ArenaId> for Arena<T> {
    #[inline]
    fn index_mut(&mut self, index: ArenaId) -> &mut Self::Output {
        self.get_mut(index).unwrap()
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

pub struct Pairs<'a, T> {
    iter: std::iter::Enumerate<std::slice::Iter<'a, T>>,
    slots: &'a [Slot],
}

impl<'a, T> Iterator for Pairs<'a, T> {
    type Item = (ArenaId, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (index, val) = self.iter.next()?;
        let index = self.slots[index].value_slot;
        match &self.slots[index].state {
            State::Used { version, .. } => Some((
                ArenaId {
                    version: *version,
                    index,
                },
                val,
            )),
            _ => panic!("expected used slot"),
        }
    }
}

pub struct PairsMut<'a, T> {
    iter: std::iter::Enumerate<std::slice::IterMut<'a, T>>,
    slots: &'a [Slot],
}

impl<'a, T> Iterator for PairsMut<'a, T> {
    type Item = (ArenaId, &'a mut T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (index, val) = self.iter.next()?;
        let index = self.slots[index].value_slot;
        match &self.slots[index].state {
            State::Used { version, .. } => Some((
                ArenaId {
                    version: *version,
                    index,
                },
                val,
            )),
            _ => panic!("expected used slot"),
        }
    }
}

pub struct Ids<'a> {
    iter: std::iter::Enumerate<std::slice::Iter<'a, Slot>>,
}

impl<'a> Iterator for Ids<'a> {
    type Item = ArenaId;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (index, slot) = self.iter.next()?;
        match &slot.state {
            State::Used { version, .. } => Some(ArenaId {
                version: *version,
                index,
            }),
            _ => None,
        }
    }
}

impl<T> Extend<T> for Arena<T> {
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for val in iter {
            self.insert(val);
        }
    }
}

impl<T> IntoIterator for Arena<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

impl<T> FromIterator<T> for Arena<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut arena = Arena::new();
        arena.extend(iter.into_iter());
        arena
    }
}
