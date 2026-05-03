// A* pentru gasirea drumului optim

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::protocol::Wall;

type Point = (i32, i32);

#[derive(Eq, PartialEq)]
struct Node {
    f: i32,
    pos: Point,
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f.cmp(&self.f)
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn heuristic(from: Point, to: Point) -> i32 {
    let dx = (from.0 - to.0).abs();
    let dy = (from.1 - to.1).abs();
    dx.max(dy) / 3
}

pub fn find_next_step(
    start: Point,
    goal: Point,
    walls: &[Wall],
    width: i32,
    height: i32,
) -> Option<(i32, i32)> {
    if start == goal {
        println!("e deja la destinatie");
        return None;
    }

    let wall_set: HashSet<Point> = walls.iter().map(|w| (w.x, w.y)).collect();

    let mut open = BinaryHeap::new();
    let mut g_score: HashMap<Point, i32> = HashMap::new();
    let mut came_from: HashMap<Point, Point> = HashMap::new();
    let mut closed: HashSet<Point> = HashSet::new();

    g_score.insert(start, 0);
    open.push(Node {
        f: heuristic(start, goal),
        pos: start,
    });

    let dirs: [(i32, i32); 8] = [
        (3, 3),
        (-3, -3),
        (3, -3),
        (-3, 3),
        (3, 0),
        (0, 3),
        (0, -3),
        (-3, 0),
    ];

    while let Some(Node { pos: current, .. }) = open.pop() {
        if !closed.insert(current) {
            continue;
        }
        if current == goal {
            let mut step = current;
            while let Some(&parent) = came_from.get(&step) {
                if parent == start {
                    return Some((step.0 - start.0, step.1 - start.1));
                }
                step = parent;
            }
            return None;
        }

        let current_g = *g_score.get(&current).unwrap_or(&i32::MAX);

        for (dx, dy) in dirs {
            let neighbor = (current.0 + dx, current.1 + dy);

            if neighbor.0 < 0 || neighbor.0 > width || neighbor.1 < 0 || neighbor.1 > height {
                continue;
            }
            if closed.contains(&neighbor) {
                continue;
            }

            if wall_set.contains(&neighbor) {
                continue;
            }

            let tentative_g = current_g + 1;
            let neighbor_g = *g_score.get(&neighbor).unwrap_or(&i32::MAX);

            if tentative_g < neighbor_g {
                came_from.insert(neighbor, current);
                g_score.insert(neighbor, tentative_g);
                let f = tentative_g + heuristic(neighbor, goal);
                open.push(Node { f, pos: neighbor });
            }
        }
    }

    None
}
