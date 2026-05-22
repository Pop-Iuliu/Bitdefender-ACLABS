use anyhow::Context;
use clap::{Parser, ValueEnum};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};
mod geometry;
mod pathfinding;
mod protocol;
use crate::protocol::{
    EndMatchArgs, ErrorArgs, HelloArgs, ReadyArgs, StartMatchArgs, StartTurnArgs,
};
mod bot;
use crate::bot::{Bot, ClientMessage};
mod evaluator;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Mode {
    #[default]
    Practice,
    Challenge,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "demo-client")]
struct CliConfig {
    /// WebSocket URL (overrides env `WS_URL` when set on the command line).
    #[arg(
        long = "url",
        env = "WS_URL",
        default_value = "wss://bitdefenders.cvjd.me/ws"
    )]
    ws_url: String,
    /// Display name (env: `PLAYER_NAME`).
    #[arg(short, long, env = "PLAYER_NAME", default_value = "Pop-Iuliu")]
    name: String,
    #[arg(short = 'm', long, value_enum, default_value_t = Mode::Practice)]
    mode: Mode,
    #[arg(long)]
    seed: Option<u32>,
    /// Practice: player slot 0 or 1 (spawn side).
    #[arg(long, default_value_t = 0, value_parser = parse_my_id)]
    my_id: i32,
    #[arg(long = "target", visible_alias = "challenge-target")]
    challenge_target: Option<String>,
    /// Challenge: queue for ranked matchmaking.
    #[arg(long)]
    ranked: bool,
    /// How many matches to play before exit.
    #[arg(long, default_value_t = 1)]
    matches: u32,
}

fn parse_my_id(s: &str) -> Result<i32, String> {
    let n: i32 = s
        .parse()
        .map_err(|_| format!("`{s}`: not a valid integer"))?;
    if n == 0 || n == 1 {
        Ok(n)
    } else {
        Err("must be 0 or 1".into())
    }
}

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
    let cfg = CliConfig::parse();

    for match_num in 1..=cfg.matches {
        if cfg.matches > 1 {
            println!("match {match_num}/{}", cfg.matches);
        }

        let (ws, _) = connect_async(&cfg.ws_url).await.unwrap();
        let (mut write, mut read) = ws.split();
        println!("connected");

        let mut bot = Bot::new(
            cfg.name.clone(),
            cfg.mode,
            cfg.seed,
            cfg.my_id,
            cfg.challenge_target.clone(),
            cfg.ranked,
        );

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
                Message::Ping(_) => continue,
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
}
