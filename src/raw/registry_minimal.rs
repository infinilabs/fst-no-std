// This module is a drop-in but inefficient replacement of the LRU registry.
// In particular, this registry will never forget a node. In other words, if
// this registry is used during construction, then you're guaranteed a minimal
// FST.
//
// This is really only meant to be used for debugging and experiments. It is
// a memory/CPU hog.
//
// One "easy" improvement here is to use an FNV hash instead of the super
// expensive SipHasher.

#![allow(dead_code)]
#[cfg(feature = "std")]
use crate::raw::build::BuilderNode;
use crate::raw::CompiledAddr;
#[cfg(feature = "std")]
use std::collections::hash_map::{Entry, HashMap};

#[derive(Debug)]
#[cfg(feature = "std")]
pub struct Registry {
    table: HashMap<BuilderNode, RegistryCell>,
}

#[derive(Debug)]
pub enum RegistryEntry<'a> {
    Found(CompiledAddr),
    NotFound(&'a mut RegistryCell),
    Rejected,
}

#[derive(Clone, Copy, Debug)]
pub struct RegistryCell(CompiledAddr);

#[cfg(feature = "std")]
impl Registry {
    pub fn new(table_size: usize, _lru_size: usize) -> Registry {
        Registry { table: HashMap::with_capacity(table_size) }
    }

    pub fn entry<'a>(&'a mut self, bnode: &BuilderNode) -> RegistryEntry<'a> {
        match self.table.entry(bnode.clone()) {
            Entry::Occupied(v) => RegistryEntry::Found(v.get().0),
            Entry::Vacant(v) => {
                RegistryEntry::NotFound(v.insert(RegistryCell(0)))
            }
        }
    }
}

impl RegistryCell {
    pub fn insert(&mut self, addr: CompiledAddr) {
        self.0 = addr;
    }
}
