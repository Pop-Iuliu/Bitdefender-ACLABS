use serde::Serialize;

use crate::Mode;
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

fn shootable(al_meu: &Hero, inamic: &Hero, walls: &[Wall]) -> bool {
    crate::geometry::has_line_of_sight((al_meu.x, al_meu.y), (inamic.x, inamic.y), walls)
}

pub struct MatchInfo {
    pub my_player_id: i32,
    pub width: i32,
    pub height: i32,
}

impl MatchInfo {
    pub fn new() -> Self {
        Self {
            my_player_id: 0,
            width: 0,
            height: 0,
        }
    }
}

pub struct Bot {
    pub info: MatchInfo,
    pub current_turn: i32,
    name: String,
    mode: Mode,
    seed: Option<u32>,
    my_id: i32,
    challenge_target: Option<String>,
    ranked: bool,
}

pub fn valid_moves(pos: (i32, i32), walls: &[Wall], width: i32, height: i32) -> Vec<(i32, i32)> {
    let d = [
        (0, 0),
        (3, 0),
        (-3, 0),
        (0, 3),
        (0, -3),
        (3, 3),
        (3, -3),
        (-3, 3),
        (-3, -3),
    ];

    let mut moves = Vec::new();
    for (dx, dy) in d {
        let nx = pos.0 + dx;
        let ny = pos.1 + dy;

        if nx < 0 || ny < 0 || nx >= width || ny >= height {
            continue;
        }

        if walls.iter().any(|w| w.x == nx && w.y == ny) {
            continue;
        }

        moves.push((nx, ny));
    }

    moves
}

impl Bot {
    pub fn new(
        name: String,
        mode: Mode,
        seed: Option<u32>,
        my_id: i32,
        challenge_target: Option<String>,
        ranked: bool,
    ) -> Self {
        Self {
            info: MatchInfo::new(),
            current_turn: 0,
            name,
            mode,
            seed,
            my_id,
            challenge_target,
            ranked,
        }
    }

    pub fn on_hello(&mut self, args: HelloArgs) -> Vec<ClientMessage> {
        vec![ClientMessage::Login(LoginArgs {
            name: self.name.clone(),
            version: args.version,
        })]
    }

    pub fn on_ready(&mut self) -> Vec<ClientMessage> {
        println!("gata de joaca cum s-ar zice");
        match self.mode {
            Mode::Practice => vec![ClientMessage::Practice(PracticeArgs {
                seed: self.seed,
                my_id: self.my_id,
            })],
            Mode::Challenge => vec![ClientMessage::Challenge(ChallengeArgs {
                name: self.challenge_target.clone(),
                seed: self.seed,
                ranked: Some(self.ranked),
            })],
        }
    }

    pub fn on_end_match(&mut self, args: EndMatchArgs) -> Vec<ClientMessage> {
        println!("gata meciul: {args:?}");
        vec![]
    }

    pub fn on_start_match(&mut self, args: StartMatchArgs) -> Vec<ClientMessage> {
        self.info.my_player_id = args.your_player_id;
        self.info.height = args.config.height;
        self.info.width = args.config.width;
        vec![]
    }

    pub fn on_start_turn(&mut self, args: StartTurnArgs) -> Vec<ClientMessage> {
        self.current_turn = args.turn;

        /*for hero in &args.state.heroes {
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

        self.enemies.retain(|_, mem| args.turn - mem.seen_turn <= 8);  */

        let mut predicted_state = args.state.clone();
        for h in &mut predicted_state.heroes {
            if h.owner_id != self.info.my_player_id && h.hp > 0 {
                let (px, py) = crate::evaluator::predict_enemy_pos(
                    h,
                    &args.state,
                    self.info.my_player_id,
                    &args.state.walls,
                    self.info.width,
                    self.info.height,
                );
                h.x = px;
                h.y = py;
            }
        }

        let mut orders: Vec<ClientMessage> = Vec::new();

        let start = std::time::Instant::now();

        for hero in &args.state.heroes {
            if hero.owner_id != self.info.my_player_id {
                continue;
            }

            let target = args
                .state
                .heroes
                .iter()
                .filter(|h| h.owner_id != self.info.my_player_id && h.hp > 0)
                .min_by_key(|h| {
                    let dist = (h.x - hero.x).abs().max((h.y - hero.y).abs());
                    (dist, h.hp)
                });

            let imminent_projectile = args
                .state
                .projectiles
                .iter()
                .any(|p| crate::geometry::chebyshev_distance((hero.x, hero.y), (p.x, p.y)) < 7);

            if let Some(t) = target {
                if hero.cooldown == 0
                    && !imminent_projectile
                    && shootable(hero, t, &args.state.walls)
                {
                    let predicted = crate::evaluator::predict_enemy_pos(
                        t,
                        &args.state,
                        self.info.my_player_id,
                        &args.state.walls,
                        self.info.width,
                        self.info.height,
                    );

                    let (shoot_x, shoot_y) = if crate::geometry::has_line_of_sight(
                        (hero.x, hero.y),
                        predicted,
                        &args.state.walls,
                    ) {
                        predicted
                    } else {
                        (t.x, t.y) // fallback la pozitia curenta
                    };

                    orders.push(ClientMessage::Shoot(ShootArgs {
                        hero_id: hero.id,
                        x: shoot_x,
                        y: shoot_y,
                    }));
                    continue;
                }
            }

            let enemies_visible: Vec<&Hero> = args
                .state
                .heroes
                .iter()
                .filter(|h| h.owner_id != self.info.my_player_id)
                .collect();

            if enemies_visible.is_empty() {
                let cx = (self.info.width / 2 / 3) * 3 + 1;
                let cy = (self.info.height / 2 / 3) * 3 + 1;

                if let Some((dx, dy)) = find_next_step(
                    (hero.x, hero.y),
                    (cx, cy),
                    &args.state.walls,
                    self.info.width,
                    self.info.height,
                ) {
                    orders.push(ClientMessage::Move(MoveArgs {
                        hero_id: hero.id,
                        x: hero.x + dx,
                        y: hero.y + dy,
                        comment: "👁️👁️".to_string(),
                    }));
                } else {
                    orders.push(ClientMessage::Move(MoveArgs {
                        hero_id: hero.id,
                        x: hero.x,
                        y: hero.y,
                        comment: "👁️👁️".to_string(),
                    }));
                }
                continue;
            }

            let ally_pos = args
                .state
                .heroes
                .iter()
                .find(|h| h.owner_id == self.info.my_player_id && h.id != hero.id)
                .map(|a| (a.x, a.y))
                .unwrap_or((-1, -1));

            let moves = valid_moves(
                (hero.x, hero.y),
                &args.state.walls,
                self.info.width,
                self.info.height,
            );

            let default = (hero.x, hero.y);
            let best = moves
                .iter()
                .max_by_key(|pos| {
                    crate::evaluator::best_score(
                        **pos,
                        hero,
                        &predicted_state,
                        self.info.my_player_id,
                        ally_pos,
                        &args.state.walls,
                        self.info.width,
                        self.info.height,
                        3,
                    )
                })
                .unwrap_or(&default);

            orders.push(ClientMessage::Move(MoveArgs {
                hero_id: hero.id,
                x: best.0,
                y: best.1,
                comment: "👀".to_string(),
            }));
            eprintln!(
                "mutarea {} erou {} -> ({},{}) scor={}",
                args.turn,
                hero.id,
                best.0,
                best.1,
                crate::evaluator::best_score(
                    *best,
                    hero,
                    &predicted_state,
                    self.info.my_player_id,
                    ally_pos,
                    &args.state.walls,
                    self.info.width,
                    self.info.height,
                    4,
                )
            );
        }

        eprintln!("mutarea {} a durat {:?}", args.turn, start.elapsed());
        orders
    }

    pub fn on_error(&mut self, args: &ErrorArgs) -> Vec<ClientMessage> {
        println!("Error: {} — {}", args.code, args.message);
        vec![]
    }
}
