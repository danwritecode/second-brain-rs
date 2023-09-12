use axum::{
    TypedHeader,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    routing::{get, post},
    Router,
    response::{Html, IntoResponse, Result, Response}, 
    http::{HeaderMap, header, StatusCode}, 
    extract::{Query, ConnectInfo, ws::CloseFrame}
};

use openai::chat::ChatCompletionMessage;
use tower_livereload::LiveReloadLayer;
use std::{net::SocketAddr, collections::HashMap, borrow::Cow, ops::ControlFlow};
use anyhow::anyhow;
use serde::Deserialize;

#[macro_use]
extern crate lazy_static;
extern crate serde_json;
extern crate tera;

use tera::{Context, Tera};

mod services;
mod models;

use services::SearchService;

use crate::{models::general::ChatWsRequest, services::ChatService};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // build our application with a route
    let app = Router::new()
        .route("/", get(root_route))
        .route("/search", get(search_route))
        .route("/ws", get(ws_handler))
        .route("/css", get(css))
        .route("/js", get(js))
        .route("/route-name/uix/clicked", post(clicked_uix))
        .layer(LiveReloadLayer::new());

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn css() -> Result<impl IntoResponse, AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/css".parse()?);
    let css = std::fs::read_to_string("dist/output.css")?;
    
    Ok((headers, css))
}

async fn js() -> Result<impl IntoResponse, AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/javascript".parse()?);
    headers.insert(header::CACHE_CONTROL, "public, max-age=3600".parse()?);
    let js = std::fs::read_to_string("dist/output.js")?;
    
    Ok((headers, js))
}

async fn root_route() -> Result<Html<String>, AppError> {
    let context = Context::new();
    let rendered = render_with_global_context("root/index.html", &context)?;
    
    Ok(Html(rendered))
}

async fn search_route(Query(params): Query<HashMap<String, String>>) -> Result<Html<String>, AppError> {
    let search_val = params.get("value").ok_or_else(|| anyhow!("Missing search value"))?;

    // search
    let search = SearchService::new().await?;
    let results = search.search(search_val, 5).await?;

    let mut context = Context::new();
    context.insert("results", &results);
    let rendered = render_with_global_context("search-route/index.html", &context)?;
    
    Ok(Html(rendered))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(mut socket: WebSocket) {
    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        println!("Pinged...");
    } else {
        println!("Could not send ping!");
        return;
    }

    
    tokio::spawn(async move {
        let mut messages = ChatService::get_base_messages(
            "you are omniscient and really kind and friendly, you possess infinite wisdom and patience"
        );

        while let Some(Ok(msg)) = socket.recv().await {
            if process_message(msg, &mut socket, &mut messages).await.is_break() {
                break;
            }
        }
    });

    // returning from the handler closes the websocket connection
    println!("Websocket context destroyed");
}

/// helper to print contents of messages to stdout. Has special treatment for Close.
async fn process_message(msg: Message, socket: &mut WebSocket, messages: &mut Vec<ChatCompletionMessage>) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            let ws_req:ChatWsRequest = serde_json::from_str(&t[..]).unwrap();

            let mut sys_context = Context::new();
            sys_context.insert("user", &true);
            sys_context.insert("message", &ws_req.chat);
            sys_context.insert("word", &"");

            // take chat and send to gpt-4
            let rendered = render_with_global_context("components/chat-box-empty.html", &sys_context).unwrap();
            socket.send(Message::Text(rendered.clone())).await.unwrap();
            
            let chat_service = ChatService::new().unwrap();
            chat_service.chat(&ws_req.chat[..], messages).await.unwrap();

            let mut usr_context = Context::new();

            let message = messages.last().unwrap().content.clone().unwrap();

            let words = message.split(" ").map(|n| n.to_string()).collect::<Vec<String>>();

            for w in words {
                let mut loop_context = Context::new();
                loop_context.insert("user", &false);
                loop_context.insert("word", &w);
                let rendered = render_with_global_context("components/chat-box-stream.html", &loop_context).unwrap();
                socket.send(Message::Text(rendered.clone())).await.unwrap();
            }


            // usr_context.insert("user", &false);
            // usr_context.insert("message", &message);
            // let rendered = render_with_global_context("components/chat-box.html", &usr_context).unwrap();
            //
            // socket.send(Message::Text(rendered.clone())).await.unwrap();
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> received close with code {} and reason `{}`",
                    cf.code, cf.reason
                );
            } else {
                println!(">>> somehow sent close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }
        _ => ()
    }
    ControlFlow::Continue(())
}

async fn clicked_uix() -> Result<Html<String>, AppError> {
    Ok(Html("<p class=\"text-center mt-10\">Hello from htmx</p>".to_string()))
}

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let tera = Tera::new("ui/templates/**/*").unwrap();

        return tera;
    };
}

fn render_with_global_context(template: &str, specific_context: &Context) -> tera::Result<String> {
    let version = env!("CARGO_PKG_VERSION");
    let mut context = Context::new();
    context.insert("cargo_version", &version);

    context.extend(specific_context.clone());
    TEMPLATES.render(template, &context)
}

#[derive(Debug)]
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use axum::{extract::Query, http::Uri};

    use crate::{root_route, search_route, css, js, clicked_uix};

    #[tokio::test]
    async fn css_test() {
        let res = css().await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn js_test() {
        let res = js().await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn root_route_test() {
        let res = root_route().await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn search_route_test() {
        let uri: Uri = "http://example.com/search?value=hello".parse().unwrap();
        let result: Query<HashMap<String, String>> = Query::try_from_uri(&uri).unwrap();
        let res = search_route(result).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn clicked_uix_test() {
        let res = clicked_uix().await;
        assert!(res.is_ok());
    }
}
