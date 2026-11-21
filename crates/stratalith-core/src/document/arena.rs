use crate::Node;

/// Identifies a node within its document by slot and generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId {
    index: usize,
    generation: u64,
}

#[derive(Clone, Default)]
pub(super) struct NodeArena {
    slots: Vec<Slot>,
    free_head: Option<usize>,
    len: usize,
    // Highest generation issued, or reserved for allocations after a history restore.
    generation: u64,
}

#[derive(Clone)]
enum Slot {
    Occupied {
        generation: u64,
        node: Node,
    },
    Vacant {
        generation: u64,
        next: Option<usize>,
    },
    Retired,
}

impl NodeArena {
    pub(super) fn resume(&mut self, previous: &Self) {
        self.generation = previous
            .generation
            .max(self.generation)
            .checked_add(1)
            .expect("node ID generations exhausted");
    }

    pub(super) fn insert_with_key(&mut self, create: impl FnOnce(NodeId) -> Node) -> NodeId {
        let id = match self.free_head {
            Some(index) => {
                let Slot::Vacant { generation, next } = self.slots[index] else {
                    unreachable!("free list contains only vacant slots");
                };
                self.generation = self.generation.max(generation);
                let generation = self.generation;
                let id = NodeId { index, generation };
                let node = create(id);
                self.slots[index] = Slot::Occupied { generation, node };
                self.free_head = next;
                id
            }
            None => {
                let id = NodeId {
                    index: self.slots.len(),
                    generation: self.generation,
                };
                let node = create(id);
                self.slots.push(Slot::Occupied {
                    generation: id.generation,
                    node,
                });
                id
            }
        };
        self.len += 1;
        id
    }

    pub(super) fn get(&self, id: NodeId) -> Option<&Node> {
        match self.slots.get(id.index)? {
            Slot::Occupied { generation, node } if *generation == id.generation => Some(node),
            _ => None,
        }
    }

    pub(super) fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        match self.slots.get_mut(id.index)? {
            Slot::Occupied { generation, node } if *generation == id.generation => Some(node),
            _ => None,
        }
    }

    pub(super) fn remove(&mut self, id: NodeId) -> Option<Node> {
        self.get(id)?;
        let Slot::Occupied { generation, node } =
            std::mem::replace(&mut self.slots[id.index], Slot::Retired)
        else {
            unreachable!("the validated slot is occupied");
        };
        // Never wrap generations: retire the slot rather than revive an old ID.
        if let Some(generation) = generation.checked_add(1) {
            self.slots[id.index] = Slot::Vacant {
                generation,
                next: self.free_head,
            };
            self.free_head = Some(id.index);
        }
        self.len -= 1;
        Some(node)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.len
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.slots.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockKind, NodeKind};

    fn node(id: NodeId) -> Node {
        Node {
            id,
            parent: None,
            kind: NodeKind::Block(BlockKind::Paragraph),
            children: Vec::new(),
        }
    }

    #[test]
    fn reuse_changes_generation_without_adding_slots() {
        let mut arena = NodeArena::default();
        let old = arena.insert_with_key(node);
        arena.remove(old).unwrap();

        let new = arena.insert_with_key(node);

        assert_eq!(old.index, new.index);
        assert_ne!(old.generation, new.generation);
        assert!(arena.get(old).is_none());
        assert_eq!(arena.get(new).unwrap().id(), new);
        assert_eq!(arena.slots.len(), 1);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn stale_key_cannot_mutate_or_remove_replacement() {
        let mut arena = NodeArena::default();
        let old = arena.insert_with_key(node);
        arena.remove(old).unwrap();
        let new = arena.insert_with_key(node);

        let stale_mutation = arena.get_mut(old).is_some();
        let stale_removal = arena.remove(old);

        assert!(!stale_mutation);
        assert!(stale_removal.is_none());
        assert_eq!(arena.get(new).unwrap().id(), new);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn live_key_allows_mutation() {
        let mut arena = NodeArena::default();
        let id = arena.insert_with_key(node);

        arena.get_mut(id).unwrap().kind = NodeKind::Block(BlockKind::Heading {
            level: crate::HeadingLevel::H2,
        });

        assert_eq!(
            arena.get(id).unwrap().kind(),
            &NodeKind::Block(BlockKind::Heading {
                level: crate::HeadingLevel::H2
            })
        );
    }

    #[test]
    fn unknown_index_is_rejected() {
        let mut arena = NodeArena::default();
        let id = NodeId {
            index: usize::MAX,
            generation: 0,
        };

        let found = arena.get(id).is_some();
        let mutable = arena.get_mut(id).is_some();
        let removed = arena.remove(id);

        assert!(!found);
        assert!(!mutable);
        assert!(removed.is_none());
        assert!(arena.is_empty());
    }

    #[test]
    fn free_list_reuses_multiple_holes_without_disturbing_live_nodes() {
        let mut arena = NodeArena::default();
        let first = arena.insert_with_key(node);
        let survivor = arena.insert_with_key(node);
        let last = arena.insert_with_key(node);
        arena.remove(first).unwrap();
        arena.remove(last).unwrap();

        let reused_last = arena.insert_with_key(node);
        let reused_first = arena.insert_with_key(node);

        assert_eq!(reused_last.index, last.index);
        assert_eq!(reused_first.index, first.index);
        assert_eq!(arena.get(survivor).unwrap().id(), survivor);
        assert_eq!(arena.slots.len(), 3);
        assert_eq!(arena.len(), 3);
    }

    #[test]
    fn generation_exhaustion_retires_slot_without_wrapping() {
        let mut arena = NodeArena::default();
        let initial = arena.insert_with_key(node);
        let last = NodeId {
            index: initial.index,
            generation: u64::MAX,
        };
        arena.slots[initial.index] = Slot::Occupied {
            generation: last.generation,
            node: node(last),
        };

        let removed = arena.remove(last).unwrap();
        let new = arena.insert_with_key(node);

        assert_eq!(removed.id(), last);
        assert_ne!(new.index, last.index);
        assert!(arena.get(initial).is_none());
        assert!(arena.get(last).is_none());
        assert!(matches!(arena.slots[last.index], Slot::Retired));
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn removal_invalidates_key_before_slot_is_reused() {
        let mut arena = NodeArena::default();
        let id = arena.insert_with_key(node);

        let removed = arena.remove(id).unwrap();

        assert_eq!(removed.id(), id);
        assert!(arena.get(id).is_none());
        assert!(arena.get_mut(id).is_none());
        assert!(arena.remove(id).is_none());
        assert!(arena.is_empty());
    }
}
