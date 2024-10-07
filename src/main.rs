use axum::{extract::State, http::StatusCode, routing::get, Router};
use core::error::Error;
use core::future::IntoFuture;
use state_actor::{gitlab_tokens_actor, ActorMessage, AppState};
use tokio::{
    net::TcpListener,
    select,
    signal::unix::{signal, SignalKind},
    sync::{mpsc, oneshot},
};

mod gitlab;
mod prometheus_metrics;
mod state_actor;

async fn root_handler() -> &'static str {
    "I'm Alive :D"
}

async fn get_gitlab_tokens_handler(State(state): State<AppState>) -> (StatusCode, String) {
    // We are going to send a message to our actor and wait for an answer
    // But first, we create a oneshot channel to get the actor's response
    let (send, recv) = oneshot::channel();
    let msg = ActorMessage::GetResponse { respond_to: send };

    // Ignore send errors. If this send fails, so does the
    // recv.await below. There's no reason to check for the
    // same failure twice.
    #[expect(clippy::let_underscore_must_use, reason = "Ignore send errors")]
    #[expect(clippy::let_underscore_untyped, reason = "Ignore send errors type")]
    let _ = state.sender.send(msg).await;

    match recv.await {
        Ok(res) => match res.len() {
            0 => (StatusCode::NO_CONTENT, res),
            _ => (StatusCode::OK, res),
        },
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

#[expect(
    clippy::integer_division_remainder_used,
    reason = "Because clippy is not happy with the tokio::select macro #1"
)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "Because clippy is not happy with the tokio::select macro #2"
)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    // An infinite stream of 'SIGTERM' signals.
    let mut sigterm_stream = signal(SignalKind::terminate())?;

    // Create a channel and then an actor
    let (sender, receiver) = mpsc::channel(8);
    let actor_handle = tokio::spawn(gitlab_tokens_actor(receiver));

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/metrics", get(get_gitlab_tokens_handler))
        .with_state(AppState { sender });

    let listener = TcpListener::bind("0.0.0.0:3000").await?;

    println!("listening on {}", listener.local_addr()?);

    // Waiting for one of the following :
    // - a SIGTERM signal
    // - the actor to finish/panic
    // - the axum server to finish
    select! {
        _ = sigterm_stream.recv() => {
            Err(Box::from("Received a SIGTERM signal! exiting."))
        },
        res = actor_handle => {
            match res {
                Ok(msg) => { println!("{msg}"); }
                Err(err) => { println!("{err}"); }
            }
            Err(Box::from("The actor died! exiting."))
        },
        _ = axum::serve(listener, app).into_future() => {
            Err(Box::from("The server died! exiting."))
        }
    }
}
