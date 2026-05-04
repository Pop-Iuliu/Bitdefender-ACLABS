use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
mod pathfinding;
use crate::pathfinding::find_next_step;
mod protocol;
use crate::protocol::{
    ChallengeArgs, EndMatchArgs, ErrorArgs, HelloArgs, Hero, LoginArgs, MoveArgs, PracticeArgs,
    ReadyArgs, ShootArgs, StartMatchArgs, StartTurnArgs, Wall,
};
use std::collections::{HashMap, HashSet};

const DEBUG: bool = false; // !!! Debug pt a afisa ce trimitem catre websocket

#[derive(Debug, Deserialize)]
#[serde(tag = "command", content = "args", rename_all = "SCREAMING_SNAKE_CASE")]
enum ServerMessage {
    Hello(HelloArgs),
    Ready(ReadyArgs),
    StartMatch(StartMatchArgs),
    StartTurn(StartTurnArgs),
    EndMatch(EndMatchArgs),
    Error(ErrorArgs),
}

#[derive(Debug, Serialize)]
#[serde(tag = "command", content = "args", rename_all = "SCREAMING_SNAKE_CASE")]
enum ClientMessage {
    Login(LoginArgs),
    Practice(PracticeArgs),
    Challenge(ChallengeArgs),
    Move(MoveArgs),
    Shoot(ShootArgs),
}

fn shootable(al_meu: &Hero, inamic: &Hero, walls: &[Wall]) -> bool {
    let wall_set: HashSet<(i32, i32)> = walls.iter().map(|w| (w.x, w.y)).collect();
    let dx: i32 = (inamic.x - al_meu.x).abs();
    let dy: i32 = -(inamic.y - al_meu.y).abs();

    let signum = |x, y| if x < y { 1 } else { -1 };
    let sx: i32 = signum(al_meu.x, inamic.x);
    let sy: i32 = signum(al_meu.y, inamic.y);

    let mut err: i128 = (dx + dy) as i128;

    let (mut x, mut y) = (al_meu.x, al_meu.y);
    let mut it = 0;
    while true {
        if (x, y) == (inamic.x, inamic.y) {
            break;
        }

        if wall_set.contains(&(x, y)) {
            return false;
        }
        if it > 1000 {
            println!("Warning : prea multe iterari (shootable fn)\n");
            return true; // hmm
        }
        it += 1;
        let e2: i128 = 2 * err;

        if e2 >= (dy as i128) {
            err += dy as i128;
            x += sx * 3;
        }

        if e2 <= (dx as i128) {
            err += dx as i128;
            y += sy * 3;
        }
    }
    true
}

struct MatchInfo {
    my_player_id: i32,
    width: i32,
    height: i32,
    rally: (i32, i32),
}

impl MatchInfo {
    fn new() -> Self {
        Self {
            my_player_id: 0,
            width: 0,
            height: 0,
            rally: (0, 0),
        }
    }
}

struct Bot {
    info: MatchInfo,
    going_down: HashMap<i32, bool>,
    second_hero_global: (i32, i32),
}

impl Bot {
    fn new() -> Self {
        Self {
            info: MatchInfo::new(),
            going_down: HashMap::new(),
            second_hero_global: (0, 0),
        }
    }

    fn on_hello(&mut self, args: HelloArgs) -> Vec<ClientMessage> {
        vec![ClientMessage::Login(LoginArgs {
            name: "Pop-Iuliu".into(),
            version: args.version,
        })]
    }

    fn on_ready(&mut self) -> Vec<ClientMessage> {
        println!("gata de joaca cum s-ar zice");
        vec![ClientMessage::Practice(PracticeArgs { seed: None })]
    }

    fn on_end_match(&mut self, args: EndMatchArgs) -> Vec<ClientMessage> {
        println!("gata meciul: {args:?}");
        vec![]
    }

    fn on_start_match(&mut self, args: StartMatchArgs) -> Vec<ClientMessage> {
        self.info.my_player_id = args.your_player_id;
        self.info.height = args.config.height;
        self.info.width = args.config.width;

        let my_player = args
            .config
            .players
            .iter()
            .find(|p| p.id == self.info.my_player_id);

        let second_hero = &my_player.unwrap().heroes[1]; // al doilea hero
        self.second_hero_global = (second_hero.x, second_hero.y);
        self.info.rally = (second_hero.x, second_hero.y);

        vec![]
    }

    fn on_start_turn(&mut self, args: StartTurnArgs) -> Vec<ClientMessage> {
        let mut orders: Vec<ClientMessage> = Vec::new();

        // caut inamic vizibil
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
                (rally_x, self.info.height - 2)
            };

            let (final_x, final_y) = if let Some(&going_down) = self.going_down.get(&hero.id) {
                if going_down && hero.y >= self.info.height - 2 {
                    self.going_down.insert(hero.id, false);
                    (rally_x, rally_y)
                } else if !going_down && hero.y <= rally_y {
                    self.going_down.insert(hero.id, true);
                    (rally_x, self.info.height - 2)
                } else if going_down {
                    (rally_x, self.info.height - 2)
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
        if DEBUG {
            println!("{:?}", orders);
        }
        orders
    }

    fn on_error(&mut self, args: &ErrorArgs) -> Vec<ClientMessage> {
        println!("Error: {} — {}", args.code, args.message);
        vec![]
    }
}

async fn send_orders<S>(write: &mut S, orders: Vec<ClientMessage>) -> anyhow::Result<()>
where
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    if orders.is_empty() {
        return Ok(());
    }

    let ws_messages: Vec<Message> = orders
        .into_iter()
        .map(|o| {
            let json = serde_json::to_string(&o).expect("serialize ClientMessage");
            Message::Text(json)
        })
        .collect();

    write
        .send_all(&mut futures_util::stream::iter(ws_messages).map(Ok))
        .await
        .context("send_all orders")?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let url = "wss://bitdefenders.cvjd.me/ws";
    let (ws, _) = connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    println!("connected");
    let mut bot = Bot::new();

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(e) => {
                println!("websocket error: {e}");
                break;
            }
        };

        let text = match msg {
            Message::Text(t) => t,
            Message::Ping(_) => {
                // aparent tokio raspunde cu pong dirct
                continue;
            }
            Message::Pong(_) => continue,
            Message::Close(frame) => {
                println!("{frame:?}");
                break;
            }
            Message::Binary(_) | Message::Frame(_) => {
                println!("woopsi: {msg:?}");
                continue;
            }
        };

        let server_msg: ServerMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                println!("mesaj rau {text:?}: {e}");
                continue;
            }
        };

        let orders: Vec<ClientMessage> = match server_msg {
            ServerMessage::Hello(args) => bot.on_hello(args),
            ServerMessage::Ready(_) => bot.on_ready(),
            ServerMessage::StartMatch(args) => bot.on_start_match(args),
            ServerMessage::StartTurn(args) => bot.on_start_turn(args),
            ServerMessage::EndMatch(args) => bot.on_end_match(args),
            ServerMessage::Error(args) => {
                let orders = bot.on_error(&args);
                if args.fatal {
                    break;
                }
                orders
            }
        };
        if let Err(e) = send_orders(&mut write, orders).await {
            println!("failed to send orders: {e}");
            break;
        }
    }
}
