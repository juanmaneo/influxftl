// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! A model of the iteration order of `java.util.HashMap` (and `HashSet`).
//!
//! Some Warp 10 results are Java hash maps whose iteration order leaks into WarpScript (the
//! maps a pickle decodes to, the maps a pickle is written from, the order of the rings of
//! the outline of a `GEOSHAPE`). [`JavaHashMap`] reproduces that order: buckets indexed by the
//! spread hash code, insertion order within a bucket, the table doubling when the size exceeds
//! three quarters of the capacity, and the bins that grow past 8 entries: they make the table
//! grow until it has 64 buckets, then become red-black trees, which iterate from their root and
//! insert a new entry after its parent in the tree.
//!
//! The trees order the keys by their hash code, then by [`JavaHashMap::with_order`] (what
//! `Comparable.compareTo` says in Java). Java breaks the ties of keys that are not comparable
//! with the identity hash code of the objects, which no model can follow: such keys go left.

use std::cmp::Ordering;

/// `HashMap.MAXIMUM_CAPACITY`.
const MAXIMUM_CAPACITY: usize = 1 << 30;

/// `HashMap.TREEIFY_THRESHOLD`: the number of entries in a bin that makes it a tree.
const TREEIFY_THRESHOLD: usize = 8;

/// `HashMap.UNTREEIFY_THRESHOLD`: the number of entries at or below which a split tree is a
/// list again.
const UNTREEIFY_THRESHOLD: usize = 6;

/// `HashMap.MIN_TREEIFY_CAPACITY`: the bins of a smaller table make it grow instead.
const MIN_TREEIFY_CAPACITY: usize = 64;

/// What `compareTo` says of two keys, `None` when `compareTo` does not apply to them (the
/// keys are not `Comparable`, or not of the same class).
pub type Order<K> = fn(&K, &K) -> Option<Ordering>;

/// The order of the keys that have none: Java breaks their ties with the identity hash code.
fn unordered<K>(_: &K, _: &K) -> Option<Ordering> {
    None
}

/// The `compareTo` of `Integer`, `Long` and `String` (for the ASCII strings of the
/// callers).
pub fn ordered<K: Ord>(a: &K, b: &K) -> Option<Ordering> {
    Some(a.cmp(b))
}

/// An entry: the `hashCode` of the key, the key and the value.
type Entry<K, V> = (i32, K, V);

/// `HashMap.tableSizeFor`.
pub fn table_size_for(capacity: usize) -> usize {
    capacity.clamp(1, MAXIMUM_CAPACITY).next_power_of_two()
}

/// `Long.hashCode`: the high 32 bits xor the low 32 bits.
pub fn long_hash(value: i64) -> i32 {
    (value ^ ((value as u64) >> 32) as i64) as i32
}

/// `HashMap.hash`: the spread hash code.
fn spread(hash: i32) -> i32 {
    let h = hash as u32;
    (h ^ (h >> 16)) as i32
}

/// The sentinel node of the trees: the black node that stands for the missing children and parent
/// of `HashMap.TreeNode` (its `null`). Its colour is never changed; its other links are written
/// where Java tests for `null`, and never read.
const NIL: usize = 0;

/// The links of a node of a tree bin: its place in the tree (`parent`, `left`, `right`), its place
/// in the iteration order of the bin (`prev`, `next`) and its colour. The default is the sentinel's.
#[derive(Clone, Copy, Debug, Default)]
struct Link {
    parent: usize,
    left: usize,
    right: usize,
    prev: usize,
    next: usize,
    red: bool,
}

/// The side of a node: the left or the right child of its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

impl Side {
    fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// A bin made a red-black tree (`HashMap.TreeNode`). Node `n` (`n >= 1`) holds `entries[n - 1]`
/// and has the links `links[n]`. A removal leaves its node in the vectors, out of reach: a tree
/// that gets too small to be a tree is converted to a list, and dropped.
#[derive(Clone, Debug)]
struct Tree<K, V> {
    entries: Vec<Entry<K, V>>,
    links: Vec<Link>,
    /// The root (always black).
    root: usize,
    /// The head of the bin: where the iteration starts.
    first: usize,
}

impl<K, V> Tree<K, V> {
    /// `TreeNode.treeify` of a list of entries, which `HashMap.treeifyBin` and the split of a
    /// table doubling make into a tree. The list is not empty.
    fn from_list(entries: Vec<Entry<K, V>>, order: Order<K>) -> Self {
        let count = entries.len();
        let links = std::iter::once(Link::default())
            .chain((0..count).map(|position| Link {
                prev: position,
                next: if position + 1 < count {
                    position + 2
                } else {
                    NIL
                },
                ..Link::default()
            }))
            .collect();
        let mut tree = Tree {
            entries,
            links,
            root: NIL,
            first: 1,
        };
        tree.treeify(order);
        tree
    }

    fn entry(&self, node: usize) -> &Entry<K, V> {
        &self.entries[node - 1]
    }

    fn entry_mut(&mut self, node: usize) -> &mut Entry<K, V> {
        &mut self.entries[node - 1]
    }

    fn child(&self, node: usize, side: Side) -> usize {
        match side {
            Side::Left => self.links[node].left,
            Side::Right => self.links[node].right,
        }
    }

    fn set_child(&mut self, node: usize, side: Side, child: usize) {
        match side {
            Side::Left => self.links[node].left = child,
            Side::Right => self.links[node].right = child,
        }
    }

    /// The side of its parent that `node` is on.
    fn side_of(&self, node: usize) -> Side {
        if self.links[self.links[node].parent].left == node {
            Side::Left
        } else {
            Side::Right
        }
    }

    /// `TreeNode.treeify`: the nodes are inserted in iteration order, then the root moves to the
    /// front of the bin.
    fn treeify(&mut self, order: Order<K>) {
        self.root = self.first;
        let mut x = self.links[self.first].next;
        while x != NIL {
            let next = self.links[x].next;
            let entry = self.entry(x);
            let (parent, side) = self.place(spread(entry.0), &entry.1, order);
            self.links[x].parent = parent;
            self.set_child(parent, side, x);
            self.balance_insertion(x);
            x = next;
        }
        self.move_root_to_front();
    }

    /// The node under which a key of spread hash `h` goes, and the side it goes to.
    fn place(&self, h: i32, key: &K, order: Order<K>) -> (usize, Side) {
        let mut p = self.root;
        loop {
            let side = self.direction(p, h, key, order);
            match self.child(p, side) {
                NIL => return (p, side),
                child => p = child,
            }
        }
    }

    /// The side of the node `p` a key of spread hash `h` goes to: by the hash, then by `order`.
    /// Keys that `order` does not tell apart go left (Java breaks that tie with the identity hash
    /// codes, which no model follows).
    fn direction(&self, p: usize, h: i32, key: &K, order: Order<K>) -> Side {
        let node = self.entry(p);
        match spread(node.0).cmp(&h) {
            Ordering::Greater => Side::Left,
            Ordering::Less => Side::Right,
            Ordering::Equal => match order(key, &node.1) {
                Some(Ordering::Greater) => Side::Right,
                _ => Side::Left,
            },
        }
    }

    /// `TreeNode.moveRootToFront`: the root heads the iteration order of the bin.
    fn move_root_to_front(&mut self) {
        let root = self.root;
        if root == self.first {
            return;
        }
        let Link { prev, next, .. } = self.links[root];
        self.links[next].prev = prev;
        self.links[prev].next = next;
        self.links[self.first].prev = root;
        self.links[root].next = self.first;
        self.links[root].prev = NIL;
        self.first = root;
    }

    /// `TreeNode.rotateLeft` (`side` is `Left`) or `rotateRight`: the child of `p` on the other
    /// side rises in its place, and `p` moves down on `side`.
    fn rotate(&mut self, p: usize, side: Side) {
        let risen = self.child(p, side.other());
        let moved = self.child(risen, side);
        self.set_child(p, side.other(), moved);
        self.links[moved].parent = p;
        let pp = self.links[p].parent;
        self.links[risen].parent = pp;
        if pp == NIL {
            self.root = risen;
            self.links[risen].red = false;
        } else {
            let pp_side = self.side_of(p);
            self.set_child(pp, pp_side, risen);
        }
        self.set_child(risen, side, p);
        self.links[p].parent = risen;
    }

    /// `TreeNode.balanceInsertion` of the new red node `x`: recolouring and rotations until the
    /// rules hold again.
    fn balance_insertion(&mut self, mut x: usize) {
        self.links[x].red = true;
        loop {
            let xp = self.links[x].parent;
            if xp == NIL {
                self.links[x].red = false;
                self.root = x;
                return;
            }
            if !self.links[xp].red {
                return;
            }
            // A red node is never the root: its parent exists.
            let xpp = self.links[xp].parent;
            let xp_side = self.side_of(xp);
            let uncle = self.child(xpp, xp_side.other());
            if self.links[uncle].red {
                self.links[uncle].red = false;
                self.links[xp].red = false;
                self.links[xpp].red = true;
                x = xpp;
                continue;
            }
            // An inner child is rotated outwards first: it is then the one that rises.
            let (xp, xpp) = if x == self.child(xp, xp_side.other()) {
                self.rotate(xp, xp_side);
                (x, xpp)
            } else {
                (xp, xpp)
            };
            self.links[xp].red = false;
            self.links[xpp].red = true;
            self.rotate(xpp, xp_side.other());
            return;
        }
    }

    /// `TreeNode.balanceDeletion` of the black node `x`, which a removal leaves one black short:
    /// recolouring and rotations, the shortage moving up while the sibling has no red child.
    fn balance_deletion(&mut self, mut x: usize) {
        loop {
            if x == self.root {
                return;
            }
            if self.links[x].red {
                self.links[x].red = false;
                return;
            }
            let xp = self.links[x].parent;
            let x_side = self.side_of(x);
            let mut sibling = self.child(xp, x_side.other());
            if self.links[sibling].red {
                self.links[sibling].red = false;
                self.links[xp].red = true;
                self.rotate(xp, x_side);
                sibling = self.child(xp, x_side.other());
            }
            // The children of the sibling: `near` is on the side of `x`, `far` is not.
            let near = self.child(sibling, x_side);
            let far = self.child(sibling, x_side.other());
            if !self.links[near].red && !self.links[far].red {
                self.links[sibling].red = true;
                x = xp;
                continue;
            }
            let far = if self.links[far].red {
                far
            } else {
                self.links[near].red = false;
                self.links[sibling].red = true;
                self.rotate(sibling, x_side.other());
                sibling = self.child(xp, x_side.other());
                self.child(sibling, x_side.other())
            };
            self.links[sibling].red = self.links[xp].red;
            self.links[far].red = false;
            self.links[xp].red = false;
            self.rotate(xp, x_side);
            return;
        }
    }

    /// `TreeNode.find`: the node of `key`, whose spread hash is `h`, below `start`.
    fn find(&self, start: usize, h: i32, key: &K, order: Order<K>) -> Option<usize>
    where
        K: PartialEq,
    {
        let mut at = start;
        while at != NIL {
            let Link { left, right, .. } = self.links[at];
            let entry = self.entry(at);
            at = match spread(entry.0).cmp(&h) {
                Ordering::Greater => left,
                Ordering::Less => right,
                Ordering::Equal if entry.1 == *key => return Some(at),
                Ordering::Equal => match (left, right) {
                    (NIL, right) => right,
                    (left, NIL) => left,
                    (left, right) => match order(key, &entry.1) {
                        Some(Ordering::Less) => left,
                        Some(Ordering::Greater) => right,
                        _ => match self.find(right, h, key, order) {
                            Some(found) => return Some(found),
                            None => left,
                        },
                    },
                },
            };
        }
        None
    }

    /// `TreeNode.putTreeVal` of a key the tree does not have: the new node comes after its parent
    /// in the iteration order.
    fn insert(&mut self, entry: Entry<K, V>, order: Order<K>) {
        let (parent, side) = self.place(spread(entry.0), &entry.1, order);
        let x = self.links.len();
        let next = self.links[parent].next;
        self.entries.push(entry);
        self.links.push(Link {
            parent,
            prev: parent,
            next,
            ..Link::default()
        });
        self.set_child(parent, side, x);
        self.links[parent].next = x;
        self.links[next].prev = x;
        self.balance_insertion(x);
        self.move_root_to_front();
    }

    /// The two children case of a removal: the successor `s` of `p` (the leftmost node of its
    /// right subtree) takes the place of `p` in the tree, and the two exchange their colours. `p`
    /// then has at most a right child. Returns the node that takes the place of `p` in the
    /// balancing: that child, or `p` itself when it has none.
    fn move_successor_up(&mut self, p: usize) -> usize {
        let Link {
            left: pl,
            right: pr,
            parent: pp,
            ..
        } = self.links[p];
        let mut s = pr;
        while self.links[s].left != NIL {
            s = self.links[s].left;
        }
        let (s_red, p_red) = (self.links[s].red, self.links[p].red);
        self.links[s].red = p_red;
        self.links[p].red = s_red;
        let sr = self.links[s].right;
        if s == pr {
            self.links[p].parent = s;
            self.links[s].right = p;
        } else {
            let sp = self.links[s].parent;
            self.links[p].parent = sp;
            self.links[sp].left = p;
            self.links[s].right = pr;
            self.links[pr].parent = s;
        }
        self.links[p].left = NIL;
        self.links[p].right = sr;
        self.links[sr].parent = p;
        self.links[s].left = pl;
        self.links[pl].parent = s;
        self.links[s].parent = pp;
        if pp == NIL {
            self.root = s;
        } else if self.links[pp].left == p {
            self.links[pp].left = s;
        } else {
            self.links[pp].right = s;
        }
        if sr == NIL { p } else { sr }
    }

    /// `TreeNode.removeTreeNode` of the node `p`: `p` leaves the iteration order, then the tree,
    /// which is rebalanced. `true` when the tree is too small to stay a tree: the bin is a list
    /// again, and the caller converts it.
    fn remove(&mut self, p: usize) -> bool {
        let Link { prev, next, .. } = self.links[p];
        if prev == NIL {
            self.first = next;
        } else {
            self.links[prev].next = next;
        }
        self.links[next].prev = prev;
        let left = self.links[self.root].left;
        if self.links[self.root].right == NIL || left == NIL || self.links[left].left == NIL {
            return true;
        }
        let Link {
            left: pl,
            right: pr,
            ..
        } = self.links[p];
        let replacement = if pl != NIL && pr != NIL {
            self.move_successor_up(p)
        } else if pl != NIL {
            pl
        } else if pr != NIL {
            pr
        } else {
            p
        };
        if replacement != p {
            let pp = self.links[p].parent;
            self.links[replacement].parent = pp;
            if self.links[pp].left == p {
                self.links[pp].left = replacement;
            } else {
                self.links[pp].right = replacement;
            }
            self.links[p].left = NIL;
            self.links[p].right = NIL;
            self.links[p].parent = NIL;
        }
        if !self.links[p].red {
            self.balance_deletion(replacement);
        }
        if replacement == p {
            // A leaf: it leaves its parent once the balancing is done.
            let pp = self.links[p].parent;
            self.links[p].parent = NIL;
            if self.links[pp].left == p {
                self.links[pp].left = NIL;
            } else {
                self.links[pp].right = NIL;
            }
        }
        self.move_root_to_front();
        false
    }

    /// The nodes in iteration order.
    fn order(&self) -> Vec<usize> {
        let mut order = Vec::with_capacity(self.entries.len());
        let mut at = self.first;
        while at != NIL {
            order.push(at);
            at = self.links[at].next;
        }
        order
    }

    /// The entries in iteration order, the removed ones left out.
    fn into_list(self) -> Vec<Entry<K, V>> {
        let order = self.order();
        let mut slots: Vec<Option<Entry<K, V>>> = self.entries.into_iter().map(Some).collect();
        take_entries(&mut slots, &order)
    }

    /// `TreeNode.split` of the tree of a bin that the table doubling splits in two: the entries
    /// whose spread hash has the `bit` of the old capacity go to the second bin. A part with more
    /// than 6 entries is a tree. When every entry lands in one part, that part is the tree
    /// itself, unchanged if it still has more than 6 entries.
    fn split(self, bit: usize, order: Order<K>) -> (Bin<K, V>, Bin<K, V>) {
        let (high, low): (Vec<usize>, Vec<usize>) = self
            .order()
            .into_iter()
            .partition(|&node| (spread(self.entry(node).0) as u32 as usize) & bit != 0);
        if high.is_empty() || low.is_empty() {
            let count = high.len() + low.len();
            let whole = if count <= UNTREEIFY_THRESHOLD {
                Bin::List(self.into_list())
            } else {
                Bin::Tree(Box::new(self))
            };
            return if low.is_empty() {
                (Bin::List(Vec::new()), whole)
            } else {
                (whole, Bin::List(Vec::new()))
            };
        }
        let mut slots: Vec<Option<Entry<K, V>>> = self.entries.into_iter().map(Some).collect();
        let low = Bin::from_list(take_entries(&mut slots, &low), order);
        let high = Bin::from_list(take_entries(&mut slots, &high), order);
        (low, high)
    }
}

/// The entries of the nodes `order` lists, in that order, taken out of their slots (the slot of
/// node `n` is `slots[n - 1]`).
fn take_entries<K, V>(slots: &mut [Option<Entry<K, V>>], order: &[usize]) -> Vec<Entry<K, V>> {
    order
        .iter()
        .filter_map(|&node| slots[node - 1].take())
        .collect()
}

/// A bucket: the entries in insertion order, or a tree once it holds too many.
#[derive(Clone, Debug)]
enum Bin<K, V> {
    List(Vec<Entry<K, V>>),
    Tree(Box<Tree<K, V>>),
}

impl<K, V> Bin<K, V> {
    /// The bin of entries a table doubling leaves: a list, or a tree past 6 entries.
    fn from_list(entries: Vec<Entry<K, V>>, order: Order<K>) -> Self {
        if entries.len() <= UNTREEIFY_THRESHOLD {
            Bin::List(entries)
        } else {
            Bin::Tree(Box::new(Tree::from_list(entries, order)))
        }
    }

    /// The two bins a table doubling makes of this one (`bit` is the old capacity).
    fn split(self, bit: usize, order: Order<K>) -> (Bin<K, V>, Bin<K, V>) {
        match self {
            Bin::List(entries) => {
                let (high, low) = entries
                    .into_iter()
                    .partition(|(hash, ..)| (spread(*hash) as u32 as usize) & bit != 0);
                (Bin::List(low), Bin::List(high))
            }
            Bin::Tree(tree) => tree.split(bit, order),
        }
    }

    /// The entries in iteration order.
    fn iter(&self) -> BinIter<'_, K, V> {
        match self {
            Bin::List(entries) => BinIter::List(entries.iter()),
            Bin::Tree(tree) => BinIter::Tree(tree, tree.first),
        }
    }

    /// The entries in iteration order, consumed.
    fn into_entries(self) -> Vec<Entry<K, V>> {
        match self {
            Bin::List(entries) => entries,
            Bin::Tree(tree) => tree.into_list(),
        }
    }
}

impl<K: PartialEq, V> Bin<K, V> {
    /// `HashMap.removeNode` within the bin: `true` when the key was there. A tree that gets too
    /// small is a list again.
    fn remove(&mut self, hash: i32, key: &K, order: Order<K>) -> bool {
        match self {
            Bin::List(entries) => {
                let Some(position) = entries.iter().position(|(h, k, _)| *h == hash && k == key)
                else {
                    return false;
                };
                entries.remove(position);
                true
            }
            Bin::Tree(tree) => {
                let Some(at) = tree.find(tree.root, spread(hash), key, order) else {
                    return false;
                };
                if tree.remove(at) {
                    let tree = std::mem::replace(self, Bin::List(Vec::new()));
                    *self = Bin::List(tree.into_entries());
                }
                true
            }
        }
    }
}

/// The entries of a bin in iteration order.
enum BinIter<'a, K, V> {
    List(std::slice::Iter<'a, Entry<K, V>>),
    /// The tree, and the node the iteration is at (`NIL` at the end).
    Tree(&'a Tree<K, V>, usize),
}

impl<'a, K, V> Iterator for BinIter<'a, K, V> {
    type Item = &'a Entry<K, V>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            BinIter::List(entries) => entries.next(),
            BinIter::Tree(tree, at) => {
                if *at == NIL {
                    return None;
                }
                let node = *at;
                *at = tree.links[node].next;
                Some(tree.entry(node))
            }
        }
    }
}

/// The entries of a Java `HashMap` in iteration order, given the Java `hashCode` of each key.
#[derive(Clone, Debug)]
pub struct JavaHashMap<K, V> {
    /// The buckets; empty until the first insertion (Java allocates lazily).
    table: Vec<Bin<K, V>>,
    /// `threshold`: the size past which the table doubles (the initial capacity while the
    /// table is not allocated).
    threshold: usize,
    /// The number of entries.
    size: usize,
    /// What `compareTo` says of two keys, to order the keys of a tree bin.
    order: Order<K>,
}

impl<K: PartialEq, V> Default for JavaHashMap<K, V> {
    fn default() -> Self {
        JavaHashMap::new()
    }
}

impl<K: PartialEq, V> JavaHashMap<K, V> {
    /// `new HashMap()`.
    pub fn new() -> Self {
        JavaHashMap {
            table: Vec::new(),
            threshold: 0,
            size: 0,
            order: unordered,
        }
    }

    /// `new HashMap(initialCapacity)`.
    pub fn with_capacity(capacity: usize) -> Self {
        JavaHashMap {
            threshold: table_size_for(capacity),
            ..JavaHashMap::new()
        }
    }

    /// `new HashSet(collection)`'s map: `new HashMap(max(size / .75f + 1, 16))`.
    pub fn for_set_copy(size: usize) -> Self {
        JavaHashMap::with_capacity((((size as f32) / 0.75f32) as usize + 1).max(16))
    }

    /// The `compareTo` of the keys, which orders the entries of the bins that became trees.
    #[must_use]
    pub fn with_order(mut self, order: Order<K>) -> Self {
        self.order = order;
        self
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.size
    }

    /// `true` when there is no entry.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// The index of the bin of a key.
    fn index(&self, hash: i32) -> usize {
        (spread(hash) as u32 as usize) & self.table.len().wrapping_sub(1)
    }

    /// `HashMap.resize`.
    fn resize(&mut self) {
        let old_cap = self.table.len();
        let old_thr = self.threshold;
        let mut new_thr = 0;
        // (Tables never reach `MAXIMUM_CAPACITY` buckets here: the input sizes are bounded.)
        let new_cap = if old_cap > 0 {
            if old_cap >= 16 {
                new_thr = old_thr * 2;
            }
            old_cap * 2
        } else if old_thr > 0 {
            old_thr
        } else {
            new_thr = 12;
            16
        };
        if new_thr == 0 {
            new_thr = ((new_cap as f32) * 0.75f32) as usize;
        }
        self.threshold = new_thr;
        let old = std::mem::take(&mut self.table);
        if old.is_empty() {
            self.table = (0..new_cap).map(|_| Bin::List(Vec::new())).collect();
            return;
        }
        // Each bin gives its lower part to the same index and the other to the same index
        // plus the old capacity, the entries keeping their order.
        let (mut low, mut high) = (Vec::with_capacity(new_cap), Vec::with_capacity(old_cap));
        for bin in old {
            let (lower, upper) = bin.split(old_cap, self.order);
            low.push(lower);
            high.push(upper);
        }
        low.extend(high);
        self.table = low;
    }

    /// `HashMap.put`: insert or replace (a replaced entry keeps its position).
    pub fn put(&mut self, hash: i32, key: K, value: V) {
        if self.table.is_empty() {
            self.resize();
        }
        let index = self.index(hash);
        let (order, capacity) = (self.order, self.table.len());
        let (mut tree, mut grow) = (None, false);
        match &mut self.table[index] {
            Bin::List(bin) => {
                if let Some(entry) = bin.iter_mut().find(|(h, k, _)| *h == hash && *k == key) {
                    entry.2 = value;
                    return;
                }
                bin.push((hash, key, value));
                // `treeifyBin`: a table that is too small grows instead.
                if bin.len() > TREEIFY_THRESHOLD {
                    if capacity < MIN_TREEIFY_CAPACITY {
                        grow = true;
                    } else {
                        tree = Some(Tree::from_list(std::mem::take(bin), order));
                    }
                }
            }
            Bin::Tree(bin) => match bin.find(bin.root, spread(hash), &key, order) {
                Some(at) => {
                    bin.entry_mut(at).2 = value;
                    return;
                }
                None => bin.insert((hash, key, value), order),
            },
        }
        if let Some(tree) = tree {
            self.table[index] = Bin::Tree(Box::new(tree));
        }
        if grow {
            self.resize();
        }
        self.size += 1;
        if self.size > self.threshold {
            self.resize();
        }
    }

    /// `HashMap.get`, with the entry.
    fn entry(&self, hash: i32, key: &K) -> Option<&Entry<K, V>> {
        match self.table.get(self.index(hash))? {
            Bin::List(bin) => bin.iter().find(|(h, k, _)| *h == hash && k == key),
            Bin::Tree(bin) => bin
                .find(bin.root, spread(hash), key, self.order)
                .map(|at| bin.entry(at)),
        }
    }

    /// `HashMap.get`.
    pub fn get(&self, hash: i32, key: &K) -> Option<&V> {
        self.entry(hash, key).map(|(_, _, value)| value)
    }

    /// `HashMap.containsKey`.
    pub fn contains_key(&self, hash: i32, key: &K) -> bool {
        self.entry(hash, key).is_some()
    }

    /// `HashMap.get`, for a value to change in place.
    pub fn get_mut(&mut self, hash: i32, key: &K) -> Option<&mut V> {
        let index = self.index(hash);
        let order = self.order;
        match self.table.get_mut(index)? {
            Bin::List(bin) => bin
                .iter_mut()
                .find(|(h, k, _)| *h == hash && k == key)
                .map(|(_, _, value)| value),
            Bin::Tree(bin) => {
                let at = bin.find(bin.root, spread(hash), key, order)?;
                Some(&mut bin.entry_mut(at).2)
            }
        }
    }

    /// `HashMap.remove`: the bin keeps the order of its other entries (a tree: the order
    /// `removeTreeNode` leaves) and the table never shrinks. `true` when the key was there.
    pub fn remove(&mut self, hash: i32, key: &K) -> bool {
        let index = self.index(hash);
        let order = self.order;
        let Some(bin) = self.table.get_mut(index) else {
            return false;
        };
        if !bin.remove(hash, key, order) {
            return false;
        }
        self.size -= 1;
        true
    }

    /// The first entry in iteration order (`keySet().iterator().next()`).
    pub fn first(&self) -> Option<&(i32, K, V)> {
        self.iter().next()
    }

    /// The idiom `v = map.get(key); if (v == null) { v = new V(); map.put(key, v); } change(v)`:
    /// change the value of `key`, a new one (made by `make`) put first when the key is absent.
    pub fn change_or_put(
        &mut self,
        hash: i32,
        key: K,
        make: impl FnOnce() -> V,
        change: impl FnOnce(&mut V),
    ) {
        match self.get_mut(hash, &key) {
            Some(value) => change(value),
            None => {
                let mut value = make();
                change(&mut value);
                self.put(hash, key, value);
            }
        }
    }

    /// `HashMap.putAll`: presize from the number of entries, then put them all.
    pub fn put_all(&mut self, entries: Vec<(i32, K, V)>) {
        let count = entries.len();
        if count > 0 {
            if self.table.is_empty() {
                let target = ((count as f32) / 0.75f32 + 1.0f32) as usize;
                // `if (t > threshold) threshold = tableSizeFor(t)`: the threshold of an empty
                // table is a power of two, so this is the larger of both.
                self.threshold = self.threshold.max(table_size_for(target));
            } else {
                while count > self.threshold {
                    self.resize();
                }
            }
        }
        for (hash, key, value) in entries {
            self.put(hash, key, value);
        }
    }

    /// The entries in iteration order.
    pub fn iter(&self) -> impl Iterator<Item = &(i32, K, V)> {
        self.table.iter().flat_map(Bin::iter)
    }

    /// The entries in iteration order, consumed.
    pub fn into_entries(self) -> Vec<(i32, K, V)> {
        self.table.into_iter().flat_map(Bin::into_entries).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn keys(map: &JavaHashMap<i32, ()>) -> Vec<i32> {
        map.iter().map(|(_, k, ())| *k).collect()
    }

    #[test]
    fn orders_like_java() {
        let mut map = JavaHashMap::new();
        assert!(map.is_empty());
        for k in [17, 1, 33, 2] {
            map.put(k, k, ());
        }
        map.put(1, 1, ());
        assert_eq!(map.len(), 4);
        assert_eq!(keys(&map), vec![17, 1, 33, 2]);
        for k in 3..14 {
            map.put(k, k, ());
        }
        // 15 entries: the table doubled to 32 buckets.
        assert_eq!(keys(&map)[..4], [1, 33, 2, 3]);
        let mut small = JavaHashMap::with_capacity(0);
        for k in [3, 1, 2] {
            small.put(k, k, ());
        }
        assert_eq!(keys(&small), vec![1, 2, 3]);
        let mut all: JavaHashMap<i32, ()> = JavaHashMap::with_capacity(0);
        all.put_all((0..20).rev().map(|k| (k, k, ())).collect());
        assert_eq!(keys(&all), (0..20).collect::<Vec<_>>());
        all.put_all((20..100).map(|k| (k, k, ())).collect());
        all.put_all(Vec::new());
        assert_eq!(all.len(), 100);
        let set: JavaHashMap<i32, ()> = JavaHashMap::for_set_copy(30);
        assert_eq!(set.threshold, 64);
        assert_eq!(JavaHashMap::<i32, ()>::default().into_entries().len(), 0);
        assert_eq!(table_size_for(usize::MAX), MAXIMUM_CAPACITY);
    }

    #[test]
    fn looks_up_changes_and_removes_like_java() {
        let mut map: JavaHashMap<i64, Vec<i64>> = JavaHashMap::new();
        // Nothing to find in a table that is not allocated.
        assert!(map.get(1, &1).is_none());
        assert!(map.get_mut(1, &1).is_none());
        assert!(!map.remove(1, &1));
        assert!(map.first().is_none());
        for key in [17, 1, 33] {
            map.change_or_put(long_hash(key), key, Vec::new, |list| list.push(key));
        }
        map.change_or_put(long_hash(1), 1, Vec::new, |list| list.push(100));
        assert_eq!(map.len(), 3);
        assert_eq!(map.get(long_hash(1), &1), Some(&vec![1, 100]));
        assert!(map.contains_key(long_hash(33), &33));
        // Same bin, other key: not found.
        assert!(!map.contains_key(long_hash(49), &49));
        assert!(map.get_mut(long_hash(49), &49).is_none());
        assert!(!map.remove(long_hash(49), &49));
        assert_eq!(map.first().map(|(_, key, _)| *key), Some(17));
        assert!(map.remove(long_hash(17), &17));
        assert_eq!(map.first().map(|(_, key, _)| *key), Some(1));
        assert_eq!(map.len(), 2);
        map.get_mut(long_hash(33), &33)
            .expect("33 is there")
            .push(34);
        assert_eq!(map.get(long_hash(33), &33), Some(&vec![33, 34]));
    }

    #[test]
    fn long_hash_folds_the_high_bits() {
        assert_eq!(long_hash(5), 5);
        assert_eq!(long_hash(-1), 0);
        assert_eq!(long_hash(0x1_0000_0003), 2);
        assert_eq!(long_hash(i64::MIN), i32::MIN);
    }

    /// Run a scenario of puts and removes of `Long` keys on a `HashMap` of the JDK and on the
    /// model, and fold the iteration order into a digest at four points: the digests the JDK
    /// gave (a program with the same 64-bit LCG for the keys) are the expectation.
    fn digests(
        ops: u64,
        range: u64,
        shift: u32,
        remove: u64,
        capacity: usize,
        scenario: u64,
    ) -> Vec<u64> {
        let mut seed = 1000 + scenario;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            seed >> 17
        };
        let mut map = if capacity == 0 {
            JavaHashMap::new()
        } else {
            JavaHashMap::with_capacity(capacity)
        }
        .with_order(ordered);
        let mut digests = Vec::new();
        for i in 0..ops {
            let key = ((next() % range) << shift) as i64;
            if next() % 100 < remove {
                map.remove(long_hash(key), &key);
            } else {
                map.put(long_hash(key), key, i as i64);
            }
            if (i + 1) % (ops / 4) == 0 {
                digests.push(
                    map.iter()
                        .fold(0xcbf2_9ce4_8422_2325u64, |hash, (_, key, _)| {
                            (hash ^ *key as u64).wrapping_mul(0x0100_0000_01b3)
                        }),
                );
            }
        }
        digests
    }

    #[test]
    fn iterates_tree_bins_like_java() {
        let scenarios: &[(u64, u64, u32, u64, usize, [u64; 4])] = &[
            (
                200,
                64,
                24,
                20,
                0,
                [
                    0x168b60b6bb14bc67,
                    0xb80c0c14ef434fbd,
                    0x46ed7633ec9ec93f,
                    0x8660b71c1075271f,
                ],
            ),
            (
                400,
                256,
                24,
                30,
                0,
                [
                    0x265ad2c20980d46f,
                    0xc9d2d3a242174237,
                    0xaf8e5461f29a79c7,
                    0xd7ceb2d636770a67,
                ],
            ),
            (
                600,
                1024,
                24,
                40,
                0,
                [
                    0x3a511954d6bb582f,
                    0xf6f1aad26117ff7f,
                    0xcabb1d127ab46a77,
                    0xabd7242aafe8254f,
                ],
            ),
            (
                800,
                4096,
                20,
                45,
                0,
                [
                    0xbd578c5b4762065f,
                    0xcfd59291e9a9362f,
                    0x147765c1274efcaf,
                    0xc8872bb8c493af87,
                ],
            ),
            (
                300,
                32,
                26,
                10,
                0,
                [
                    0x36ec69867c98787d,
                    0xaa721ec0d6b90acd,
                    0xdaa5257cc24b0f47,
                    0xd5f468eb524b0f47,
                ],
            ),
            (
                500,
                128,
                26,
                35,
                0,
                [
                    0xb29d48adb075271f,
                    0x38b75b825e786e3d,
                    0x9b1edbcedd90cc87,
                    0x8d0eea60f37ee805,
                ],
            ),
            (
                1000,
                512,
                16,
                50,
                0,
                [
                    0x7e26d5f34a319647,
                    0xd4f055d24ca149bd,
                    0xc48d2d8dd2de683d,
                    0xf5d838432450ed75,
                ],
            ),
            (
                1000,
                2048,
                12,
                49,
                0,
                [
                    0x1c656cd752cdcb8f,
                    0x9024cc8d354580d5,
                    0xa6bd15dac5cbc54d,
                    0xcca8aa67dbc65b7f,
                ],
            ),
            (
                2000,
                300,
                24,
                48,
                0,
                [
                    0xc2dccf00b8d6be4d,
                    0x5a837772b8b2006d,
                    0x2df1333360cabfa5,
                    0xace4f73b8783a55f,
                ],
            ),
            (
                3000,
                1000,
                22,
                47,
                0,
                [
                    0x572881ca374c1e1d,
                    0x49416716779cfb7f,
                    0xea26e60d850ac215,
                    0x9c317285e8fd9d6f,
                ],
            ),
            (
                150,
                40,
                28,
                5,
                0,
                [
                    0x1c452b3b77b6a32d,
                    0xfa65619a59d081d5,
                    0xf1044159fd8af5a5,
                    0xbc7a5c8951434e75,
                ],
            ),
            (
                250,
                90,
                30,
                25,
                0,
                [
                    0xfa3ce6d481434e75,
                    0xec2c7009c508907,
                    0x9e055a71800ae3b5,
                    0xb69b5bb67fde72c7,
                ],
            ),
            (
                700,
                70,
                32,
                20,
                0,
                [
                    0x57b9a5111280ec8f,
                    0x1e66ff61e6b1c0f7,
                    0xf7482355f7dce585,
                    0xa89342c11c508907,
                ],
            ),
            (
                900,
                200,
                24,
                55,
                0,
                [
                    0x167bcb5af80ae3b5,
                    0x8e35de32b156d7dd,
                    0xc8aa58d499d26367,
                    0x8e6d4380116a871d,
                ],
            ),
            (
                1200,
                20000,
                0,
                40,
                0,
                [
                    0xd7329fd331896175,
                    0x19f60c4ca95881cb,
                    0x7697c64afdb79271,
                    0xea723d3877ef8966,
                ],
            ),
            (
                800,
                300,
                24,
                30,
                1024,
                [
                    0xcba6a2590490d36d,
                    0x477549445bcb875,
                    0x18d5457886ac6f85,
                    0xc33ad48814635a87,
                ],
            ),
            (
                600,
                200,
                24,
                50,
                4,
                [
                    0x95c35e812495ef15,
                    0x5f864724ffd0825,
                    0x8eec889e5184f40f,
                    0xd0201176144e16d5,
                ],
            ),
            (
                1500,
                150,
                26,
                60,
                0,
                [
                    0x10c553567734ac55,
                    0xcfcff7660bf4d6df,
                    0x8b9bd6071280ec8f,
                    0xb4683e61e334ac55,
                ],
            ),
            (
                2500,
                600,
                24,
                52,
                0,
                [
                    0x1ecf60bf49dab29d,
                    0xa49aae14c6bb725,
                    0x8e296426c1bb5a6d,
                    0xdae1ffe573653705,
                ],
            ),
            (
                4000,
                2000,
                24,
                51,
                0,
                [
                    0xa932f922b0ee8125,
                    0x553808c7aaa05285,
                    0x54379525622aeb27,
                    0xde507c7a0c4c4adf,
                ],
            ),
            (
                1800,
                100,
                28,
                58,
                0,
                [
                    0xe3cd1f8b4bc540af,
                    0xf2b53b98b0f65c45,
                    0x8a0bc1ced0f65c45,
                    0x9e2db53703cff60d,
                ],
            ),
            (
                1000,
                60,
                24,
                70,
                0,
                [
                    0xee69d4893d574577,
                    0x38f282221e14bc67,
                    0x55d36ef29b98787d,
                    0xb3de3cfb71320587,
                ],
            ),
            (
                3500,
                800,
                24,
                55,
                0,
                [
                    0x7188443b31cb7dcf,
                    0xd9ec8cb57546623d,
                    0x5028edc393d442d,
                    0xc2209b5a25dbd2c5,
                ],
            ),
            (
                2200,
                90,
                24,
                62,
                0,
                [
                    0x71468d7cc607e27,
                    0xa95092e7784c9cef,
                    0x62b19de3902eaa1d,
                    0xf1e46070381f675f,
                ],
            ),
            (
                500,
                12,
                24,
                30,
                0,
                [
                    0x899e5a01d69029bf,
                    0x1c064353c71a39c5,
                    0xfa62f21681a39c5,
                    0xbaa63eab701a39c5,
                ],
            ),
            (
                800,
                16,
                28,
                40,
                0,
                [
                    0xed7ac98be81a39c5,
                    0x94f9bfad23990c97,
                    0x11f2e88b3990c97,
                    0xbd42c35bf6876aa7,
                ],
            ),
            (
                3000,
                400,
                24,
                65,
                0,
                [
                    0x76b1a10e4b858375,
                    0xded281bcdbbcc155,
                    0xcd2c6c8b03b2006d,
                    0xd27ecb09d96927bf,
                ],
            ),
            (
                2600,
                250,
                26,
                56,
                0,
                [
                    0xfe874d63b46a871d,
                    0x212c7fa8b56cbb8f,
                    0xdaa0df2cd2ee3fff,
                    0x86790c7ad2ee3fff,
                ],
            ),
        ];
        for (number, (ops, range, shift, remove, capacity, expected)) in
            scenarios.iter().enumerate()
        {
            assert_eq!(
                digests(*ops, *range, *shift, *remove, *capacity, number as u64),
                expected,
                "scenario {number}"
            );
        }
    }

    /// The rules of the trees of the bins (the colours, the parents, the keys in order when
    /// `ordered_keys`, the list of the iteration order) and the entries of the map against a
    /// model: what the digests cannot show of the shape of the trees.
    fn check_map(map: &JavaHashMap<i64, i64>, model: &BTreeMap<i64, i64>, ordered_keys: bool) {
        assert_eq!(map.len(), model.len());
        let mut entries = BTreeMap::new();
        for (hash, key, value) in map.iter() {
            assert_eq!(*hash, long_hash(*key));
            assert!(
                entries.insert(*key, *value).is_none(),
                "{key} is listed twice"
            );
        }
        assert_eq!(&entries, model);
        for bin in &map.table {
            if let Bin::Tree(tree) = bin {
                check_tree(tree, ordered_keys);
            }
        }
    }

    fn check_tree(tree: &Tree<i64, i64>, ordered_keys: bool) {
        assert!(!tree.links[tree.root].red, "the root is black");
        assert_eq!(tree.links[tree.root].parent, NIL);
        let mut count = 0;
        check_subtree(tree, tree.root, NIL, (None, None), ordered_keys, &mut count);
        let listed = tree.order();
        assert_eq!(count, listed.len(), "every node of the tree is listed");
        assert_eq!(tree.links[tree.first].prev, NIL);
        for node in listed {
            let next = tree.links[node].next;
            if next != NIL {
                assert_eq!(tree.links[next].prev, node);
            }
        }
    }

    /// The black height below `at` (a missing node is black), checking the rules on the way.
    fn check_subtree(
        tree: &Tree<i64, i64>,
        at: usize,
        parent: usize,
        (lower, upper): (Option<(i32, i64)>, Option<(i32, i64)>),
        ordered_keys: bool,
        count: &mut usize,
    ) -> usize {
        if at == NIL {
            return 1;
        }
        let link = tree.links[at];
        *count += 1;
        assert_eq!(link.parent, parent);
        let key = (spread(tree.entry(at).0), tree.entry(at).1);
        if ordered_keys {
            assert!(
                lower.is_none_or(|bound| key > bound),
                "{key:?} below {lower:?}"
            );
            assert!(
                upper.is_none_or(|bound| key < bound),
                "{key:?} above {upper:?}"
            );
        }
        for child in [link.left, link.right] {
            assert!(
                !(link.red && tree.links[child].red),
                "two red nodes in a row"
            );
        }
        let left = check_subtree(tree, link.left, at, (lower, Some(key)), ordered_keys, count);
        let right = check_subtree(
            tree,
            link.right,
            at,
            (Some(key), upper),
            ordered_keys,
            count,
        );
        assert_eq!(left, right, "the black heights of {key:?} differ");
        left + usize::from(!link.red)
    }

    /// Random puts (new keys and replacements), removes and lookups, checked against a model:
    /// keys sharing their low bits make tree bins, keys with a common bit make the table
    /// doubling split them all to one side.
    #[test]
    fn random_operations_keep_the_trees_valid() {
        let scenarios: &[(u64, u64, u32, u64, usize, u64, bool)] = &[
            (3000, 120, 24, 45, 0, 0, true),
            (3000, 120, 24, 45, 0, 0, false),
            (4000, 60, 26, 60, 0, 0, true),
            (4000, 2000, 0, 35, 0, 0, true),
            (2000, 500, 3, 50, 16, 0, true),
            (2500, 90, 24, 70, 1024, 0, false),
            (3000, 300, 24, 40, 0, 0x40, true),
            (3000, 300, 24, 40, 0, 0x40, false),
            (5000, 700, 6, 55, 0, 0x2a, true),
            (1500, 64, 20, 85, 0, 0x40, true),
        ];
        for (number, &(ops, range, shift, remove, capacity, salt, ordered_keys)) in
            scenarios.iter().enumerate()
        {
            let mut seed = 77 + number as u64;
            let mut next = move || {
                seed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                seed >> 17
            };
            let order: Order<i64> = if ordered_keys { ordered } else { unordered };
            let mut map = if capacity == 0 {
                JavaHashMap::new()
            } else {
                JavaHashMap::with_capacity(capacity)
            }
            .with_order(order);
            let mut model = BTreeMap::new();
            for i in 0..ops {
                let key = (((next() % range) << shift) | salt) as i64;
                if next() % 100 < remove {
                    assert_eq!(
                        map.remove(long_hash(key), &key),
                        model.remove(&key).is_some()
                    );
                } else {
                    map.put(long_hash(key), key, i as i64);
                    model.insert(key, i as i64);
                }
                assert_eq!(
                    map.get(long_hash(key), &key).copied(),
                    model.get(&key).copied()
                );
                if i % 61 == 0 {
                    check_map(&map, &model, ordered_keys);
                }
            }
            check_map(&map, &model, ordered_keys);
        }
    }

    /// Scenarios of the JDK 27 `HashMap` with heavy removals (its digests come from a program
    /// with the same generator as `digests`).
    #[test]
    fn removes_in_tree_bins_like_java() {
        let scenarios: &[(u64, u64, u32, u64, usize, u64, [u64; 4])] = &[
            (
                1600,
                40,
                24,
                75,
                0,
                2000,
                [
                    0x9e19c7c4d6990c97,
                    0x4d0cb850751a39c5,
                    0x82f0c8dc409029bf,
                    0x6f1fb78dd990c97,
                ],
            ),
            (
                2400,
                12,
                24,
                70,
                0,
                2001,
                [
                    0x8328ae0abeb6fed,
                    0xf3e8182ad2be94f,
                    0x554f8cec8dce13f5,
                    0x8328adb92eb6fed,
                ],
            ),
            (
                2000,
                6,
                20,
                80,
                0,
                2002,
                [
                    0xcbf29ce484222325,
                    0xa8328836543b6fed,
                    0x8f63bd4c4fa1b7df,
                    0xb83288ee602b6fed,
                ],
            ),
            (
                1200,
                30,
                26,
                85,
                0,
                2003,
                [
                    0xece700e23a876aa7,
                    0xda44042601a39c5,
                    0x98c5b03449ce13f5,
                    0xdb4abbdd1c0f2fb7,
                ],
            ),
            (
                3000,
                100,
                24,
                45,
                0,
                2004,
                [
                    0xf17abd33b34a45d7,
                    0xe18e14273011f6fd,
                    0x261e5f3935a00ff,
                    0xb937b8c1d580ec8f,
                ],
            ),
            (
                6000,
                8,
                22,
                50,
                0,
                2005,
                [
                    0x19546ca46b4f2fb7,
                    0x88328ba3a82b6fed,
                    0x8d2e0ea8814e13f5,
                    0x19547042eb0f2fb7,
                ],
            ),
        ];
        for &(ops, range, shift, remove, capacity, scenario, expected) in scenarios {
            assert_eq!(
                digests(ops, range, shift, remove, capacity, scenario),
                expected,
                "scenario {scenario}"
            );
        }
    }
}
