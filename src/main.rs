mod get_vromfs;
mod file;

use std::sync::Arc;
use axum::{
    routing::{get, post},
    http::StatusCode,
    Json, Router,
};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use crate::get_vromfs::{get_latest, print_latest_version, update_cache_loop, VromfCache};

#[derive(Default)]
pub struct AppState {
    vromf_cache: RwLock<VromfCache>,
    octocrab: Mutex<Octocrab>,
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();
    color_eyre::install().unwrap();

    let state= Arc::new(AppState::default());

    // build our application with a route
    let app = Router::new()
        .route("/latest", get(get_latest))
        // `GET /` goes to `root`
        .route("/", get(root))
        // `POST /users` goes to `create_user`
        .route("/users", post(create_user))
        .route("/metadata/latest", get(print_latest_version))
        .with_state(state.clone());

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    update_cache_loop(state);

    axum::serve(listener, app).await.unwrap();
}

// basic handler that responds with a static string
async fn root() -> &'static str {
    "Hello, World!"
}

async fn create_user(
    // this argument tells axum to parse the request body
    // as JSON into a `CreateUser` type
    Json(payload): Json<CreateUser>,
) -> (StatusCode, Json<User>) {
    // insert your application logic here
    let user = User {
        id: 1337,
        username: payload.username,
    };

    // this will be converted into a JSON response
    // with a status code of `201 Created`
    (StatusCode::CREATED, Json(user))
}

// the input to our `create_user` handler
#[derive(Deserialize)]
struct CreateUser {
    username: String,
}

// the output to our `create_user` handler
#[derive(Serialize)]
struct User {
    id: u64,
    username: String,
}