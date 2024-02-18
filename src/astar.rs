//! This implementation of A* is intended to be generic, but is in practice used with the Eve Map
//! data structures. This
use std::cmp::Ordering;
use std::fmt::Debug;
use crate::astar::AStarError::*;
use crate::astar::ClosedListState::*;

/// OpenList is a general trait to allow templating of a priority queue implementation for the
/// AStar algorithm.
pub trait OpenList<Element> {
    fn is_empty(&self) -> bool;
    fn push_open(&mut self, e: Element);
    fn pop_min(&mut self) -> Option<Element>;
}

/// OpenItem is an item in the OpenList.
/// It needs to implement Ord so that it is sortable / ordered
/// Additionally, in the implementation of ordering for BinaryHeap, it must reverse the Ordering
#[derive(Debug, Clone)]
pub struct OpenItem<Node, Cost> {
    pub heuristic: Cost,
    pub node: Node,
}

#[derive(Debug, Copy, Clone)]
pub enum ClosedListState<Node, Cost> {
    /// Unvisited is the expected starting state of most nodes, allowing them to be explored
    Unvisited,
    /// StartingPoint is
    StartingPoint(Cost),
    /// PathFrom 
    PathFrom(Node, Cost)
}

pub trait ClosedList<Node: Copy + Clone, Cost>: std::ops::IndexMut<Node, Output = ClosedListState<Node, Cost>>{
    fn unwind(&self, node: Node) -> Vec<Node> {
        let mut r = node;
        let mut path: Vec<Node> = vec![r];

        while let PathFrom(last, _) = &self[r] {
            path.push(*last);
            r = *last;
        }

        path.reverse();
        path
    }
}

pub enum AStarError {
    OpenItemNotInClosedList,
    FoundHigherCostPath,
    PathNotFound
}

/// astar implements A* over a number of trait bounds and using mostly things managed outside of it
/// This uses a number of trait bounds on things like Cost to be generic over integers / floats
pub fn astar<
    Node: Copy,
    Open: OpenList<OpenItem<Node, Cost>>,
    Closed: ClosedList<Node, Cost>,
    Cost: Ord + Copy + std::ops::Add<Output=Cost>,
    IsGoalFn: Fn(&Node) -> bool,
    HeuristicFn: Fn(&Node) -> Cost,
    GetNeighboursFn: Fn(&Node) -> Vec<(Cost, Node)>
>(
    openlist: &mut Open,
    closed: &mut Closed,
    is_goal: IsGoalFn,
    heuristic: HeuristicFn,
    neighbours: GetNeighboursFn,
) -> Result<Node, AStarError>
{
    while let Some(item) = openlist.pop_min() {
        let current_node = item.node;

        if is_goal(&current_node) {
            return Ok(current_node);
        }

        // If the current system is not in the closed list, assume it is the origin and has cost 0
        let current_cost = match &closed[current_node] {
            PathFrom(_, c) => *c,
            StartingPoint(c) => *c,
            Unvisited => return Err(OpenItemNotInClosedList),
        };

        for (neighbour_cost, neighbour) in neighbours(&current_node) {
            let potential_path_cost = neighbour_cost + current_cost;

            match closed[neighbour] {
                PathFrom(_, existing_cost) if existing_cost <= potential_path_cost => continue,
                StartingPoint(_) => continue,
                PathFrom(_, _) => return Err(FoundHigherCostPath),
                Unvisited => (),
            };

            // Set the cost of the neighbour to the total cost, and the origin as the current node
            closed[neighbour] = PathFrom(current_node, potential_path_cost);

            // Add the neighbour to the openlist to be explored when it is the lowest total estimated distance
            openlist.push_open(OpenItem {
                heuristic: potential_path_cost + heuristic(&neighbour),
                node: neighbour,
            });
        }
    }

    Err(PathNotFound)
}

// TODO: There are a a whole load of relations that have to be guaranteed here
//       Need to double check that the required relationships apply with these implementations
impl<Node: Eq, Cost> Eq for OpenItem<Node, Cost> {}

impl<Node: Eq, Cost> PartialEq for OpenItem<Node, Cost> {
    fn eq(&self, other: &Self) -> bool {
        self.node.eq(&other.node)
    }
}

impl<Node: Eq, Cost: Ord> Ord for OpenItem<Node, Cost> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl<Node: Eq, Cost: Ord> PartialOrd for OpenItem<Node, Cost> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.heuristic
            .partial_cmp(&other.heuristic)
            .map(|x| x.reverse()) // reverse the ordering so that a priority queue is min first
    }
}