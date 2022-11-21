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

/// A contiguous growable container which assigns and returns IDs to values when
/// they are added to it.
///
/// These IDs can then be used to access their corresponding values at any time,
/// like an index, except that they remain valid even if other items in the arena
/// are removed or if the arena is sorted.
///
/// A big advantage of this collection over something like a [`HashMap`](std::collections::HashMap)
/// is that, since the values are stored in contiguous memory, you can access this
/// slice [directly](Arena::as_slice) and get all the benefits that you would from
/// having an array or a [`Vec`], such as parallel iterators with [`rayon`](https://crates.io/crates/rayon).
///
/// # Examples
///
/// ```
/// use arena::Arena;
///
/// // create an arena and add 3 values to it
/// let mut arena = Arena::new();
/// let a = arena.insert('A');
/// let b = arena.insert('B');
/// let c = arena.insert('C');
///
/// // we can access the slice of values directly
/// assert_eq!(arena.as_slice(), &['A', 'B', 'C']);
///
/// // or we can use the returned IDs to access them
/// assert_eq!(arena.get(a), Some(&'A'));
/// assert_eq!(arena.get(b), Some(&'B'));
/// assert_eq!(arena.get(c), Some(&'C'));
///
/// // remove a value from the middle
/// arena.remove(b);
///
/// // the slice now only has the remaining values
/// assert_eq!(arena.as_slice(), &['A', 'C']);
///
/// // even though `C` changed position, its ID is still valid
/// assert_eq!(arena.get(a), Some(&'A'));
/// assert_eq!(arena.get(b), None);
/// assert_eq!(arena.get(c), Some(&'C'));
///
/// // IDs are copyable so they can be passed around easily
/// let some_id = c;
/// assert_eq!(arena.get(some_id), Some(&'C'));
/// ```
///
/// # Iteration
///
/// Because arena implements [`Deref<Target = [T]>`](Arena::deref), you can iterate over
/// the values in the contiguous slice directly:
///
/// ```
/// # use arena::Arena;
/// let mut arena = Arena::from(['A', 'B', 'C']);
///
/// let mut iter = arena.iter();
/// assert_eq!(Some(&'A'), iter.next());
/// assert_eq!(Some(&'B'), iter.next());
/// assert_eq!(Some(&'C'), iter.next());
/// ```
///
/// Alternatively, you can iterate over ID/value pairs:
///
/// ```
/// # use arena::Arena;
/// let mut arena = Arena::new();
/// let a = arena.insert('A');
/// let b = arena.insert('B');
/// let c = arena.insert('C');
///
/// let mut pairs = arena.pairs();
/// assert_eq!(Some((a, &'A')), pairs.next());
/// assert_eq!(Some((b, &'B')), pairs.next());
/// assert_eq!(Some((c, &'C')), pairs.next());
/// ```
///
/// Or iterate over just the IDs:
///
/// ```
/// # use arena::Arena;
/// # let mut arena = Arena::new();
/// # let a = arena.insert('A');
/// # let b = arena.insert('B');
/// # let c = arena.insert('C');
/// let mut ids = arena.ids();
/// assert_eq!(Some(a), ids.next());
/// assert_eq!(Some(b), ids.next());
/// assert_eq!(Some(c), ids.next());
/// ```
///
/// # Performance
///
/// Lookups by ID are only slightly slower than indexing into a [`Vec`], and like
/// a vector they do not take longer even when the collection grows. To provide this
/// ability, though, adding and removing from the arena has more overhead than a vector.
///
/// To keep removal fast, the arena uses a "pop & swap" method to remove values, meaning
/// the last value will get moved into the removed value's position. The ID of that value
/// will then get remapped to prevent it from being invalidated. Because of this, you
/// should never assume the values or IDs in an arena remain in the order you added them.
#[derive(Debug, Clone)]
pub struct Arena<T> {
    values: Vec<T>,
    slots: Vec<Slot>,
    next_version: u64,
    first_free: Option<usize>,
}

impl<T> Arena<T> {
    /// Constructs a new, empty `Arena<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// # use arena::Arena;
    /// let mut arena: Arena<String> = Arena::new();
    /// ```
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            slots: Vec::new(),
            next_version: 1,
            first_free: None,
        }
    }

    /// Constructs a new, empty `Arena<T>` with at least the specified capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// # use arena::Arena;
    /// let mut arena: Arena<String> = Arena::with_capacity(1000);
    /// ```
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            slots: Vec::with_capacity(capacity),
            next_version: 1,
            first_free: None,
        }
    }

    /// Returns `true` if the arena contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::new();
    /// assert!(arena.is_empty());
    ///
    /// arena.insert('A');
    /// assert!(!arena.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the amount of slots the arena is using to map IDs.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::from(['A', 'B', 'C']);
    ///
    /// assert_eq!(arena.len(), 3);
    /// assert_eq!(arena.slot_count(), 3);
    ///
    /// arena.clear();
    ///
    /// assert_eq!(arena.len(), 0);
    /// assert_eq!(arena.slot_count(), 3);
    /// ```
    #[inline]
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Returns the amount of empty slots the arena has. New values added to
    /// the arena will make use of these slots instead of creating new ones.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::from(['A', 'B', 'C']);
    ///
    /// assert_eq!(arena.slot_count(), 3);
    /// assert_eq!(arena.free_slot_count(), 0);
    ///
    /// let _ = arena.pop();
    ///
    /// assert_eq!(arena.slot_count(), 3);
    /// assert_eq!(arena.free_slot_count(), 1);
    /// ```
    #[inline]
    pub fn free_slot_count(&self) -> usize {
        self.slot_count() - self.len()
    }

    /// Extracts a slice containing all the arena's values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::from(['A', 'B', 'C']);
    ///
    /// assert_eq!(arena.as_slice(), &['A', 'B', 'C']);
    ///
    /// let _ = arena.pop();
    ///
    /// assert_eq!(arena.as_slice(), &['A', 'B']);
    /// ```
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.values.as_slice()
    }

    /// Extracts a mutable slice containing all the arena's values.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena: Arena<i32> = Arena::from([1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(arena.as_mut_slice(), &[1, 2, 3, 4, 5]);
    ///
    /// for num in arena.as_mut_slice() {
    ///     *num += 1;
    /// }
    ///
    /// assert_eq!(arena.as_mut_slice(), &[2, 3, 4, 5, 6]);
    ///
    /// ```
    ///
    /// # Warning
    ///
    /// Re-arranging the values in this mutable slice will invalidate the IDs given
    /// when they were added to the arena. It is recommended only to use this slice
    /// for modifying the values in place, either in sequence or in parallel (for
    /// example, with the `rayon` library).
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.values.as_mut_slice()
    }

    /// Returns an unsafe mutable pointer to the arena's value buffer, or a dangling
    /// raw pointer valid for zero sized reads if the arena didn't allocate.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.values.as_mut_ptr()
    }

    /// Returns a reference to the value assigned with the ID, or `None` if the
    /// value was removed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::new();
    /// let a = arena.insert('A');
    /// let b = arena.insert('B');
    /// let c = arena.insert('C');
    ///
    /// assert_eq!(arena.get(a), Some(&'A'));
    /// assert_eq!(arena.get(b), Some(&'B'));
    /// assert_eq!(arena.get(c), Some(&'C'));
    ///
    /// arena.remove(b);
    ///
    /// assert_eq!(arena.get(a), Some(&'A'));
    /// assert_eq!(arena.get(b), None);
    /// assert_eq!(arena.get(c), Some(&'C'));
    /// ```
    #[inline]
    pub fn get(&self, id: ArenaId) -> Option<&T> {
        match &self.slots.get(id.index)?.state {
            State::Used { version, value } if *version == id.version => Some(&self.values[*value]),
            _ => None,
        }
    }

    /// Returns a mutable reference to the value assigned with the ID, or `None`
    /// if the value was removed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::new();
    /// let a = arena.insert('A');
    /// let b = arena.insert('B');
    ///
    /// assert_eq!(arena.as_slice(), &['A', 'B']);
    ///
    /// if let Some(a_val) = arena.get_mut(a) {
    ///     *a_val = 'B';
    /// }
    ///
    /// if let Some(b_val) = arena.get_mut(b) {
    ///     *b_val = 'A';
    /// }
    ///
    /// assert_eq!(arena.as_slice(), &['B', 'A']);
    /// ```
    #[inline]
    pub fn get_mut(&mut self, id: ArenaId) -> Option<&mut T> {
        match &self.slots.get(id.index)?.state {
            State::Used { version, value } if *version == id.version => {
                Some(&mut self.values[*value])
            }
            _ => None,
        }
    }

    /// Returns true if the arena contains a value assigned with the ID.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::new();
    /// let a = arena.insert('A');
    /// let b = arena.insert('B');
    /// let c = arena.insert('C');
    ///
    /// assert!(arena.contains(a));
    /// assert!(arena.contains(b));
    /// assert!(arena.contains(c));
    ///
    /// arena.remove(a);
    ///
    /// assert!(!arena.contains(a));
    /// assert!(arena.contains(b));
    /// assert!(arena.contains(c));
    /// ```
    #[inline]
    pub fn contains(&self, id: ArenaId) -> bool {
        self.get(id).is_some()
    }

    /// Returns the ID assigned to the value at the corresponding index, or
    /// `None` if the index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::new();
    /// let a = arena.insert('A');
    /// let b = arena.insert('B');
    /// let c = arena.insert('C');
    ///
    /// assert_eq!(arena.id_at(0), Some(a));
    /// assert_eq!(arena.id_at(1), Some(b));
    /// assert_eq!(arena.id_at(2), Some(c));
    ///
    /// println!("{:#?}", arena);
    ///
    /// arena.remove(b);
    ///
    /// println!("{:#?}", arena);
    ///
    /// assert_eq!(arena.id_at(0), Some(a));
    /// assert_eq!(arena.id_at(1), Some(c));
    /// assert_eq!(arena.id_at(2), None);
    ///
    /// ```
    #[inline]
    pub fn id_at(&self, index: usize) -> Option<ArenaId> {
        if index >= self.len() {
            return None;
        }
        let slot = self.slots.get(index)?.value_slot;
        match &self.slots[slot].state {
            State::Used { version, value } if *value == index => Some(ArenaId {
                version: *version,
                index: slot,
            }),
            _ => None,
        }
    }

    /// Returns the index of the value corresponding to the ID if it is in the arena.
    ///
    /// # Examples
    ///
    /// ```
    /// # use arena::Arena;
    /// let mut arena = Arena::from(['A', 'B', 'C', 'D']);
    /// let e = arena.insert('E');
    ///
    /// arena.remove_at(3);
    ///
    ///
    /// ```
    #[inline]
    pub fn index_of(&self, id: ArenaId) -> Option<usize> {
        match &self.slots.get(id.index)?.state {
            State::Used { version, .. } if *version == id.version => Some(id.index),
            _ => None,
        }
    }

    /// Inserts a value in the arena, returning an ID that can be used to
    /// access the value at a later time, even if the values were re-arranged.
    #[inline]
    pub fn insert(&mut self, value: T) -> ArenaId {
        self.insert_with(|_| value)
    }

    /// Inserts a value, created by the provided function, to the arena. The
    /// function is passed the ID assigned to the value, which is useful if
    /// the values themselves want to store the IDs on construction.
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

    /// Removes the value from the arena assigned to the ID. If the value existed
    /// in the arena, it will be returned.
    pub fn remove(&mut self, id: ArenaId) -> Option<T> {
        let to_slot = {
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

        // the value that was at the back is now in the removed spot
        let from_slot = self.values.len() - 1;
        self.slots[to_slot].value_slot = self.slots[from_slot].value_slot;
        match &mut self.slots[from_slot].state {
            State::Used { value, .. } => *value = to_slot,
            _ => unreachable!(),
        }

        // pop + swap out the removed value
        Some(self.values.swap_remove(to_slot))
    }

    /// Removes the value at the specified index.
    pub fn remove_at(&mut self, index: usize) -> Option<T> {
        self.remove(self.id_at(index)?)
    }

    /// Pops a value off the end of the arena and returns it.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        let value = self.values.pop()?;
        let slot = self.slots[self.values.len()].value_slot;
        self.slots[slot].state = State::Free {
            next_free: self.first_free.replace(slot),
        };
        Some(value)
    }

    fn clear_opt(&mut self, clear_slots: bool) {
        if self.is_empty() {
            return;
        }

        if clear_slots {
            self.slots.clear();
            self.first_free = None;
        } else {
            for i in 0..self.values.len() {
                let slot = self.slots[i].value_slot;
                self.slots[slot].state = State::Free {
                    next_free: self.first_free.replace(slot),
                };
            }
        }

        self.values.clear();
    }

    /// Clears all values from the arena.
    pub fn clear(&mut self) {
        self.clear_opt(false);
    }

    /// Clears all values and slots from the arena.
    pub fn clear_all(&mut self) {
        self.clear_opt(true);
    }

    /// Swaps the position of the two values corresponding to the provided IDs without
    /// invalidating them.
    #[inline]
    pub fn swap_positions(&mut self, i: ArenaId, j: ArenaId) -> bool {
        if let Some(i) = self.index_of(i) {
            if let Some(j) = self.index_of(j) {
                self.swap(i, j);
                return true;
            }
        }
        false
    }

    /// Swaps values from the two positions in the arena without invalidating their IDS.
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

    /// Sorts the values in the arena, using the provided function, without
    /// invalidating their IDs.
    #[inline]
    pub fn sort_by<F: FnMut(&T, &T) -> Ordering>(&mut self, mut compare: F) {
        if self.len() > 1 {
            self.quicksort(0, self.len() - 1, &mut compare);
        }
    }

    /// Returns an iterator that allows modifying each value.
    ///
    /// The iterator yields all items from start to end.
    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.values.iter_mut()
    }

    /// Returns an iterator over all ID/value pairs in the arena.
    #[inline]
    pub fn pairs(&self) -> Pairs<'_, T> {
        Pairs {
            iter: self.values.iter().enumerate(),
            slots: &self.slots,
        }
    }

    /// Returns a mutable iterator over all ID/value pairs in the arena.
    #[inline]
    pub fn pairs_mut(&mut self) -> PairsMut<'_, T> {
        PairsMut {
            iter: self.values.iter_mut().enumerate(),
            slots: &self.slots,
        }
    }

    /// Returns an iterator over all IDs in the arena.
    #[inline]
    pub fn ids(&self) -> Ids<'_> {
        Ids {
            iter: self.slots[..self.len()].iter().enumerate(),
        }
    }
}

impl<T: Clone> Arena<T> {
    /// Adds all values from the slice to the arena.
    #[inline]
    pub fn extend_from_slice(&mut self, slice: &[T]) {
        self.values.reserve(slice.len());
        self.extend(slice.iter().cloned());
    }
}

impl<T: Ord> Arena<T> {
    /// Sorts the values in the arena, without invalidating their IDs.
    #[inline]
    pub fn sort(&mut self) {
        self.sort_by(|a, b| a.cmp(b));
    }
}

impl<T> Default for Arena<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Deref for Arena<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.values.as_slice()
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

impl<T> Extend<T> for Arena<T> {
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for val in iter {
            self.insert(val);
        }
    }
}

impl<'a, T: Clone + 'a> Extend<&'a T> for Arena<T> {
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.extend(iter.into_iter().cloned())
    }
}

impl<T> From<Vec<T>> for Arena<T> {
    fn from(values: Vec<T>) -> Self {
        let mut slots = Vec::new();
        let mut version = 0;
        for i in 0..values.len() {
            slots.push(Slot {
                value_slot: i,
                state: State::Used { version, value: i },
            });
            version += 1;
        }
        Self {
            values,
            slots,
            first_free: None,
            next_version: version,
        }
    }
}

impl<'a, T: Clone + 'a> From<&'a [T]> for Arena<T> {
    #[inline]
    fn from(values: &'a [T]) -> Self {
        Self::from_iter(values.iter().cloned())
    }
}

impl<'a, T: Clone + 'a> From<&'a mut [T]> for Arena<T> {
    #[inline]
    fn from(values: &'a mut [T]) -> Self {
        Self::from_iter(values.iter().cloned())
    }
}

impl<T, const N: usize> From<[T; N]> for Arena<T> {
    #[inline]
    fn from(values: [T; N]) -> Self {
        Self::from(Vec::from(values))
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

#[derive(Debug, Clone)]
struct Slot {
    value_slot: usize,
    state: State,
}

#[derive(Debug, Clone)]
enum State {
    Used { version: u64, value: usize },
    Free { next_free: Option<usize> },
}

/// An ID assigned to a value when it was added to an arena.
///
/// Unlike an index, this ID will remain a valid handle to the value even
/// if other values are removed from the arena and the value vector gets
/// re-ordered.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ArenaId {
    version: u64,
    index: usize,
}

impl PartialOrd for ArenaId {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ArenaId {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        (self.version, self.index).cmp(&(other.version, other.index))
    }
}

/// Iterator over an arena's ID/value pairs.
///
/// This struct is created by the [`pairs`](Arena::pairs) method on [`Arena`].
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

/// Mutable iterator over an arena's ID/value pairs.
///
/// This struct is created by the [`pairs_mut`](Arena::pairs_mut) method on [`Arena`].
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

/// Iterator over an arena's IDs.
///
/// This struct is created by the [`ids`](Arena::ids) method on [`Arena`].
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
