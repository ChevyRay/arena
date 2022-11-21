# Compact Generational Arena

This crate provides `Arena<T>`, a contiguous growable container
which assigns and returns IDs to values when they are added to it.

These IDs can then be used to access their corresponding values at
any time, like an index, except that they remain valid even if other
items in the arena are removed or if the arena is sorted.

## Example

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

## Why Use This?

A common problem encountered in Rust is that the restrictions that
lifetimes put on references means they can't be used as freely as
references in languages like C# or JS, or pointers in C++.

### The Graph Problem

For example, in C# you could represent a graph of connected nodes
like this:

```cs
class Graph {
    List<Node> nodes;
}

class Node {
    List<Node> connections;
}
```

Trying to do this in Rust is much more difficult, because as soon
as a `Node` is trying to store a reference, the list of nodes is
no longer mutable. Beginners will often, after losing the battle with
lifetimes, try to resort to combinations ofsmart pointers such as
`Rc` and `RefCell` to make this work.

### The Alternative

A simple alternative to this is to not use references to the nodes
directly outside of the main list, but instead to refer to them by
their index in that list. For example:

```rust
struct Graph {
    nodes: Vec<Node>,
}

struct Node {
    connections: Vec<usize>,
}
```

This works fine, and is quite performant, until you need to be
able to remove nodes from the main list. As soon as you do this,
the indices of nodes have changed, and the connection lists can
easily and silently get invalidated.

Also, some use-cases require very large amounts of nodes, such as
a game engine where the graph might be made up of thousands of live
game objects, dozens being created and destroyed at any moment. In
this situation, constantly re-validating all the nodes and the
shifting node list can harm performance and creates a lot of places
for bugs to sneak through.

### Solving with a HashMap

One solution is to use a `HashMap`, where each node is assigned a
unique ID (maybe using something like [`uuid`](https://crates.io/crates/uuid)),
and then by only storing those IDs, you can bypass the reference
problem and access entities only when you need them.