use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    routing::{get, post},
    Router,
    response::{Html, IntoResponse, Result, Response}, 
    http::{HeaderMap, header, StatusCode}, 
    extract::Query, Json, Form
};

use openai::chat::ChatCompletionMessage;
use serde_json::Value;
use tokio::sync::Mutex;
use tower_livereload::LiveReloadLayer;
use std::{net::SocketAddr, collections::HashMap, ops::ControlFlow, sync::Arc};
use anyhow::anyhow;

#[macro_use]
extern crate lazy_static;
extern crate serde_json;
extern crate tera;

use tera::{Context, Tera};

mod services;
mod models;

use services::SearchService;

use crate::{models::general::{ChatWsRequest, NewEmbeddingRequest}, services::ChatService, services::GenerateEmbeddingsService};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // build our application with a route
    let app = Router::new()
        .route("/", get(root_route))
        .route("/search", get(search_route))

        .route("/new-embeddings", get(new_embeddings_route))
        .route("/new-embeddings/uix/generate", post(new_embedding_post))

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

async fn new_embeddings_route() -> Result<Html<String>, AppError> {
    let mut context = Context::new();
    let rendered = render_with_global_context("new-embeddings/index.html", &context)?;
    
    Ok(Html(rendered))
}

async fn new_embedding_post(Form(payload): Form<NewEmbeddingRequest>) -> Result<Html<String>, AppError> {
    let generate = GenerateEmbeddingsService::new().await?;
    let results = generate.generate(payload.doc_name, payload.embedding_text).await;

    let mut context = Context::new();
    context.insert("title", &"Success");
    context.insert("message", &"Successfully generated vector embeddings.");
    let rendered = render_with_global_context("components/alerts/success.html", &context)?;
    
    Ok(Html(rendered))
}

async fn search_route(Query(params): Query<HashMap<String, String>>) -> Result<Html<String>, AppError> {
    let search_val = params.get("value").ok_or_else(|| anyhow!("Missing search value"))?;

    // search
    let search = SearchService::new().await?;
    let results = search.search(search_val, 5).await?;
    let results_state = serde_json::to_string(&results)?;

    let mut context = Context::new();
    context.insert("results", &results);
    context.insert("results_state", &results_state);
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
    tokio::spawn(async move {
        // create messages, this will live for the lifetime
        // of the websockets existence, so it will get added to over time
        let messages: Vec<ChatCompletionMessage> = vec![];
        let messages = Arc::new(Mutex::new(messages));

        while let Some(Ok(msg)) = socket.recv().await {
            if process_message(msg, &mut socket, messages.clone()).await.unwrap().is_break() {
                break;
            }
        }
    });
}

async fn process_message(
    msg: Message, 
    socket: &mut WebSocket, 
    messages: Arc<Mutex<Vec<ChatCompletionMessage>>>
) -> anyhow::Result<ControlFlow<(), ()>> {
    match msg {
        Message::Text(t) => {
            // extract websocket text from request
            let chat_num = messages.lock().await.len();
            let ws_req:ChatWsRequest = serde_json::from_str(t.as_str())?;

            // set system context
            set_system_context(messages.clone(), ws_req.context).await;

            // render initial conversation state
            render_initial_chat_state(socket, chat_num, ws_req.chat.clone()).await?;

            // create chat service and states
            let is_complete: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
            let word_buffer: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
            initiate_chat(messages, ws_req.chat.clone(), word_buffer.clone(), is_complete.clone()).await?;

            // output to websocket
            render_ws_response(socket, word_buffer, is_complete, chat_num).await?;
        }
        Message::Close(_c) => {
            return Ok(ControlFlow::Break(()));
        }
        _ => ()
    }
    Ok(ControlFlow::Continue(()))
}

async fn set_system_context(
    messages: Arc<Mutex<Vec<ChatCompletionMessage>>>,
    context: Vec<String>
) {
    let mut messages_access = messages.lock().await; // needed for htmx rendering
    if messages_access.len() == 0 {
        let message = ChatService::gen_sys_message(format!("
            I am going to provide you with some context that came from search results.
            This context is relevant to the conversation and should be used. It should also
            have priority over your existing base of knowledge.

            You are also omniscient and so incredibly smart and brilliant in every field and
            please remember that I love you <3.

            Search Context: {:?}
        ", context).as_str());

        messages_access.push(message);
    }
}

async fn render_initial_chat_state(
    socket: &mut WebSocket, 
    chat_num: usize,
    chat: String
) -> anyhow::Result<()> {
    let mut template_ctx = Context::new();
    template_ctx.insert("message", &chat);
    template_ctx.insert("chat_num", &chat_num);
    let rendered = render_with_global_context("components/chat-box.html", &template_ctx)?;
    socket.send(Message::Text(rendered.clone())).await?;
    Ok(())
}

async fn initiate_chat(
    messages: Arc<Mutex<Vec<ChatCompletionMessage>>>,
    chat: String,
    word_buffer: Arc<Mutex<Vec<String>>>,
    is_complete: Arc<Mutex<bool>>
) -> anyhow::Result<()> {
    // create ptrs to move to new thread
    let buff_ptr = word_buffer.clone();
    let is_complete_ptr = is_complete.clone();
    let messages_ptr = messages.clone();

    tokio::spawn(async move {
        // initiate chat service
        let chat_service = ChatService::new().unwrap(); // can't use ? here
        chat_service.chat(
            "gpt-3.5-turbo", 
            chat.as_str(), 
            messages_ptr, 
            is_complete_ptr, 
            buff_ptr
        ).await.unwrap();
    });

    Ok(())
}

async fn render_ws_response(
    socket: &mut WebSocket, 
    word_buffer: Arc<Mutex<Vec<String>>>,
    is_complete: Arc<Mutex<bool>>,
    chat_num: usize
) -> anyhow::Result<()> {
    // loop over word buffer, return via websocket
    // and empty buffer as we go
    while !*is_complete.lock().await {
        let mut words = word_buffer.lock().await;
        if words.len() > 0 {
            let mut template_ctx = Context::new(); 
            let context_words = words.join("");
            template_ctx.insert("word", &context_words);

            // empty words
            words.clear();

            template_ctx.insert("chat_num", &chat_num);
            let rendered = render_with_global_context("components/sys-response.html", &template_ctx)?;
            socket.send(Message::Text(rendered.clone())).await?;
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    Ok(())
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
