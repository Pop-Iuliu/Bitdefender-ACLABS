use anyhow::Context;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};
mod pathfinding;
mod protocol;
use crate::protocol::{
    EndMatchArgs, ErrorArgs, HelloArgs, ReadyArgs, StartMatchArgs, StartTurnArgs,
};
mod bot;
use crate::bot::{Bot, ClientMessage};

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
