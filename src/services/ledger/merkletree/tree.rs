extern crate ring;
extern crate rustc_serialize;

use std::cmp;

use self::ring::digest::{ Algorithm, Digest };
use self::rustc_serialize::{ Encodable, Encoder, Decodable, Decoder };

use services::ledger::merkletree::hashutils::{ HashUtils };

pub use services::ledger::merkletree::proof::{
    Proof,
    Lemma,
    Positioned
};

pub type TreeLeafData = String;

/// Binary Tree where leaves hold a stand-alone value.
#[derive(Clone, Debug, PartialEq)]
pub enum Tree {
    Empty {
        hash: Vec<u8>
    },

    Leaf {
        hash: Vec<u8>,
        value: TreeLeafData
    },

    Node {
        hash: Vec<u8>,
        left: Box<Tree>,
        right: Box<Tree>
    }
}

impl Tree {
    /// Create an empty tree
    pub fn empty(hash: Digest) -> Self {
        Tree::Empty {
            hash: hash.as_ref().into()
        }
    }

    /// Create a new tree
    pub fn new(hash: Digest, value: TreeLeafData) -> Self {
        Tree::Leaf {
            hash: hash.as_ref().into(),
            value: value
        }
    }

    /// Create a new leaf
    pub fn new_leaf(algo: &'static Algorithm, value: TreeLeafData) -> Tree {

        let hash = algo.hash_leaf(&value);
        Tree::new(hash, value)
    }

    /// Returns a hash from the tree.
    pub fn hash(&self) -> &Vec<u8> {
        match *self {
            Tree::Empty { ref hash }    => hash,
            Tree::Leaf { ref hash, .. } => hash,
            Tree::Node { ref hash, .. } => hash
        }
    }

    /// Returns a borrowing iterator over the leaves of the tree.
    pub fn iter(&self) -> LeavesIterator {
        LeavesIterator::new(self)
    }

    pub fn get_height(&self) -> usize {
        match *self {
            Tree::Empty { .. } => { 0 },
            Tree::Node { ref left, ref right, .. } => {
                1 + cmp::max(left.get_height(),right.get_height())
            },
            Tree::Leaf { .. } => { 0 }
        }
    }

    pub fn get_count(&self) -> usize {
        match *self {
            Tree::Empty { .. } => { 0 },
            Tree::Node { ref left, ref right, .. } => {
                left.get_count() + right.get_count()
            },
            Tree::Leaf { .. } => { 1 }
        }
    }
}

impl Encodable for Tree {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        match *self {
            Tree::Empty { ref hash, .. } => {
                s.emit_struct("node", 4, |s| {
                    s.emit_struct_field("type", 0, |s| { s.emit_str("empty") })?;
                    s.emit_struct_field("hash", 1, |s| { hash.encode(s) })?;
                    s.emit_struct_field("", 2, |s| { s.emit_str("") })?;
                    s.emit_struct_field("", 3, |s| { s.emit_str("") })?;
                    Ok(())
                })
            }
            Tree::Node { ref hash, ref left, ref right, .. } => {
                s.emit_struct("node", 4, |s| {
                    s.emit_struct_field("type", 0, |s| { s.emit_str("node") })?;
                    s.emit_struct_field("hash", 1, |s| { hash.encode(s) })?;
                    s.emit_struct_field("left", 2, |s| { left.encode(s) })?;
                    s.emit_struct_field("right", 3, |s| { right.encode(s) })?;
                    Ok(())
                })
            }
            Tree::Leaf { ref hash, ref value, .. } => {
                s.emit_struct("node", 4, |s| {
                    s.emit_struct_field("type", 0, |s| { s.emit_str("leaf") })?;
                    s.emit_struct_field("hash", 1, |s| { hash.encode(s) })?;
                    s.emit_struct_field("value", 2, |s| { value.encode(s) })?;
                    s.emit_struct_field("", 3, |s| { s.emit_str("") })?;
                    Ok(())
                })
            }
        }
    }
}

impl Decodable for Tree {
    fn decode<D: Decoder>(d: &mut D) -> Result<Tree, D::Error> {
        d.read_struct("node", 4, |d| {
            let nodetype = d.read_struct_field("type", 0, |d| { d.read_str() })?;
            let hash = d.read_struct_field("hash", 0, |d| { Vec::<u8>::decode(d) })?;
            match nodetype.as_ref() {
                "empty" => {
                    Ok(Tree::Empty{
                        hash: hash
                    })
                }
                "node" => {
                    let left = d.read_struct_field("left", 1, |d| { Tree::decode(d) })?;
                    let right = d.read_struct_field("right", 2, |d| { Tree::decode(d) })?;
                    Ok(Tree::Node{
                        hash: hash,
                        left: Box::new(left),
                        right: Box::new(right)
                    })
                }
                "leaf" => {
                    let value = d.read_struct_field("value", 1, |d| { TreeLeafData::decode(d) })?;
                    Ok(Tree::Leaf{
                        hash: hash,
                        value: value
                    })
                }
                _ => {
                    Err(d.error("bad node type"))
                }
            }
        })
    }
}

/// An borrowing iterator over the leaves of a `Tree`.
/// Adapted from http://codereview.stackexchange.com/q/110283.
#[allow(missing_debug_implementations)]
pub struct LeavesIterator<'a> {
    current_value: Option<&'a TreeLeafData>,
    right_nodes: Vec<&'a Tree>
}

impl <'a> LeavesIterator<'a> {

    fn new(root: &'a Tree) -> Self {
        let mut iter = LeavesIterator {
            current_value: None,
            right_nodes: Vec::new()
        };

        iter.add_left(root);

        iter
    }

    fn add_left(&mut self, mut tree: &'a Tree) {
        loop {
            match *tree {
                Tree::Empty { .. } => {
                    self.current_value = None;
                    break;
                },

                Tree::Node { ref left, ref right, .. } => {
                    self.right_nodes.push(right);
                    tree = left;
                },

                Tree::Leaf { ref value, .. } => {
                    self.current_value = Some(value);
                    break;
                }
            }
        }
    }

}

impl <'a> Iterator for LeavesIterator<'a> {

    type Item = &'a TreeLeafData;

    fn next(&mut self) -> Option<&'a TreeLeafData> {
        let result = self.current_value.take();

        if let Some(rest) = self.right_nodes.pop() {
            self.add_left(rest);
        }

        result
    }

}

/// An iterator over the leaves of a `Tree`.
#[allow(missing_debug_implementations)]
pub struct LeavesIntoIterator {
    current_value: Option<TreeLeafData>,
    right_nodes: Vec<Tree>
}

impl LeavesIntoIterator {

    fn new(root: Tree) -> Self {
        let mut iter = LeavesIntoIterator {
            current_value: None,
            right_nodes: Vec::new()
        };

        iter.add_left(root);

        iter
    }

    fn add_left(&mut self, mut tree: Tree) {
        loop {
            match tree {
                Tree::Empty { .. } => {
                    self.current_value = None;
                    break;
                },

                Tree::Node { left, right, .. } => {
                    self.right_nodes.push(*right);
                    tree = *left;
                },

                Tree::Leaf { value, .. } => {
                    self.current_value = Some(value);
                    break;
                }
            }
        }
    }

}

impl Iterator for LeavesIntoIterator {

    type Item = TreeLeafData;

    fn next(&mut self) -> Option<TreeLeafData> {
        let result = self.current_value.take();

        if let Some(rest) = self.right_nodes.pop() {
            self.add_left(rest);
        }

        result
    }

}

impl IntoIterator for Tree {

    type Item     = TreeLeafData;
    type IntoIter = LeavesIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        LeavesIntoIterator::new(self)
    }

}
