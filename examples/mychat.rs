use core::fmt;
use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use dashmap::DashMap;
use futures::stream::{SplitStream, StreamExt};
use futures::SinkExt;

use tokio::sync::mpsc;
use tokio::{net::TcpStream, sync::mpsc::Sender};
use tokio_util::codec::{Framed, LinesCodec};
use tracing::level_filters::LevelFilter;
use tracing::{info, warn};
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer as _;
const CHANNEL_BUFFER_SIZE: usize = 32;
struct AppState {
    /// A map of all connected peers.
    /// we'll find a peer by its address. then we can send messages to it.
    peers: DashMap<SocketAddr, Sender<Arc<Message>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            peers: DashMap::new(),
        }
    }
}

impl AppState {
    async fn on_user_join(
        &self,
        name: String,
        addr: SocketAddr,
        stream: Framed<TcpStream, LinesCodec>,
    ) -> Result<SplitStream<Framed<TcpStream, LinesCodec>>> {
        // we should use channel to send message to peer
        let (tx, mut rx) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        self.peers.insert(addr, tx);
        // split stream to reader and writer
        let (mut sender, reader) = stream.split();

        // just receive from channel and send to client
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = sender.send(message.to_string()).await {
                    warn!("Failed to send message to {}: {:?}", addr, e);
                    break;
                }
            }
        });

        // should broadcast to all peers
        let join_message = Arc::new(Message::user_joined(&name));
        info!("{}", join_message);
        self.broadcast(addr, &join_message).await;
        Ok(reader)
    }

    async fn on_user_leave(&self, name: String, addr: SocketAddr) {
        self.peers.remove(&addr);
        let leave_message = Arc::new(Message::user_left(&name));
        info!("{}", leave_message);
        self.broadcast(addr, &leave_message).await;
    }

    // when user send a message. we broadcast it to all peers except the sender
    async fn broadcast(&self, addr: SocketAddr, message: &Arc<Message>) {
        for peer in self.peers.iter() {
            if peer.key() == &addr {
                continue;
            }
            if let Err(e) = peer.value().send(message.clone()).await {
                warn!("Failed to send message to {}: {:?}", peer.key(), e);
                self.peers.remove(peer.key());
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // tracing
    let layer = Layer::new().pretty().with_filter(LevelFilter::INFO);
    tracing_subscriber::registry().with(layer).init();

    let addr = "0.0.0.0:8000";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Listening on: {}", addr);

    // state manage all connected peers
    let state = Arc::new(AppState::default());
    // The server listens for incoming connections and spawns a new task for each one.
    loop {
        let state_clone = Arc::clone(&state);
        // The listener accepts a new connection and returns a new TcpStream.
        let (stream, addr) = listener.accept().await?;
        info!("Accepted connection from: {}", addr);
        tokio::spawn(async move {
            if let Err(e) = handle_client(state_clone, addr, stream).await {
                warn!("handle_client Error: {:?}", e);
            }
        });
    }
}

#[derive(Debug)]
enum Message {
    Chat(String, String),
    UserJoined(String),
    UserLeft(String),
}

impl Message {
    fn chat(username: String, content: String) -> Self {
        Self::Chat(username, content)
    }

    fn user_joined(username: &str) -> Self {
        Self::UserJoined(username.to_string())
    }

    fn user_left(username: &str) -> Self {
        Self::UserLeft(username.to_string())
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chat(username, content) => write!(f, "{}: {}", username, content),
            Self::UserJoined(username) => write!(f, "[>>{}] joined the chat", username),
            Self::UserLeft(username) => write!(f, "[<<{}] left the chat", username),
        }
    }
}
// The handle_client function reads data from the client and writes it back.
async fn handle_client(
    state: Arc<AppState>,
    addr: SocketAddr,
    stream: tokio::net::TcpStream,
) -> Result<()> {
    // prompt for username

    // line framed codec
    let mut frame = Framed::new(stream, tokio_util::codec::LinesCodec::new());
    frame.send("Enter your username:").await?;

    // get name from frame
    let username = match frame.next().await {
        Some(Ok(username)) => username,
        Some(Err(e)) => return Err(e.into()),
        _ => {
            return Err(anyhow::anyhow!("Failed to read username"));
        }
    };
    // join the chat
    let mut reader = state.on_user_join(username.clone(), addr, frame).await?;
    // receive message from peer, then broadcast
    while let Some(message) = reader.next().await {
        let message = match message {
            Ok(message) => message,
            Err(e) => {
                warn!("Failed to read line from {}: {:?}", addr, e);
                break;
            }
        };
        let message = Arc::new(Message::chat(username.clone(), message));
        state.broadcast(addr, &message).await;
    }

    // here leave the chat
    state.on_user_leave(username, addr).await;
    Ok(())
}
