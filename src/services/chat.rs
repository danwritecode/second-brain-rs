use openai::{chat::{self, ChatCompletionMessage, ChatCompletionMessageRole, ChatCompletionDelta, ChatCompletion}, set_key};
use dotenv::dotenv;
use anyhow::{Result, anyhow};

use std::{
    env,
    io::{stdin, stdout, Write},
};

use tokio::sync::mpsc::Receiver;

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

    pub async fn chat(&self, user_message: &str, messages: &mut Vec<ChatCompletionMessage>) -> Result<()> {
        let user_message = ChatCompletionMessage {
            role: ChatCompletionMessageRole::User,
            content: Some(user_message.to_string()),
            name: None,
            function_call: None,
        };

        messages.push(user_message);

        println!("messages before: {:#?}", messages);

        let chat_stream = ChatCompletionDelta::builder("gpt-3.5-turbo-16k", messages.clone()).create_stream().await?;

        let chat_completion: ChatCompletion = self.listen_for_tokens(chat_stream).await?;
        let returned_message = chat_completion.choices.first().unwrap().message.clone();

        messages.push(returned_message);

        println!("messages after: {:#?}", messages);
        
        Ok(())
    }

    async fn listen_for_tokens(&self, mut chat_stream: Receiver<ChatCompletionDelta>) -> Result<ChatCompletion> {
        let mut merged: Option<ChatCompletionDelta> = None;
        while let Some(delta) = chat_stream.recv().await {
            let choice = &delta.choices[0];
            if let Some(role) = &choice.delta.role {
                print!("{:#?}: ", role);
            }
            if let Some(content) = &choice.delta.content {
                print!("{}", content);
            }
            if let Some(_) = &choice.finish_reason {
                // The message being streamed has been fully received.
                print!("\n");
            }
            stdout().flush()?;

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
