# Compact Generational Arena

This crate provides `Arena<T>`, a contiguous growable container
which assigns and returns IDs to values when they are added to it.

These IDs can then be used to access their corresponding values at
any time, like an index, except that they remain valid even if other
items in the arena are removed or if the arena is sorted.

- faster insersion, removal, and lookup speed than a `HashMap`
- values are stored in a contiguous vector you can access/iterate
- memory and cache efficient: uses only 2 heap-allocated vectors

## Examples

```rust
use arena::Arena;

// create a new empty arena
let mut arena = Arena::new();

// add values to it and store their returned IDs
let a = arena.insert('A');
let b = arena.insert('B');
let c = arena.insert('C');
let d = arena.insert('D');

// we can use those IDs to fetch the values
assert_eq!(arena.get(a), Some(&'A'));
assert_eq!(arena.get(b), Some(&'B'));
assert_eq!(arena.get(c), Some(&'C'));
assert_eq!(arena.get(d), Some(&'D'));

// the values live in a contiguous vector we can access
assert_eq!(arena.as_slice(), &['A', 'B', 'C', 'D']);

// we can remove a value from anywhere using its ID
arena.remove(b);

// the value at the end will fill the hole left by the removed one
assert_eq!(arena.as_slice(), &['A', 'D', 'C']);

// even though 'D' moved, its ID is still valid and can be used
assert_eq!(arena.get(a), Some(&'A'));
assert_eq!(arena.get(b), Some(&'B'));
assert_eq!(arena.get(c), None);
assert_eq!(arena.get(d), Some(&'D'));

// we can even sort the values to order them
arena.sort();

// and all the IDs will still be valid
assert_eq!(arena.as_slice(), &['A', 'B', 'D']);
assert_eq!(arena.get(a), Some(&'A'));
assert_eq!(arena.get(b), Some(&'B'));
assert_eq!(arena.get(c), None);
assert_eq!(arena.get(d), Some(&'D'));
```