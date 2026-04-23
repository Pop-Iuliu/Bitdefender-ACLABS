use serde::Deserialize;
use std::fs;
use std::collections::VecDeque;

#[derive(Debug, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Deserialize)]
struct Cell {
    #[serde(rename = "type")]
    cell_type: String,
    x: i32,
    y: i32,
}

#[derive(Debug, Deserialize)]
struct Maze {
    width: i32,
    height: i32,
    start: Point,
    goal: Point,
    grid: Vec<Cell>,
}

fn to_vec(maze: &Maze) -> Vec<Vec<bool>> {
    let w: usize = maze.width as usize;
    let h: usize = maze.height as usize;
    
    let mut grid = vec![vec![false; w]; h];

    for cell in &maze.grid {
        if cell.cell_type == "wall" {
            grid[cell.x as usize][cell.y as usize] = true;
        }
    }
    grid
}

// alternativa mai eficienta in cazul acesta este djikstra (speranta mea este sa implementez si djikstra imediat)

fn bfs(maze: &Vec<Vec<bool>>, start: &Point, end: &Point, w: isize, h: isize) -> Option<i32>
{
    let mut dq: VecDeque<(usize, usize)> = VecDeque::new();
    let mut dist = vec![vec![i32::MAX; w as usize]; h as usize];
    dist[start.x as usize][start.y as usize] = 0;
    dq.push_back((start.x as usize, start.y as usize));

    let modif:[(isize, isize); 4] = [(-1, 0), (1, 0), (0, 1), (0, -1)];

    while let Some((x, y)) = dq.pop_front() {
        if x == (end.x as usize) && y == (end.y as usize) {
            return Some(dist[x][y]);
        }

        for (dx, dy) in modif {
            let new_x = x as isize + dx;
            let new_y = y as isize + dy;

            if new_x < 0 || new_y < 0 || new_x >= w || new_y >= h {
                continue;
            }
            
            let new_x = new_x as usize;
            let new_y = new_y as usize;

            if !maze[new_x][new_y] && dist[x][y] + 1 < dist[new_x][new_y] {
                dist[new_x][new_y] = dist[x][y] + 1;
                dq.push_back((new_x, new_y));
            }
        }
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = fs::read_to_string("labyrinth.json")?;

    let maze: Maze = serde_json::from_str(&data)?;
    
    let new_maze = to_vec(&maze);
    
    let start = maze.start;
    let end = maze.goal;
    let w = maze.width;
    let h = maze.height;

    match bfs(&new_maze, &start, &end, w as isize, h as isize) {
        Some(d) => println!("cel mai scurt drumulet: {d}"),
        None    => println!("nu s-a putut gasi nici un drum"),
    }
    Ok(())
}