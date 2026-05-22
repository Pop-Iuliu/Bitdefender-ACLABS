// evaluarea pozitiei, pozitia cu cel mai inalt scor va castiga

use crate::geometry::{chebyshev_distance, has_line_of_sight};
use crate::protocol::{GameState, Hero, Wall};

pub fn eval_position(
    pos: (i32, i32),
    hero: &Hero,
    state: &GameState,
    my_player_id: i32,
    ally_pos: (i32, i32),
    width: i32,
    height: i32,
) -> i32 {
    let mut score = 0;

    let enemies: Vec<&Hero> = state
        .heroes
        .iter()
        .filter(|h| h.owner_id != my_player_id && h.hp > 0)
        .collect();

    if enemies.is_empty() {
        return 0;
    }

    // no overlap :)
    if pos == ally_pos {
        score -= 400;
    }

    // sa fie coechipierul (ally) cat mai aproape ( <= 5 e ok)
    if ally_pos != (-1, -1) {
        let ally_dist = chebyshev_distance(pos, ally_pos);
        if ally_dist > 15 {
            score -= (ally_dist - 15) * 20;
        }
    }

    // cat mai departe de margini :)
    let edge_dist = pos
        .0
        .min(pos.1)
        .min(width - 1 - pos.0)
        .min(height - 1 - pos.1);
    if edge_dist < 10 {
        score -= (10 - edge_dist) * 30;
    }

    // la cooldown 0 devine mai agresiv gen
    if hero.cooldown == 0 {
        for enemy in &enemies {
            if has_line_of_sight(pos, (enemy.x, enemy.y), &state.walls) {
                score += 400;
                if has_line_of_sight(ally_pos, (enemy.x, enemy.y), &state.walls) {
                    score += 250;
                }
            }
        }
        // pastreaza distanta de shooting, nu te lipi
        if let Some(nearest_dist) = enemies
            .iter()
            .map(|e| chebyshev_distance(pos, (e.x, e.y)))
            .min()
        {
            let ideal = 12;
            score -= (nearest_dist - ideal).abs() * 10;
        }
    }
    for projectile in &state.projectiles {
        let proj_dist = chebyshev_distance(pos, (projectile.x, projectile.y));
        if proj_dist < 8 && projectile.owner_id != my_player_id {
            score -= (8 - proj_dist) * 500;
        }
    }

    if hero.cooldown > 0 {
        for enemy in &enemies {
            if has_line_of_sight(pos, (enemy.x, enemy.y), &state.walls) {
                score -= 2000;
            }
        }
        if let Some(nearest_dist) = enemies
            .iter()
            .map(|e| chebyshev_distance(pos, (e.x, e.y)))
            .min()
        {
            let ideal = 18;
            score -= (nearest_dist - ideal).abs() * 10;
        }
    }

    score
}

pub fn best_score(
    pos: (i32, i32),
    hero: &Hero,
    state: &GameState,
    my_player_id: i32,
    ally_pos: (i32, i32),
    walls: &[Wall],
    width: i32,
    height: i32,
    depth: i32,
) -> i32 {
    let mut step_penalty = 0;
    for projectile in &state.projectiles {
        let proj_dist = crate::geometry::chebyshev_distance(pos, (projectile.x, projectile.y));
        if proj_dist < 8 && projectile.owner_id != my_player_id {
            step_penalty -= (8 - proj_dist) * 500;
        }
    }

    if depth == 0 {
        return eval_position(pos, hero, state, my_player_id, ally_pos, width, height);
    }

    let moves = crate::bot::valid_moves(pos, walls, width, height);

    let future = moves
        .iter()
        .map(|m| {
            best_score(
                *m,
                hero,
                state,
                my_player_id,
                ally_pos,
                walls,
                width,
                height,
                depth - 1,
            )
        })
        .max()
        .unwrap_or(eval_position(
            pos,
            hero,
            state,
            my_player_id,
            ally_pos,
            width,
            height,
        ));

    step_penalty + future
}

pub fn predict_enemy_pos(
    enemy: &Hero,
    state: &GameState,
    _my_player_id: i32,
    walls: &[Wall],
    width: i32,
    height: i32,
) -> (i32, i32) {
    if enemy.cooldown == 0 {
        return (enemy.x, enemy.y);
    }
    let enemy_ally = state
        .heroes
        .iter()
        .find(|h| h.owner_id == enemy.owner_id && h.id != enemy.id && h.hp > 0);
    let enemy_ally_pos = enemy_ally.map(|a| (a.x, a.y)).unwrap_or((-1, -1));

    let moves = crate::bot::valid_moves((enemy.x, enemy.y), walls, width, height);

    moves
        .iter()
        .max_by_key(|pos| {
            eval_position(
                **pos,
                enemy,
                state,
                enemy.owner_id,
                enemy_ally_pos,
                width,
                height,
            )
        })
        .copied()
        .unwrap_or((enemy.x, enemy.y))
}
