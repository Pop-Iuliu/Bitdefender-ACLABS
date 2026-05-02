use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
mod protocol;
use crate::protocol::{
    ChallengeArgs, EndMatchArgs, ErrorArgs, HelloArgs, LoginArgs, MoveArgs, PracticeArgs,
    ReadyArgs, ShootArgs, StartMatchArgs, StartTurnArgs,
};

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

struct MatchInfo {
    my_player_id: i32,
    width: i32,
    height: i32,
}

impl MatchInfo {
    fn new() -> Self {
        Self {
            my_player_id: 0,
            width: 0,
            height: 0,
        }
    }
}

struct Bot {
    info: MatchInfo,
}

impl Bot {
    fn new() -> Self {
        Self {
            info: MatchInfo::new(),
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
        // daca e sa dau challenge, schimb de aici
        vec![ClientMessage::Practice(PracticeArgs { seed: None })]
    }

    fn on_end_match(&mut self, args: EndMatchArgs) -> Vec<ClientMessage> {
        println!("gata meciul: {args:?}");
        vec![]
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
            Message::Text(json.into())
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
        let msg = msg.unwrap();
        let text = msg.to_text().unwrap();
        let server_msg: ServerMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                println!("bad message {text:?}: {e}");
                continue;
            }
        };
        println!("{server_msg:?}");

        let orders: Vec<ClientMessage> = match server_msg {
            ServerMessage::Hello(args) => bot.on_hello(args),
            ServerMessage::Ready(_) => bot.on_ready(),
            ServerMessage::EndMatch(args) => bot.on_end_match(args),
            ServerMessage::Error(args) => {
                let orders = bot.on_error(&args);
                if args.fatal {
                    break;
                }
                orders
            }
            ServerMessage::StartMatch(args) => {
                println!("a inceput meciul");

                bot.info.my_player_id = args.your_player_id;
                bot.info.height = args.config.height;
                bot.info.width = args.config.width;
                vec![]
            }
            ServerMessage::StartTurn(args) => {
                let mut orders: Vec<ClientMessage> = Vec::new();

                // SHOOT
                for hero in &args.state.heroes {
                    if hero.owner_id != bot.info.my_player_id {
                        continue;
                    }
                    if hero.cooldown != 0 {
                        continue;
                    }

                    if let Some(target) = args
                        .state
                        .heroes
                        .iter()
                        .find(|h| h.owner_id != bot.info.my_player_id)
                    {
                        orders.push(ClientMessage::Shoot(ShootArgs {
                            hero_id: hero.id,
                            x: target.x,
                            y: target.y,
                        }));
                    }
                }

                // colectez id celor ce au tras deja
                let shooters: Vec<i32> = orders
                    .iter()
                    .filter_map(|o| match o {
                        ClientMessage::Shoot(s) => Some(s.hero_id),
                        _ => None,
                    })
                    .collect();

                // MOVE
                let directions: [(i32, i32); 8] = [
                    (3, 3),
                    (3, 0),
                    (3, -3),
                    (0, 3),
                    (0, -3),
                    (-3, 3),
                    (-3, 0),
                    (-3, -3),
                ];

                for hero in &args.state.heroes {
                    if hero.owner_id != bot.info.my_player_id {
                        continue;
                    }
                    if shooters.contains(&hero.id) {
                        continue;
                    }

                    for (dx, dy) in directions {
                        let new_x = hero.x + dx;
                        let new_y = hero.y + dy;

                        if new_x < 0
                            || new_x > bot.info.width
                            || new_y < 0
                            || new_y > bot.info.height
                        {
                            continue;
                        }

                        let blocked = args
                            .state
                            .walls
                            .iter()
                            .any(|w| w.x == new_x && w.y == new_y);

                        if !blocked {
                            orders.push(ClientMessage::Move(MoveArgs {
                                hero_id: hero.id,
                                x: new_x,
                                y: new_y,
                            }));
                            break;
                        }
                    }
                }

                orders
            }
        };

        if let Err(e) = send_orders(&mut write, orders).await {
            println!("oops {e}");
            break;
        }
    }
}
