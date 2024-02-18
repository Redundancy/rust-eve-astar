use crate::SolarSystemIndex;
use crate::astar::{ClosedList, ClosedListState};

pub struct SimpleClosed<Cost> (Vec<ClosedListState<SolarSystemIndex, Cost>>);

impl<Cost: Copy> SimpleClosed<Cost> {
    pub fn new(capacity: usize) -> Self {
        SimpleClosed(std::iter::repeat(ClosedListState::Unvisited).take(capacity).collect())
    }
}

// TODO: using Index implies that anything looking for an item beyond the bounds will cause a panic
//       this is probably sufficient, but it might be nice to ensure that this could not happen.
impl<Cost> std::ops::Index<SolarSystemIndex> for SimpleClosed<Cost> {
    type Output = ClosedListState<SolarSystemIndex, Cost>;

    fn index(&self, index: SolarSystemIndex) -> &Self::Output {
        &self.0[usize::from(index)]
    }
}

impl<Cost> std::ops::IndexMut<SolarSystemIndex> for SimpleClosed<Cost> {
    fn index_mut(&mut self, index: SolarSystemIndex) -> &mut Self::Output {
        &mut self.0[usize::from(index)]
    }
}

// Implement the super-trait
impl<Cost> ClosedList<SolarSystemIndex, Cost> for SimpleClosed<Cost> {}