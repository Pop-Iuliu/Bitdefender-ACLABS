use crate::protocol::Wall;

pub fn chebyshev_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

pub fn bresenham_line(from: (i32, i32), to: (i32, i32)) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let dx = (to.0 - from.0).abs();
    let dy = -(to.1 - from.1).abs();
    let sx = (to.0 - from.0).signum();
    let sy = (to.1 - from.1).signum();

    let mut err = dx + dy;
    let (mut x, mut y) = (from.0, from.1);

    loop {
        points.push((x, y));
        if x == to.0 && y == to.1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }

    points
}

pub fn has_line_of_sight(from: (i32, i32), to: (i32, i32), walls: &[Wall]) -> bool {
    let line = bresenham_line(from, to);

    for &(x, y) in line.iter().skip(1).take(line.len().saturating_sub(2)) {
        for w in walls {
            if (x - w.x).abs() <= 1 && (y - w.y).abs() <= 1 {
                return false;
            }
        }
    }

    true
}
