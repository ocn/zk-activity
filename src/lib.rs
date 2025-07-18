use rand::{distributions::Alphanumeric, Rng};
use serenity::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn, Level};

pub mod commands;
pub mod config;
pub mod discord_bot;
pub mod esi;
pub mod models;
pub mod processor;
pub mod redis_q;

use crate::commands::find_unsubscribed::FindUnsubscribedChannelsCommand;
use commands::diag::DiagCommand;
use commands::subscribe::SubscribeCommand;
use commands::sync_clear::SyncClearCommand;
use commands::sync_remove::SyncRemoveCommand;
use commands::sync_standings::SyncStandingsCommand;
use commands::unsubscribe::UnsubscribeCommand;
use commands::{Command, PingCommand};
use discord_bot::CommandMap;

pub struct AppStateContainer;

impl TypeMapKey for AppStateContainer {
    type Value = Arc<config::AppState>;
}

fn generate_queue_id() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}

// fn log_loaded_subscriptions(subscriptions: &HashMap<serenity::model::id::GuildId, Vec<Subscription>>) {
//     info!("--- Loaded Subscriptions ---");
//     if subscriptions.is_empty() {
//         info!("No subscriptions found.");
//     } else {
//         for (guild_id, subs) in subscriptions {
//             info!("Guild: {}", guild_id);
//             let mut subs_by_channel: HashMap<u64, Vec<&Subscription>> = HashMap::new();
//             for sub in subs {
//                 subs_by_channel.entry(sub.action.channel_id).or_default().push(sub);
//             }
//             for (channel_id, channel_subs) in subs_by_channel {
//                 info!("  Channel: {}", channel_id);
//                 for sub in channel_subs {
//                     info!("    - ID: '{}', Description: '{}'", sub.id, sub.description);
//                 }
//             }
//         }
//     }
//     info!("--------------------------");
// }

pub async fn run() {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting Killbot-Rust...");

    // --- Load all configurations ---
    let app_config = match config::load_app_config() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load application configuration: {}", e);
            return;
        }
    };

    let systems = config::load_systems().unwrap_or_else(|e| {
        warn!(
            "Failed to load systems.json: {}. Starting with an empty map.",
            e
        );
        HashMap::new()
    });
    let ships = config::load_ships().unwrap_or_else(|e| {
        warn!(
            "Failed to load ships.json: {}. Starting with an empty map.",
            e
        );
        HashMap::new()
    });
    let names = config::load_names().unwrap_or_else(|e| {
        warn!(
            "Failed to load names.json: {}. Starting with an empty map.",
            e
        );
        HashMap::new()
    });

    let user_standings = config::load_user_standings().unwrap_or_else(|e| {
        warn!(
            "Failed to load user_standings.json: {}. Starting with an empty map.",
            e
        );
        HashMap::new()
    });

    let subscriptions = config::load_all_subscriptions("config/");
    // log_loaded_subscriptions(&subscriptions);

    // --- Initialize application state ---
    let app_state = Arc::new(config::AppState::new(
        app_config.clone(),
        systems,
        ships,
        names,
        subscriptions,
        user_standings,
    ));

    // --- Initialize Commands ---
    let mut command_map: HashMap<String, Box<dyn Command>> = HashMap::new();

    let ping_command = Box::new(PingCommand);
    command_map.insert(ping_command.name(), ping_command);

    let subscribe_command = Box::new(SubscribeCommand);
    command_map.insert(subscribe_command.name(), subscribe_command);

    let unsubscribe_command = Box::new(UnsubscribeCommand);
    command_map.insert(unsubscribe_command.name(), unsubscribe_command);

    let diag_command = Box::new(DiagCommand);
    command_map.insert(diag_command.name(), diag_command);

    let sync_standings_command = Box::new(SyncStandingsCommand);
    command_map.insert(sync_standings_command.name(), sync_standings_command);

    let sync_remove_command = Box::new(SyncRemoveCommand);
    command_map.insert(sync_remove_command.name(), sync_remove_command);

    let sync_clear_command = Box::new(SyncClearCommand);
    command_map.insert(sync_clear_command.name(), sync_clear_command);

    let find_unsubscribed_command = Box::new(FindUnsubscribedChannelsCommand);
    command_map.insert(find_unsubscribed_command.name(), find_unsubscribed_command);

    let command_map_arc = Arc::new(command_map);

    // --- Start Discord Bot ---
    let discord_token = app_config.discord_bot_token.clone();
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_INTEGRATIONS;
    let mut client = Client::builder(&discord_token, intents)
        .event_handler(discord_bot::Handler)
        .await
        .expect("Error creating Discord client");

    {
        let mut data = client.data.write().await;
        data.insert::<AppStateContainer>(app_state.clone());
        data.insert::<CommandMap>(command_map_arc.clone());
    }

    let http_client = client.cache_and_http.http.clone();

    tokio::spawn(async move {
        if let Err(why) = client.start().await {
            error!("Discord client error: {:?}", why);
        }
    });

    // --- Main killmail processing loop ---
    let queue_id = generate_queue_id();
    let listener = redis_q::RedisQListener::new(&queue_id);
    info!("Listening for killmails from RedisQ...");

    loop {
        match listener.listen().await {
            Ok(Some(zk_data)) => {
                let kill_id = zk_data.killmail.killmail_id;
                info!("[Kill: {}] Received", kill_id);
                let matched = processor::process_killmail(&app_state, &zk_data).await;

                if !matched.is_empty() {
                    for (guild_id, subscription, filter_result) in matched {
                        info!(
                            "[Kill: {}] Matched subscription '{}' for channel {}, this filter was a match: {}",
                            kill_id, subscription.description, subscription.action.channel_id, filter_result.name
                        );
                        if let Err(e) = discord_bot::send_killmail_message(
                            &http_client,
                            &app_state,
                            &subscription,
                            &zk_data,
                            filter_result,
                        )
                        .await
                        {
                            match e {
                                discord_bot::KillmailSendError::CleanupChannel(e) => {
                                    warn!(
                                        "Cleaning up subscriptions for channel {} due to error: {:#?}",
                                        subscription.action.channel_id, e
                                    );
                                    let _lock = app_state.subscriptions_file_lock.lock().await;
                                    let mut subs_map = app_state.subscriptions.write().unwrap();

                                    if let Some(guild_subs) = subs_map.get_mut(&guild_id) {
                                        guild_subs.retain(|s| {
                                            s.action.channel_id != subscription.action.channel_id
                                        });
                                        if let Err(save_err) = config::save_subscriptions_for_guild(
                                            guild_id, guild_subs,
                                        ) {
                                            error!("Failed to save subscriptions after cleanup for guild {}: {}", guild_id, save_err);
                                        }
                                    }
                                }
                                discord_bot::KillmailSendError::Other(err) => {
                                    error!(
                                        "Error sending message for subscription {}: {}",
                                        subscription.id, err
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                // No new data, continue loop
            }
            Err(e) => {
                error!("Error listening for killmails: {}", e);
                // Wait a bit before retrying to avoid spamming logs on persistent errors
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}
