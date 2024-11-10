//! Export the number of days before GitLab tokens expire as Prometheus metrics.

use axum::{extract::State, http::StatusCode, routing::get, Router};
use core::future::IntoFuture;
use std::process::ExitCode;
use tokio::{
    net::TcpListener,
    select,
    signal::unix::{signal, SignalKind},
    sync::{mpsc, oneshot},
};
use tracing::{error, info, instrument};

use crate::state_actor::{gitlab_tokens_actor, ActorState, StateActorMessage};
use crate::timer::spawn_timer_actor;

mod gitlab;
mod prometheus_metrics;
mod state_actor;
mod timer;

/// Static response for requests on `/`
async fn root_handler() -> &'static str {
    "I'm Alive :D"
}

async fn get_gitlab_tokens_handler(
    State(sender): State<mpsc::Sender<StateActorMessage>>,
) -> (StatusCode, String) {
    // We are going to send a message to our actor and wait for an answer
    // But first, we create a oneshot channel to get the actor's response
    let (send, recv) = oneshot::channel();
    let msg = StateActorMessage::Get { respond_to: send };

    // Ignore send errors. If this send fails, so does the
    // recv.await below. There's no reason to check for the
    // same failure twice.
    #[expect(clippy::let_underscore_must_use, reason = "Ignore send errors")]
    #[expect(clippy::let_underscore_untyped, reason = "Ignore send errors type")]
    let _ = sender.send(msg).await;

    match recv.await {
        Ok(res) => match res {
            ActorState::Loading => (StatusCode::NO_CONTENT, String::new()),
            ActorState::Loaded(state) => (StatusCode::OK, state),
            ActorState::Error(err) => (StatusCode::INTERNAL_SERVER_ERROR, err),
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
#[instrument]
async fn main() -> ExitCode {
    #[expect(clippy::absolute_paths, reason = "Only call to this function")]
    tracing_subscriber::fmt::init();

    // An infinite stream of 'SIGTERM' signals.
    let mut sigterm_stream = match signal(SignalKind::terminate()) {
        Ok(sigterm_stream) => sigterm_stream,
        Err(err) => {
            error!("{err}");
            return ExitCode::FAILURE;
        }
    };

    // Create a channel and then our main actor, gitlab_tokens_actor()
    let (sender, receiver) = mpsc::channel(8);
    let gitlab_tokens_actor_handle = tokio::spawn(gitlab_tokens_actor(receiver, sender.clone()));

    // // Create the timer actor
    let timer_actor_handle = spawn_timer_actor(sender.clone());

    let app = Router::new()
        .route("/", get(root_handler))
        .route("/metrics", get(get_gitlab_tokens_handler))
        .with_state(sender);

    let listener = match TcpListener::bind("0.0.0.0:3000").await {
        Ok(listener) => listener,
        Err(err) => {
            error!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let local_addr = match listener.local_addr() {
        Ok(local_addr) => local_addr,
        Err(err) => {
            error!("{err}");
            return ExitCode::FAILURE;
        }
    };

    info!("listening on {local_addr}");

    // Waiting for one of the following :
    // - a SIGTERM signal
    // - the actor to finish/panic
    // - the axum server to finish
    select! {
        _ = sigterm_stream.recv() => {
            error!("Received a SIGTERM signal! exiting.");
            return ExitCode::FAILURE;
        },
        _ = gitlab_tokens_actor_handle => {
            error!("The state actor died! exiting.");
            return ExitCode::FAILURE;
        },
        _ = timer_actor_handle => {
            error!("The timer actor died! exiting.");
            return ExitCode::FAILURE;
        },
        _ = axum::serve(listener, app).into_future() => {
            error!("The server died! exiting.");
            return ExitCode::FAILURE;
        }
    }
}
