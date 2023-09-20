use openai::{chat::{ChatCompletionMessage, ChatCompletionMessageRole, ChatCompletionDelta, ChatCompletion}, set_key};
use dotenv::dotenv;
use anyhow::Result;

use std::{
    io::{stdout, Write}, sync::Arc,
};

use tokio::sync::{mpsc::Receiver, Mutex};

pub struct ChatService {
}

impl ChatService {
    pub fn new() -> Result<Self> {
        dotenv().ok();
        set_key(std::env::var("OPENAI_KEY")?);

        Ok(ChatService {})
    }

    pub fn get_base_messages(system_message: &str) -> Vec<ChatCompletionMessage> {
        let messages = vec![
            ChatCompletionMessage {
                role: ChatCompletionMessageRole::System,
                content: Some(system_message.to_string()),
                name: None,
                function_call: None,
            },
        ];

        messages
    }

    pub fn gen_sys_message(system_message: &str) -> ChatCompletionMessage {
        let message = ChatCompletionMessage {
                role: ChatCompletionMessageRole::System,
                content: Some(system_message.to_string()),
                name: None,
                function_call: None,
        };

        message
    }

    pub fn gen_user_message(user_message: &str) -> ChatCompletionMessage {
        let message = ChatCompletionMessage {
                role: ChatCompletionMessageRole::User,
                content: Some(user_message.to_string()),
                name: None,
                function_call: None,
        };

        message
    }

    pub async fn chat(
        &self, 
        model: &str,
        user_message: &str, 
        messages: Arc<Mutex<Vec<ChatCompletionMessage>>>,
        is_complete: Arc<Mutex<bool>>,
        word_buffer: Arc<Mutex<Vec<String>>>
    ) -> Result<()> {
        let user_message = ChatService::gen_user_message(user_message);

        let mut messages_compiled = messages.lock().await;
        messages_compiled.push(user_message);

        let chat_stream = ChatCompletionDelta::builder(model, messages_compiled.clone()).create_stream().await?;
        let chat_completion: ChatCompletion = self.listen_for_tokens(chat_stream, word_buffer).await?;
        let returned_message = chat_completion.choices.first().unwrap().message.clone();

        messages_compiled.push(returned_message);

        let mut is_complete = is_complete.lock().await;
        *is_complete = true;

        Ok(())
    }

    async fn listen_for_tokens(&self, mut chat_stream: Receiver<ChatCompletionDelta>, word_buffer: Arc<Mutex<Vec<String>>>) -> Result<ChatCompletion> {
        let mut merged: Option<ChatCompletionDelta> = None;
        while let Some(delta) = chat_stream.recv().await {
            let choice = &delta.choices[0];
            if let Some(content) = &choice.delta.content {
                let mut buff = word_buffer.lock().await;
                buff.push(content.clone());
            }

            // Merge completion into accrued.
            match merged.as_mut() {
                Some(c) => {
                    c.merge(delta).unwrap();
                }
                None => merged = Some(delta),
            };
        }

        Ok(merged.unwrap().into())
    }
}
