/*
Initial state is that the openlist should contain only the starting node(s)

We need OpenItem, where we track a node and the estimated cost for it (the total heuristic of the note)
NB: to be more optimal, we ought to


*/
use crate::astar;
use std::collections::binary_heap::BinaryHeap;

/// SimpleOpenList is a simplistic implementation of an astar::OpenList
/// It uses a BinaryHeap to implement a priority queue, but does not check for the presence
/// of an existing OpenItem entry for the same Node. This makes the implementation potentially
/// somewhat inefficient because it allows the priority queue to grow with items of higher cost
/// than already pending ones.
pub struct SimpleOpenList<N, Cost: Ord> {
    ordering: BinaryHeap<astar::OpenItem<N, Cost>>,
    // TODO: use this to prevent multiple instances of a Node being added
    //
    //node_check: Vec<astar::OpenItem<N, Cost>>
}

impl<N, Cost: Ord> SimpleOpenList<N, Cost>
where
    astar::OpenItem<N, Cost>: Ord,
{
    pub fn new() -> Self {
        Self {
            ordering: BinaryHeap::new(),
            //node_check: vec![],
        }
    }
}

impl<N, Cost: Ord> astar::OpenList<astar::OpenItem<N, Cost>> for SimpleOpenList<N, Cost>
where
    astar::OpenItem<N, Cost>: Ord,
{
    fn is_empty(&self) -> bool {
        self.ordering.is_empty()
    }

    fn push_open(&mut self, e: astar::OpenItem<N, Cost>) {
        // TODO: There's no need for multiple items of the same node
        //       we only need the lowest cost item.
        //       This implies that if we know the current lowest cost in the queue for an item,
        //       then we can reject any new items of greater cost
        self.ordering.push(e);
    }

    fn pop_min(&mut self) -> Option<astar::OpenItem<N, Cost>> {
        self.ordering.pop()
    }
}