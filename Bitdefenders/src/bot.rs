// Bot.rs

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::pathfinding::find_next_step;
use crate::protocol::{
    ChallengeArgs, EndMatchArgs, ErrorArgs, HelloArgs, Hero, LoginArgs, MoveArgs, PracticeArgs,
    ShootArgs, StartMatchArgs, StartTurnArgs, Wall,
};

#[derive(Debug, Serialize)]
#[serde(tag = "command", content = "args", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClientMessage {
    Login(LoginArgs),
    Practice(PracticeArgs),
    Challenge(ChallengeArgs),
    Move(MoveArgs),
    Shoot(ShootArgs),
}

pub fn hero_la_far(hero: &Hero, far_x: i32, far_y: i32) -> bool {
    hero.x == far_x && hero.y == far_y
}

pub fn shootable(al_meu: &Hero, inamic: &Hero, walls: &[Wall]) -> bool {
    let mut wall_set: HashSet<(i32, i32)> = HashSet::new();
    for w in walls {
        for dx in -1..=1 {
            for dy in -1..=1 {
                wall_set.insert((w.x + dx, w.y + dy));
            }
        }
    }
    let dx: i32 = (inamic.x - al_meu.x).abs();
    let dy: i32 = -(inamic.y - al_meu.y).abs();

    let sx: i32 = (inamic.x - al_meu.x).signum();
    let sy: i32 = (inamic.y - al_meu.y).signum();

    let mut err = dx + dy;

    let (mut x, mut y) = (al_meu.x, al_meu.y);
    let mut it = 0;
    loop {
        if (x - inamic.x).abs() <= 1 && (y - inamic.y).abs() <= 1 {
            break;
        }

        if wall_set.contains(&(x, y)) {
            return false;
        }
        if it > 1000 {
            println!("Warning : prea multe iterari (shootable fn)\n");
            return false;
        }
        it += 1;
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
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone)]
pub struct EnemyMemory {
    pub pos: Point,
    pub seen_turn: i32,
    pub hp: i32,
    pub cooldown: i32,
}

pub struct MatchInfo {
    pub my_player_id: i32,
    pub width: i32,
    pub height: i32,
    pub rally: (i32, i32),
    pub patrol_far: (i32, i32),
}

impl MatchInfo {
    pub fn new() -> Self {
        Self {
            my_player_id: 0,
            width: 0,
            height: 0,
            rally: (0, 0),
            patrol_far: (0, 0),
        }
    }
}

pub struct Bot {
    pub info: MatchInfo,
    pub going_down: HashMap<i32, bool>,
    pub second_hero_global: (i32, i32), // sorry for this
    pub enemies: HashMap<i32, EnemyMemory>,
    pub current_turn: i32,
}

impl Bot {
    pub fn new() -> Self {
        Self {
            info: MatchInfo::new(),
            going_down: HashMap::new(),
            second_hero_global: (0, 0),
            enemies: HashMap::new(),
            current_turn: 0,
        }
    }

    pub fn on_hello(&mut self, args: HelloArgs) -> Vec<ClientMessage> {
        vec![ClientMessage::Login(LoginArgs {
            name: "Pop-Iuliu".into(),
            version: args.version,
        })]
    }

    pub fn on_ready(&mut self) -> Vec<ClientMessage> {
        println!("gata de joaca cum s-ar zice");
        vec![ClientMessage::Practice(PracticeArgs {
            seed: None,
            my_id: 1,
        })]
    }

    pub fn on_end_match(&mut self, args: EndMatchArgs) -> Vec<ClientMessage> {
        println!("gata meciul: {args:?}");
        vec![]
    }

    pub fn on_start_match(&mut self, args: StartMatchArgs) -> Vec<ClientMessage> {
        self.info.my_player_id = args.your_player_id;
        self.info.height = args.config.height;
        self.info.width = args.config.width;

        let my_player = args
            .config
            .players
            .iter()
            .find(|p| p.id == self.info.my_player_id)
            .expect("nu ma gasesc");

        let second_hero = &my_player.heroes[1];
        self.second_hero_global = (second_hero.x, second_hero.y);
        self.info.rally = (second_hero.x, second_hero.y);

        let enemy_player = args
            .config
            .players
            .iter()
            .find(|p| p.id != self.info.my_player_id)
            .expect("nu gasesc adversarul");

        let enemy_avg_y =
            enemy_player.heroes.iter().map(|h| h.y).sum::<i32>() / enemy_player.heroes.len() as i32;

        let target_y = if enemy_avg_y > self.info.rally.1 {
            self.info.height - 2
        } else {
            1
        };

        self.info.patrol_far = (self.info.rally.0, target_y);

        vec![]
    }

    pub fn on_start_turn(&mut self, args: StartTurnArgs) -> Vec<ClientMessage> {
        self.current_turn = args.turn;

        for hero in &args.state.heroes {
            if hero.owner_id == self.info.my_player_id {
                continue;
            }

            self.enemies.insert(
                hero.id,
                EnemyMemory {
                    pos: Point {
                        x: hero.x,
                        y: hero.y,
                    },
                    seen_turn: args.turn,
                    hp: hero.hp,
                    cooldown: hero.cooldown,
                },
            );
        }

        self.enemies.retain(|_, mem| args.turn - mem.seen_turn <= 8);
        let mut orders: Vec<ClientMessage> = Vec::new();

        let target = args
            .state
            .heroes
            .iter()
            .find(|h| h.owner_id != self.info.my_player_id);

        for hero in &args.state.heroes {
            if hero.owner_id != self.info.my_player_id {
                continue;
            }

            if let Some(t) = target
                && hero.cooldown == 0
                && shootable(hero, t, &args.state.walls)
            {
                orders.push(ClientMessage::Shoot(ShootArgs {
                    hero_id: hero.id,
                    x: t.x,
                    y: t.y,
                }));
                continue;
            }

            let (rally_x, rally_y) = self.info.rally;
            let (far_x, far_y) = self.info.patrol_far;

            let altul = args
                .state
                .heroes
                .iter()
                .find(|h| h.owner_id == self.info.my_player_id && h.id != hero.id);

            let am_la_rally = hero.x == rally_x && hero.y == rally_y;

            if am_la_rally && let Some(altul) = altul {
                let dist = (altul.x - hero.x).abs().max((altul.y - hero.y).abs());
                if dist > 12
                    && (hero.x, hero.y) == (self.second_hero_global.0, self.second_hero_global.1)
                {
                    orders.push(ClientMessage::Move(MoveArgs {
                        hero_id: hero.id,
                        x: hero.x,
                        y: hero.y,
                        comment: "hmm stau 👀".to_string(),
                    }));
                    continue;
                }
            }

            let (dest_x, dest_y) = if hero.x != rally_x || hero.y != rally_y {
                (rally_x, rally_y)
            } else {
                self.going_down.insert(hero.id, true);
                (far_x, far_y)
            };

            let (final_x, final_y) = if let Some(&going_down) = self.going_down.get(&hero.id) {
                if going_down && hero_la_far(hero, far_x, far_y) {
                    self.going_down.insert(hero.id, false);
                    (rally_x, rally_y)
                } else if !going_down && hero.x == rally_x && hero.y == rally_y {
                    self.going_down.insert(hero.id, true);
                    (far_x, far_y)
                } else if going_down {
                    (far_x, far_y)
                } else {
                    (rally_x, rally_y)
                }
            } else {
                (dest_x, dest_y)
            };

            let next_step = find_next_step(
                (hero.x, hero.y),
                (final_x, final_y),
                &args.state.walls,
                self.info.width,
                self.info.height,
            );

            if let Some((dx, dy)) = next_step {
                let new_x = hero.x + dx;
                let new_y = hero.y + dy;

                orders.push(ClientMessage::Move(MoveArgs {
                    hero_id: hero.id,
                    x: new_x,
                    y: new_y,
                    comment: "te voi prinde 👀".to_string(),
                }));
            }
        }
        orders
    }

    pub fn on_error(&mut self, args: &ErrorArgs) -> Vec<ClientMessage> {
        println!("Error: {} — {}", args.code, args.message);
        vec![]
    }
}
