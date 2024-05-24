use std::sync::Arc;
use std::{fmt, net::SocketAddr};

use anyhow::Result;
use dashmap::DashMap;

use futures::stream::SplitStream;
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::codec::{Framed, LinesCodec};
use tracing::{info, warn};

const MAX_MESSAGES: usize = 128;
#[derive(Debug, Default)]
struct State {
    peers: DashMap<SocketAddr, mpsc::Sender<Arc<Message>>>,
}

#[derive(Debug)]
enum Message {
    UserJoined(String),
    UserLeft(String),
    Chat { sender: String, content: String },
}

#[derive(Debug)]
struct Peer {
    username: String,
    stream: SplitStream<Framed<TcpStream, LinesCodec>>,
}
#[tokio::main]
async fn main() -> Result<()> {
    // let layer = Layer::new().pretty().with_filter(LevelFilter::INFO);
    // tracing_subscriber::registry().with(layer).init();
    console_subscriber::init();

    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on {}", addr);
    let state = Arc::new(State::default());
    loop {
        let (client, addr) = listener.accept().await?;
        info!("Accepted connection from: {}", addr);
        let cloned_state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = handle_client(cloned_state, addr, client).await {
                warn!("Failed to  handle client {}: {:?}", addr, e);
            }
            Ok::<(), anyhow::Error>(())
        });
    }
}
async fn handle_client(state: Arc<State>, addr: SocketAddr, stream: TcpStream) -> Result<()> {
    let mut stream = Framed::new(stream, LinesCodec::new());
    stream.send("Enter your username:").await?; // send to client

    // read from client
    let username = match stream.next().await {
        Some(Ok(username)) => username,
        Some(Err(e)) => return Err(e.into()),
        None => return Ok(()),
    };
    let mut peer = state.add(addr, username, stream).await;

    // notify others that  a new user has joined
    let message = Arc::new(Message::user_joined(&peer.username));
    info!("{}", message);
    state.broadcast(addr, &message).await;

    // broadcast messages from the client to others
    while let Some(line) = peer.stream.next().await {
        let content = match line {
            Ok(line) => line,
            Err(e) => {
                warn!("Failed to read line from {}: {:?}", addr, e);
                break;
            }
        };
        let message = Arc::new(Message::chat(peer.username.clone(), content));
        state.broadcast(addr, &message).await;
    }
    state.peers.remove(&addr);
    let message = Arc::new(Message::user_left(&peer.username));
    info!("{}", message);
    state.broadcast(addr, &message).await;
    Ok(())
}
impl State {
    async fn broadcast(&self, addr: SocketAddr, message: &Arc<Message>) {
        for peer in self.peers.iter() {
            if peer.key() == &addr {
                continue;
            }
            if let Err(e) = peer.value().send(message.clone()).await {
                warn!("Failed to send message to:{}: {:?}", peer.key(), e);
                // if send failed, peer might be gone
                self.peers.remove(peer.key());
            }
        }
    }

    async fn add(
        &self,
        addr: SocketAddr,
        username: String,
        stream: Framed<TcpStream, LinesCodec>,
    ) -> Peer {
        let (tx, mut rx) = mpsc::channel(MAX_MESSAGES);
        self.peers.insert(addr, tx);

        let (mut stream_sender, stream_receiver) = stream.split();
        // receive messages from the others, and send them to the client
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                // send to client
                // state -> peer -> client
                if let Err(e) = stream_sender.send(message.to_string()).await {
                    warn!("Failed to send message to {}: {:?}", addr, e);
                    break;
                }
            }
        });
        // return a peer
        Peer {
            username,
            stream: stream_receiver,
        }
    }
}

impl Message {
    fn user_joined(username: &str) -> Self {
        let content = format!("{} has joined the chat", username);
        Self::UserJoined(content)
    }

    fn user_left(username: &str) -> Self {
        let content = format!("{} has left the chat", username);
        Self::UserLeft(content)
    }

    fn chat(sender: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Chat {
            sender: sender.into(),
            content: content.into(),
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserJoined(content) => write!(f, "[{}]", content),
            Self::UserLeft(content) => write!(f, "[{} :(]", content),
            Self::Chat { sender, content } => write!(f, "{}: {}", sender, content),
        }
    }
}
