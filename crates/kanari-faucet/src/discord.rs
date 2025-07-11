// Copyright (c) Kanari Network
// SPDX-License-Identifier: Apache-2.0

use crate::{App, FaucetRequest};
use clap::Parser;
use kanari_rpc_api::jsonrpc_types::UnitedAddressView;
use move_core_types::u256::U256;
use serenity::all::{CommandDataOption, CommandDataOptionValue, CommandOptionType};
use serenity::async_trait;
use serenity::builder::{
    CreateCommand, CreateCommandOption, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage,
};
use serenity::model::{
    application::{Command, Interaction},
    gateway::Ready,
    id::{ChannelId, GuildId},
};
use serenity::prelude::*;
use std::{
    str::FromStr,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

#[derive(Parser, Debug, Clone)]
#[clap(rename_all = "kebab-case")]
pub struct DiscordConfig {
    #[arg(long, env = "KANARI_FAUCET_DISCORD_TOKEN")]
    pub discord_token: Option<String>,

    #[arg(long, env = "KANARI_FAUCET_NOTIFY_CHANNEL_ID")]
    pub notify_channel_id: Option<u64>,

    #[arg(long, env = "KANARI_FAUCET_CHECK_INTERVAL", default_value = "3600")]
    pub check_interval: u64,

    #[arg(
        long,
        env = "KANARI_FAUCET_NOTIFY_THRESHOLD",
        default_value = "10000000000"
    )]
    pub notify_threshold: U256,
}

impl App {
    async fn handle_faucet_request(&self, options: &[CommandDataOption]) -> String {
        let value = options
            .first()
            .expect("Expected address option")
            .value
            .clone();

        match value {
            CommandDataOptionValue::String(origin_address) => {
                match UnitedAddressView::from_str(&origin_address) {
                    Ok(parsed_address) => {
                        let request = FaucetRequest {
                            claimer: parsed_address.clone(),
                        };
                        match self.request(request).await {
                            Ok(amount) => {
                                let funds = amount.unchecked_as_u64() as f64 / 100000000f64;
                                format!("Sending {} RGas to {origin_address:?}", funds)
                            }
                            Err(err) => {
                                tracing::error!(
                                    "Failed make faucet request for {parsed_address:?}: {}",
                                    err
                                );
                                format!("Failed to send funds to {origin_address:?}, {}", err)
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse address: {}", e);
                        format!("Invalid address: {origin_address:?}")
                    }
                }
            }
            _ => "No address found!".to_string(),
        }
    }
}

#[async_trait]
impl EventHandler for App {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            tracing::info!("Received command interaction: {:#?}", command);

            let content = match command.data.name.as_str() {
                "faucet" => self.handle_faucet_request(&command.data.options).await,
                _ => "not implemented".to_string(),
            };

            let data = CreateInteractionResponseMessage::new().content(content);
            let builder = CreateInteractionResponse::Message(data);
            if let Err(why) = command.create_response(&ctx.http, builder).await {
                tracing::error!("Cannot respond to slash command: {:#?}", why);
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        tracing::info!("{} is connected!", ready.user.name);

        let command = CreateCommand::new("faucet")
            .description("Request funds from the faucet")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "address",
                    "Your BTC/Kanari address",
                )
                .required(true),
            );

        let guild_command = Command::create_global_command(&ctx.http, command).await;
        tracing::info!("I created the following global slash command: {guild_command:#?}");
    }

    async fn cache_ready(&self, ctx: Context, _guilds: Vec<GuildId>) {
        tracing::info!("Cache built successfully!");

        let discord_cfg = self.discord_config.clone();

        match discord_cfg.notify_channel_id {
            Some(notify_channel_id) => {
                let ctx = Arc::new(ctx);

                // We need to check that the loop is not already running when this event triggers, as this
                // event triggers every time the bot enters or leaves a guild, along every time the ready
                // shard event triggers.
                //
                // An AtomicBool is used because it doesn't require a mutable reference to be changed, as
                // we don't have one due to self being an immutable reference.
                if !self.is_loop_running.load(Ordering::Relaxed) {
                    // We have to clone the Arc, as it gets moved into the new thread.
                    let ctx1 = Arc::clone(&ctx);
                    let app = Arc::new(self.clone());
                    // tokio::spawn creates a new green thread that can run in parallel with the rest of
                    // the application.
                    tokio::spawn(async move {
                        loop {
                            let result = app.check_gas_balance().await;

                            match result {
                                Ok(v) => {
                                    if v < discord_cfg.notify_threshold {
                                        let embed = CreateEmbed::new()
                                            .title("Insufficient gas balance")
                                            .field("current balance", v.to_string(), true);
                                        let builder = CreateMessage::new().embed(embed);
                                        let message = ChannelId::new(notify_channel_id)
                                            .send_message(&ctx1, builder)
                                            .await;
                                        if let Err(why) = message {
                                            tracing::error!("Error sending message: {why:?}");
                                        };
                                    }
                                }
                                Err(e) => {
                                    let embed = CreateEmbed::new()
                                        .title("Check gas balance failed")
                                        .field("error", e.to_string(), false);
                                    let builder = CreateMessage::new().embed(embed);
                                    let message = ChannelId::new(notify_channel_id)
                                        .send_message(&ctx1, builder)
                                        .await;
                                    if let Err(why) = message {
                                        tracing::error!("Error sending message: {why:?}");
                                    };
                                }
                            }

                            tokio::time::sleep(Duration::from_secs(discord_cfg.check_interval))
                                .await;
                        }
                    });

                    let ctx2 = Arc::clone(&ctx);
                    let app2 = Arc::new(self.clone());
                    tokio::spawn(async move {
                        while let Some(err) = app2.err_receiver.write().await.recv().await {
                            let embed = CreateEmbed::new().title("Sending gas funds failed").field(
                                "error",
                                err.to_string(),
                                false,
                            );
                            let builder = CreateMessage::new().embed(embed);
                            let message = ChannelId::new(notify_channel_id)
                                .send_message(&ctx2, builder)
                                .await;
                            if let Err(why) = message {
                                tracing::error!("Error sending message: {why:?}");
                            };
                        }
                    });

                    // Now that the loop is running, we set the bool to true
                    self.is_loop_running.swap(true, Ordering::Relaxed);
                }
            }
            None => tracing::info!("Notify channel id is zero, not check gas balance!"),
        };
    }
}
