use anyhow::Context;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
mod protocol;
use crate::protocol::{HelloArgs, LoginArgs, StartMatchArgs, StartTurnArgs};

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketMessage {
    command: Command,
    args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Command {
    Hello,
    Login,
    Error,
    Ready,
    Practice,
    Challenge,
    StartMatch,
    StartTurn,
    EndMatch,
    Move,
}
async fn send_command<
    S: SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
>(
    write: &mut S,
    msg: WebSocketMessage,
) -> anyhow::Result<()> {
    let msg_deserialized = serde_json::to_string(&msg).context("serialize message")?;
    write
        .send(Message::Text(msg_deserialized.into()))
        .await
        .context("send message")?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let url = "wss://bitdefenders.cvjd.me/ws";
    let (ws, _) = connect_async(url).await.unwrap();
    let (mut write, mut read) = ws.split();

    println!("connected");
    let mut my_player_id:i32 = 0;


    // directions 

    while let Some(msg) = read.next().await {
        let msg = msg.unwrap();
        let message: WebSocketMessage = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        println!("{message:?}");
        match message.command {
            Command::Hello => {
                // Send login
                if let Err(e) = send_command(
                    &mut write,
                    WebSocketMessage {
                        command: Command::Login,
                        args: serde_json::json!({"version": 1, "name": "Pop-Iuliu"}),
                    },
                )
                .await {
                    println!("Failed to send login command: {e}");
                    break;
                }
            }
            Command::Login => {
                panic!("What are you doing here?");
            },
            Command::Error => {
                println!("Error: {message:?}");
                break;
            }
            Command::Ready => {
                println!("You are ready to play!");
                if let Err(e) = send_command(
                    &mut write,
                    WebSocketMessage {
                        command: Command::Practice,
                        args: serde_json::json!({}),
                    },
                )
                .await {
                    println!("Failed to start practice: {e}");
                    break;
                }
            },
            Command::Practice => { 
                if let Err(e) = send_command(
                    &mut write,
                    WebSocketMessage {
                        command: Command::Practice,
                        args: serde_json::json!({}),
                    },
                )
                .await {
                    println!("Failed to start practice: {e}");
                    break;
                }
            },
            Command::StartMatch => {
                let response_start: StartMatchArgs = serde_json::from_value(message.args.clone())
                    .expect("parse StartMatchArgs");
                println!("started the match: {response_start:?}");
                my_player_id = response_start.your_player_id;
            }, 
            Command::EndMatch => {
                println!("ended the match ! \n");
            },
            Command::Move => {
                println!("nope (;\n");

            },
            Command::StartTurn => {
                let response: StartTurnArgs = serde_json::from_value(message.args.clone())
                    .expect("parse StartTurnArgs");

                let directions: [(i32, i32); 8] = [
                    (-1, -1), (-1, 0),(-1, 1),
                    (0, -1), (0, 1),
                    (1, -1), (1, 0), (1, 1),
                ];

                for hero in &response.state.heroes {
                    if hero.owner_id != my_player_id { continue; }

                    for (dx, dy) in directions {
                        let new_x = hero.x + dx;
                        let new_y = hero.y + dy;

                        let blocked = response.state.walls.iter().any(|w| w.x == new_x && w.y == new_y);

                        if !blocked {
                            if let Err(e) = send_command(&mut write, WebSocketMessage {
                                command: Command::Move,
                                args: serde_json::json!({
                                    "hero_id": hero.id,
                                    "x": new_x,
                                    "y": new_y,
                                }),
                            }).await {
                                println!("failed to send move: {e}");
                                break;
                            }
                            println!("DEBUG {}, {}", new_x, new_y);
                            break; 
                        }
                    }
                }
        },
            Command::Challenge => {
                if let Err(e) = send_command(
                    &mut write,
                    WebSocketMessage {
                        command: Command::Practice,
                        args: serde_json::json!({"seed": 2, "name": "miron-victor"}),
                    },
                )
                .await {
                    println!("Failed to start practice: {e}");
                    break;
                }
            }
        }
    }
}