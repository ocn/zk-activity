use crate::commands::sync_standings::SyncStandingsCommand;
use crate::commands::Command;
use crate::config::{
    save_names, save_ships, save_systems, save_user_standings, AppState, Filter, FilterNode,
    PingType, SimpleFilter, StandingSource, Subscription, System,
};
use crate::esi::Celestial;
use crate::models::{Attacker, ZkData};
use crate::processor::{AttackerKey, Color, NamedFilterResult};
use chrono::{DateTime, FixedOffset, Utc};
use futures::future::join_all;
use serenity::async_trait;
use serenity::builder::CreateEmbed;
use serenity::http::error::Error;
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::guild::UnavailableGuild;
use serenity::model::prelude::{ChannelId, Interaction};
use serenity::prelude::*;
use serenity::utils::Colour;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tracing::{error, info, trace, warn};

const SHIP_GROUP_PRIORITY: &[u32] = &[
    30,   // Titan
    659,  // Supercarrier
    4594, // Lancer
    485,  // Dreadnought
    1538, // FAX
    547,  // Carrier
    883,  // Capital Industrial Ship
    902,  // Jump Freighter
    513,  // Freighter
];

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MatchedEntity {
    pub ship_name: String,
    pub type_id: u32,
    pub group_id: u32,
    pub corp_id: Option<u64>,
    pub alliance_id: Option<u64>,
    pub color: Color,
}

#[derive(Debug)]
pub enum KillmailSendError {
    CleanupChannel(serenity::Error),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for KillmailSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KillmailSendError::CleanupChannel(e) => write!(f, "Channel cleanup required: {}", e),
            KillmailSendError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for KillmailSendError {}

pub struct CommandMap;
impl TypeMapKey for CommandMap {
    type Value = Arc<HashMap<String, Box<dyn Command>>>;
}

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn guild_delete(&self, _ctx: Context, incomplete: UnavailableGuild) {
        info!("Kicked from guild: {}", incomplete.id);
        let mut subs = _ctx.data.write().await;
        let app_state = subs.get_mut::<crate::AppStateContainer>().unwrap();
        let _lock = app_state.subscriptions_file_lock.lock().await;
        let mut subscriptions = app_state.subscriptions.write().unwrap();
        subscriptions.remove(&incomplete.id);
        if let Err(e) = crate::config::save_subscriptions_for_guild(incomplete.id, &[]) {
            error!(
                "Failed to delete subscription file for guild {}: {}",
                incomplete.id, e
            );
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore messages from bots and messages that are not in DMs
        if msg.author.bot || msg.guild_id.is_some() {
            return;
        }

        // Check if the message content looks like an SSO callback URL
        if msg.content.starts_with("https://github.headempty.space/") && msg.content.len() < 2000 {
            let url = match url::Url::parse(&msg.content) {
                Ok(url) => url,
                Err(_) => {
                    // Inform the user that the URL is invalid
                    if let Err(why) = msg.channel_id.say(&ctx.http, "Invalid URL format.").await {
                        error!("Error sending message: {:?}", why);
                    }
                    return;
                }
            };

            let query_params: HashMap<String, String> = url.query_pairs().into_owned().collect();
            let code = query_params.get("code");
            let state = query_params.get("state");

            if let (Some(code), Some(state)) = (code, state) {
                let data = ctx.data.read().await;
                let app_state = data.get::<crate::AppStateContainer>().unwrap();

                let sso_state = {
                    let mut sso_states = app_state.sso_states.lock().await;
                    sso_states.remove(state)
                };

                if let Some(sso_state) = sso_state {
                    let client_id = app_state.app_config.eve_client_id.clone();
                    let client_secret = app_state.app_config.eve_client_secret.clone();

                    // 1. Exchange code for token
                    match app_state
                        .esi_client
                        .exchange_code_for_token(code, &client_id, &client_secret)
                        .await
                    {
                        Ok(token) => {
                            let mut full_token = token.clone();
                            let _ = msg
                                .channel_id
                                .say(
                                    &ctx.http,
                                    format!(
                                        "Successfully authenticated as {}. Fetching contacts...",
                                        token.character_name
                                    ),
                                )
                                .await;

                            // 2. Fetch affiliations and contacts
                            let (corp_id, alliance_id) = match app_state
                                .esi_client
                                .get_character_affiliation(token.character_id)
                                .await
                            {
                                Ok(affiliation) => affiliation,
                                Err(e) => {
                                    error!("Failed to get character affiliation: {}", e);
                                    let _ = msg
                                        .channel_id
                                        .say(
                                            &ctx.http,
                                            "Failed to fetch character affiliation. Aborting.",
                                        )
                                        .await;
                                    return;
                                }
                            };

                            let source_entity_id = match sso_state.standing_source {
                                StandingSource::Character => token.character_id,
                                StandingSource::Corporation => corp_id,
                                StandingSource::Alliance => alliance_id.unwrap_or(corp_id), // Fallback to corp if no alliance
                            };

                            let contacts = app_state
                                .esi_client
                                .get_contacts(
                                    source_entity_id,
                                    &token.access_token,
                                    match sso_state.standing_source {
                                        StandingSource::Character => "characters",
                                        StandingSource::Corporation => "corporations",
                                        StandingSource::Alliance => "alliances",
                                    },
                                )
                                .await
                                .unwrap_or_default();

                            // 3. Save token and contacts
                            {
                                let _lock = app_state.user_standings_file_lock.lock().await;
                                let mut standings_map = app_state.user_standings.write().unwrap();
                                let user_standings =
                                    standings_map.entry(sso_state.discord_user_id).or_default();

                                // Add the affiliation to the token before saving
                                full_token.corporation_id = corp_id;
                                full_token.alliance_id = alliance_id;

                                user_standings
                                    .tokens
                                    .retain(|t| t.character_id != full_token.character_id);
                                user_standings.tokens.push(full_token);

                                user_standings
                                    .contact_lists
                                    .contacts
                                    .insert(source_entity_id, contacts);
                                save_user_standings(&standings_map);
                            }

                            // 4. Update subscription
                            let guild_id = sso_state.original_interaction.guild_id.unwrap();
                            let mut subscription_updated = false;
                            {
                                let _lock = app_state.subscriptions_file_lock.lock().await;
                                let mut subs_map = app_state.subscriptions.write().unwrap();
                                if let Some(guild_subs) = subs_map.get_mut(&guild_id) {
                                    let original_channel_id =
                                        sso_state.original_interaction.channel_id.to_string();
                                    if let Some(sub) = guild_subs.iter_mut().find(|s| {
                                        s.id == sso_state.subscription_id
                                            && s.action.channel_id == original_channel_id
                                    }) {
                                        let new_filter = FilterNode::Condition(Filter::Simple(
                                            SimpleFilter::IgnoreHighStanding {
                                                synched_by_user_id: sso_state.discord_user_id.0,
                                                source: sso_state.standing_source,
                                                source_entity_id,
                                            },
                                        ));

                                        if let FilterNode::And(ref mut conditions) = sub.root_filter
                                        {
                                            // Remove any existing high standing filters before adding the new one.
                                            conditions.retain(|c| {
                                                !matches!(
                                                    c,
                                                    FilterNode::Condition(Filter::Simple(
                                                        SimpleFilter::IgnoreHighStanding { .. }
                                                    ))
                                                )
                                            });
                                            conditions.push(new_filter);
                                        } else {
                                            // If it's not an AND node, it might be a single condition.
                                            // We'll wrap the old and new filters in an AND node.
                                            let old_root = sub.root_filter.clone();
                                            // But first, check if the old root is the one we want to replace.
                                            if matches!(
                                                &old_root,
                                                FilterNode::Condition(Filter::Simple(
                                                    SimpleFilter::IgnoreHighStanding { .. }
                                                ))
                                            ) {
                                                sub.root_filter = new_filter;
                                            } else {
                                                sub.root_filter =
                                                    FilterNode::And(vec![old_root, new_filter]);
                                            }
                                        }
                                        if let Err(e) = crate::config::save_subscriptions_for_guild(
                                            guild_id, guild_subs,
                                        ) {
                                            error!(
                                                "Failed to save subscriptions for guild {}: {}",
                                                guild_id, e
                                            );
                                        } else {
                                            subscription_updated = true;
                                        }
                                    }
                                }
                            }

                            if subscription_updated {
                                let _ = msg.channel_id.say(&ctx.http, format!("Subscription '{}' has been successfully synced with {}'s standings.", sso_state.subscription_id, token.character_name)).await;
                            } else {
                                let _ = msg
                                    .channel_id
                                    .say(
                                        &ctx.http,
                                        "No matching subscription found. Please try again.",
                                    )
                                    .await;
                            }
                        }
                        Err(e) => {
                            error!("Failed to exchange token: {}", e);
                            let _ = msg
                                .channel_id
                                .say(
                                    &ctx.http,
                                    "Failed to authenticate with EVE SSO. Please try again.",
                                )
                                .await;
                        }
                    }
                } else if let Err(why) = msg
                    .channel_id
                    .say(
                        &ctx.http,
                        "Invalid or expired state. Please try the command again.",
                    )
                    .await
                {
                    error!("Error sending message: {:?}", why);
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, data_about_bot: Ready) {
        info!("Discord bot {} is connected!", data_about_bot.user.name);

        let data = ctx.data.read().await;
        let command_map = data.get::<CommandMap>().unwrap();

        if let Err(e) =
            serenity::model::application::command::Command::set_global_application_commands(
                &ctx.http,
                |commands| {
                    for cmd in command_map.values() {
                        commands.create_application_command(|c| cmd.register(c));
                    }
                    commands
                },
            )
            .await
        {
            error!("Failed to register global commands: {}", e);
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::ApplicationCommand(command) => {
                let data = ctx.data.read().await;
                let command_map = data.get::<CommandMap>().unwrap();
                let app_state = data.get::<crate::AppStateContainer>().unwrap();

                if let Some(cmd) = command_map.get(&command.data.name) {
                    cmd.execute(&ctx, &command, app_state).await;
                }
            }
            Interaction::MessageComponent(component_interaction) => {
                let custom_id = &component_interaction.data.custom_id;
                let data = ctx.data.read().await;
                let app_state = data.get::<crate::AppStateContainer>().unwrap();

                if custom_id.starts_with("standings_select_") {
                    let state = custom_id.strip_prefix("standings_select_").unwrap();
                    let sso_state = app_state.sso_states.lock().await.remove(state);

                    if let (Some(sso_state), Some(values)) =
                        (sso_state, Some(&component_interaction.data.values))
                    {
                        if let Some(character_id_str) = values.first() {
                            let character_id = character_id_str.parse::<u64>().unwrap();

                            let token_clone = {
                                let standings_map = app_state.user_standings.read().unwrap();
                                if let Some(user_standings) =
                                    standings_map.get(&sso_state.discord_user_id)
                                {
                                    user_standings
                                        .tokens
                                        .iter()
                                        .find(|t| t.character_id == character_id)
                                        .cloned()
                                } else {
                                    None
                                }
                            };

                            if let Some(token) = token_clone {
                                if let Ok((corp_id, alliance_id)) = app_state
                                    .esi_client
                                    .get_character_affiliation(token.character_id)
                                    .await
                                {
                                    let source_entity_id = match sso_state.standing_source {
                                        StandingSource::Character => token.character_id,
                                        StandingSource::Corporation => corp_id,
                                        StandingSource::Alliance => alliance_id.unwrap_or(corp_id),
                                    };

                                    let guild_id = sso_state.original_interaction.guild_id.unwrap();
                                    let mut subscription_updated = false;
                                    {
                                        let _lock = app_state.subscriptions_file_lock.lock().await;
                                        let mut subs_map = app_state.subscriptions.write().unwrap();
                                        if let Some(guild_subs) = subs_map.get_mut(&guild_id) {
                                            let original_channel_id = sso_state
                                                .original_interaction
                                                .channel_id
                                                .to_string();
                                            if let Some(sub) = guild_subs.iter_mut().find(|s| {
                                                s.id == sso_state.subscription_id
                                                    && s.action.channel_id == original_channel_id
                                            }) {
                                                let new_filter =
                                                    FilterNode::Condition(Filter::Simple(
                                                        SimpleFilter::IgnoreHighStanding {
                                                            synched_by_user_id: sso_state
                                                                .discord_user_id
                                                                .0,
                                                            source: sso_state.standing_source,
                                                            source_entity_id,
                                                        },
                                                    ));
                                                if let FilterNode::And(ref mut conditions) =
                                                    sub.root_filter
                                                {
                                                    // Remove any existing high standing filters before adding the new one.
                                                    conditions.retain(|c| {
                                                        !matches!(c, FilterNode::Condition(Filter::Simple(SimpleFilter::IgnoreHighStanding { .. })))
                                                    });
                                                    conditions.push(new_filter);
                                                } else {
                                                    // If it's not an AND node, it might be a single condition.
                                                    // We'll wrap the old and new filters in an AND node.
                                                    let old_root = sub.root_filter.clone();
                                                    // But first, check if the old root is the one we want to replace.
                                                    if matches!(
                                                        &old_root,
                                                        FilterNode::Condition(Filter::Simple(
                                                            SimpleFilter::IgnoreHighStanding { .. }
                                                        ))
                                                    ) {
                                                        sub.root_filter = new_filter;
                                                    } else {
                                                        sub.root_filter = FilterNode::And(vec![
                                                            old_root, new_filter,
                                                        ]);
                                                    }
                                                }

                                                if let Err(e) =
                                                    crate::config::save_subscriptions_for_guild(
                                                        guild_id, guild_subs,
                                                    )
                                                {
                                                    error!("Failed to save subscriptions for guild {}: {}", guild_id, e);
                                                } else {
                                                    subscription_updated = true;
                                                }
                                            }
                                        }
                                    }

                                    if subscription_updated {
                                        if let Err(why) = component_interaction
                                            .create_interaction_response(&ctx.http, |r| {
                                                r.interaction_response_data(|m| {
                                                    m.content(format!(
                                                        "Subscription '{}' updated successfully.",
                                                        sso_state.subscription_id
                                                    ))
                                                    .ephemeral(true)
                                                })
                                            })
                                            .await
                                        {
                                            error!("Cannot respond to interaction: {}", why);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if custom_id.starts_with("standings_reauth_") {
                    let state = custom_id.strip_prefix("standings_reauth_").unwrap();
                    let sso_states = app_state.sso_states.lock().await;
                    if let Some(sso_state) = sso_states.get(state) {
                        let sync_command = SyncStandingsCommand;
                        sync_command
                            .initiate_sso(&ctx, &sso_state.original_interaction, app_state, state)
                            .await;
                        let _ = component_interaction
                            .create_interaction_response(&ctx.http, |r| {
                                r.interaction_response_data(|m| {
                                    m.content("A new authorization link has been sent to your DMs.")
                                        .ephemeral(true)
                                })
                            })
                            .await;
                    }
                }
            }
            _ => {}
        }
    }
}

// --- Dynamic Data Fetching and Caching ---

pub async fn get_system(app_state: &Arc<AppState>, system_id: u32) -> Option<System> {
    {
        let systems = app_state.systems.read().unwrap();
        if let Some(system) = systems.get(&system_id) {
            return Some(system.clone());
        }
    }
    match app_state.esi_client.get_system(system_id).await {
        Ok(system) => {
            let _lock = app_state.systems_file_lock.lock().await;
            let mut systems = app_state.systems.write().unwrap();
            systems.insert(system_id, system.clone());
            save_systems(&systems);
            Some(system)
        }
        Err(e) => {
            warn!("Failed to fetch system data for {}: {}", system_id, e);
            None
        }
    }
}

pub async fn get_ship_group_id(app_state: &Arc<AppState>, ship_id: u32) -> Option<u32> {
    {
        let ships = app_state.ships.read().unwrap();
        if let Some(group_id) = ships.get(&ship_id) {
            return Some(*group_id);
        }
    }
    match app_state.esi_client.get_ship_group_id(ship_id).await {
        Ok(group_id) => {
            let _lock = app_state.ships_file_lock.lock().await;
            let mut ships = app_state.ships.write().unwrap();
            ships.insert(ship_id, group_id);
            save_ships(&ships);
            Some(group_id)
        }
        Err(e) => {
            warn!("Failed to fetch ship group for {}: {}", ship_id, e);
            None
        }
    }
}

pub async fn get_name(app_state: &Arc<AppState>, id: u64) -> Option<String> {
    {
        let names = app_state.names.read().unwrap();
        if let Some(name) = names.get(&id) {
            return Some(name.clone());
        }
    }
    match app_state.esi_client.get_name(id).await {
        Ok(name) => {
            let _lock = app_state.names_file_lock.lock().await;
            let mut names = app_state.names.write().unwrap();
            names.insert(id, name.clone());
            save_names(&names);
            Some(name)
        }
        Err(e) => {
            warn!("Failed to fetch name for ID {}: {}", id, e);
            None
        }
    }
}

async fn get_closest_celestial(
    app_state: &Arc<AppState>,
    zk_data: &ZkData,
) -> Option<Arc<Celestial>> {
    let killmail = &zk_data.killmail;
    let position = match killmail.victim.position.as_ref() {
        None => {
            warn!(
                "Killmail {} has no position data for victim: {:#?}\nLocation ID: {:#?}",
                killmail.killmail_id, killmail.victim.position, zk_data.zkb.location_id
            );
            return None;
        }
        Some(pos) => pos,
    };
    let cache_key = killmail.solar_system_id;

    if let Some(celestial) = app_state.celestial_cache.get(&cache_key) {
        return Some(celestial);
    }

    let celestial = app_state
        .esi_client
        .get_celestial(killmail.solar_system_id, position.x, position.y, position.z)
        .await;

    match celestial {
        Ok(celestial) => {
            let celestial_arc = Arc::new(celestial);
            app_state
                .celestial_cache
                .insert(cache_key, celestial_arc.clone())
                .await;
            Some(celestial_arc)
        }
        Err(e) => {
            warn!(
                "Failed to fetch celestial data for system {} and location {:#?}: {:#?}",
                killmail.solar_system_id, zk_data.zkb.location_id, e
            );
            None
        }
    }
}

async fn select_best_entity_for_display(
    app_state: &Arc<AppState>,
    zk_data: &ZkData,
    matched_attackers: &HashSet<AttackerKey>,
    victim_matched: bool,
) -> Option<MatchedEntity> {
    let mut potential_matches = Vec::new();

    // Find the full Attacker structs corresponding to the matched keys
    let attacker_map: HashMap<AttackerKey, &Attacker> = zk_data
        .killmail
        .attackers
        .iter()
        .map(|a| (AttackerKey::new(a), a))
        .collect();

    for key in matched_attackers {
        if let Some(attacker) = attacker_map.get(key) {
            if let Some(type_id) = attacker.ship_type_id.or(attacker.weapon_type_id) {
                if let Some(group_id) = get_ship_group_id(app_state, type_id).await {
                    potential_matches.push(MatchedEntity {
                        ship_name: get_name(app_state, type_id as u64)
                            .await
                            .unwrap_or_default(),
                        type_id,
                        group_id,
                        corp_id: attacker.corporation_id,
                        alliance_id: attacker.alliance_id,
                        color: Color::Green,
                    });
                }
            }
        }
    }

    // If the victim was also a match, add them to the list of potential entities to display
    if victim_matched {
        if let Some(group_id) =
            get_ship_group_id(app_state, zk_data.killmail.victim.ship_type_id).await
        {
            potential_matches.push(MatchedEntity {
                ship_name: get_name(app_state, zk_data.killmail.victim.ship_type_id as u64)
                    .await
                    .unwrap_or_default(),
                type_id: zk_data.killmail.victim.ship_type_id,
                group_id,
                corp_id: zk_data.killmail.victim.corporation_id,
                alliance_id: zk_data.killmail.victim.alliance_id,
                color: Color::Red,
            });
        }
    }

    // Prioritize the list and return the best one
    potential_matches.into_iter().min_by_key(|entity| {
        SHIP_GROUP_PRIORITY
            .iter()
            .position(|&p| p == entity.group_id)
            .unwrap_or(usize::MAX)
    })
}

// --- Message Sending and Embed Building ---

pub async fn send_killmail_message(
    http: &Arc<Http>,
    app_state: &Arc<AppState>,
    subscription: &Subscription,
    zk_data: &ZkData,
    filter_result: NamedFilterResult,
) -> Result<(), KillmailSendError> {
    let channel = match subscription.action.channel_id.parse::<u64>() {
        Ok(id) => ChannelId(id),
        Err(e) => {
            error!(
                "[Kill: {}] Invalid channel ID '{}': {:#?}",
                zk_data.kill_id, subscription.action.channel_id, e
            );
            return Err(KillmailSendError::Other("Invalid channel ID".into()));
        }
    };
    let embed = build_killmail_embed(app_state, zk_data, &filter_result).await; // Pass it here

    let content = match &subscription.action.ping_type {
        None => None,
        Some(ping_type) => {
            let kill_time = DateTime::parse_from_rfc3339(&zk_data.killmail.killmail_time)
                .unwrap_or_else(|_| Utc::now().into());
            let kill_age = Utc::now().signed_duration_since(kill_time);

            let max_delay = ping_type.max_ping_delay_in_minutes().unwrap_or(0);
            if max_delay == 0 || kill_age.num_minutes() <= max_delay as i64 {
                let channel_id = subscription.action.channel_id.parse::<u64>().unwrap_or(0);
                let mut ping_times = app_state.last_ping_times.lock().await;

                let now = Instant::now();
                let last_ping = ping_times
                    .entry(channel_id)
                    .or_insert(now - Duration::from_secs(301));

                if now.duration_since(*last_ping) > Duration::from_secs(300) {
                    *last_ping = now;
                    Some(match ping_type {
                        PingType::Here { .. } => "@here",
                        PingType::Everyone { .. } => "@everyone",
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
    };

    let result = channel
        .send_message(http, |m| {
            if let Some(content) = content {
                m.content(content)
            } else {
                m
            }
            .set_embed(embed)
        })
        .await;

    if let Err(e) = result {
        if let serenity::Error::Http(http_err) = &e {
            match &**http_err {
                Error::UnsuccessfulRequest(resp) => match resp.status_code {
                    serenity::http::StatusCode::FORBIDDEN => {
                        error!(
                            "[Kill: {}] Forbidden to send message to channel {}. Removing subscriptions.",
                            zk_data.kill_id, channel
                        );
                        return Err(KillmailSendError::CleanupChannel(e));
                    }
                    serenity::http::StatusCode::NOT_FOUND => {
                        error!(
                            "[Kill: {}] Channel {} not found. Removing subscriptions.",
                            zk_data.kill_id, channel
                        );
                        return Err(KillmailSendError::CleanupChannel(e));
                    }
                    _ => {}
                },
                _ => {
                    error!(
                        "[Kill: {}] HTTP error while sending message to channel {}: {:#?}",
                        zk_data.kill_id, channel, e
                    );
                    return Err(KillmailSendError::Other(Box::new(e)));
                }
            }
        }
        error!(
            "[Kill: {}] Failed to send message to channel {}: {:#?}",
            zk_data.kill_id, channel, e
        );
        return Err(KillmailSendError::Other(Box::new(e)));
    }
    info!(
        "[Kill: {}] Sent message to channel {}",
        zk_data.kill_id, channel
    );
    Ok(())
}

fn abbreviate_number(n: f64) -> String {
    if n < 1_000.0 {
        return format!("{:.0}", n);
    }
    if n < 1_000_000.0 {
        return format!("{:.1}K", n / 1_000.0);
    }
    if n < 1_000_000_000.0 {
        return format!("{:.1}M", n / 1_000_000.0);
    }
    if n < 1_000_000_000_000.0 {
        return format!("{:.1}B", n / 1_000_000_000.0);
    }
    format!("{:.1}T", n / 1_000_000_000_000.0)
}

fn get_relative_time(killmail_time: &str) -> String {
    let kill_time = match DateTime::parse_from_rfc3339(killmail_time) {
        Ok(t) => t.with_timezone(&Utc),
        Err(e) => {
            error!("Failed to parse killmail time '{}': {}", killmail_time, e);
            return "just now".to_string();
        } // Early return on parse failure
    };
    let now = Utc::now();
    let diff = now.signed_duration_since(kill_time);

    let seconds = diff.num_seconds();
    if seconds < 1 {
        return "just now".to_string();
    }
    if seconds == 1 {
        return "1 second later".to_string();
    }

    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let weeks = days / 7;
    // Approximation: 4 weeks per month. Note: This is not perfectly accurate.
    let months = weeks / 4;
    let years = months / 12;

    if years > 1 {
        return format!("{} years later", years);
    }
    if years == 1 {
        return "1 year later".to_string();
    }
    if months > 1 {
        return format!("{} months later", months);
    }
    if months == 1 {
        return "1 month later".to_string();
    }
    if weeks > 1 {
        return format!("{} weeks later", weeks);
    }
    if weeks == 1 {
        return "1 week later".to_string();
    }
    if days > 1 {
        return format!("{} days later", days);
    }
    if days == 1 {
        return "1 day later".to_string();
    }
    if hours > 1 {
        return format!("{} hours later", hours);
    }
    if hours == 1 {
        return "1 hour later".to_string();
    }
    if minutes > 1 {
        return format!("{} minutes later", minutes);
    }
    if minutes == 1 {
        return "1 minute later".to_string();
    }

    format!("{} seconds later", seconds)
}

fn str_alliance_icon(id: u64) -> String {
    format!("https://images.evetech.net/alliances/{}/logo?size=64", id)
}

#[allow(unused)]
fn str_corp_icon(id: u64) -> String {
    format!(
        "https://images.evetech.net/corporations/{}/logo?size=64",
        id
    )
}

fn str_ship_icon(id: u32) -> String {
    format!("https://images.evetech.net/types/{}/icon?size=64", id)
}

#[allow(unused)]
fn str_pilot_zk(id: u64) -> String {
    format!("https://zkillboard.com/character/{}/", id)
}
fn str_corp_zk(id: u64) -> String {
    format!("https://zkillboard.com/corporation/{}/", id)
}
fn str_alliance_zk(id: u64) -> String {
    format!("https://zkillboard.com/alliance/{}/", id)
}
fn str_system_dotlan(id: u32) -> String {
    format!("http://evemaps.dotlan.net/system/{}", id)
}
fn str_region_dotlan(id: u32) -> String {
    format!("http://evemaps.dotlan.net/region/{}", id)
}
fn str_location(id: u64) -> String {
    format!("https://zkillboard.com/location/{}/", id)
}

enum DotlanJumpType {
    Super,
    Fax,
    Blops,
}
fn str_jump_dotlan(from: &str, to: &str, with: DotlanJumpType) -> String {
    let with = match with {
        DotlanJumpType::Super => "Nyx",
        DotlanJumpType::Fax => "Lif",
        DotlanJumpType::Blops => "Sin",
    };
    format!(
        "https://evemaps.dotlan.net/jump/{},555/{}:{}",
        with, from, to
    )
}

// Returns: ID, count
fn most_common_ship_type(attackers: &[Attacker]) -> Option<(u64, u64)> {
    attackers
        .iter()
        .fold(HashMap::new(), |mut map, val| {
            if val.ship_type_id.is_none() {
                return map;
            }
            map.entry(val.ship_type_id.unwrap() as u64)
                .and_modify(|freq| *freq += 1)
                .or_insert(1u64);
            map
        })
        .iter()
        .max_by(|a, b| a.1.cmp(b.1))
        .map(|(k, v)| (*k, *v))
}

/// Formats a DateTime object into a YYYYMMDDHH00 string, suitable for battle report URLs.
/// This function effectively rounds the time down to the nearest hour.
fn format_datetime_to_timestamp(date: &DateTime<FixedOffset>) -> String {
    // Convert to UTC to ensure consistency, similar to getUTCFullYear, etc.
    let date_utc = date.with_timezone(&Utc);
    // Format the date and append "00" for the minutes.
    format!("{}00", date_utc.format("%Y%m%d%H"))
}

async fn build_killmail_embed(
    app_state: &Arc<AppState>,
    zk_data: &ZkData,
    named_filter_result: &NamedFilterResult,
) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    let filter_result = &named_filter_result.filter_result;
    let killmail = &zk_data.killmail;

    let system_info = get_system(app_state, killmail.solar_system_id).await;
    let system_name = system_info.as_ref().map_or("Unknown System", |s| &s.name);
    let system_id = system_info.as_ref().map_or(0, |s| s.id);
    let region_name = system_info.as_ref().map_or("Unknown Region", |s| &s.region);
    let region_id = system_info.as_ref().map_or(0, |s| s.region_id);
    let most_common_ship = most_common_ship_type(&killmail.attackers);
    let best_match = select_best_entity_for_display(
        app_state,
        zk_data,
        &filter_result.matched_attackers,
        filter_result.matched_victim,
    )
        .await;

    let total_value_str = abbreviate_number(zk_data.zkb.total_value);
    let relative_time = get_relative_time(&killmail.killmail_time);
    let killmail_time = DateTime::parse_from_rfc3339(&killmail.killmail_time)
        .unwrap_or_else(|_| Utc::now().with_timezone(&FixedOffset::east_opt(0).unwrap()));

    let killmail_url = format!("https://zkillboard.com/kill/{}/", killmail.killmail_id);
    let related_br = format!(
        "https://br.evetools.org/related/{}/{}",
        system_id,
        format_datetime_to_timestamp(&killmail_time)
    );

    // Location details
    let mut location_details = String::new();
    if let Some(celestial) = get_closest_celestial(app_state, zk_data).await {
        let distance_km = celestial.distance / 1000.0;
        let distance_str = if distance_km > 1_500_000.0 {
            format!("{:.1} AU", distance_km / 149_597_870.7)
        } else {
            format!("{:.1} km", distance_km)
        };
        location_details = format!(
            "on: [{}]({}), {} away",
            celestial.item_name,
            str_location(celestial.item_id),
            distance_str,
        );
    }

    // Victim details
    let victim_ship_name = get_name(app_state, killmail.victim.ship_type_id as u64)
        .await
        .unwrap_or_else(|| "Unknown Ship".to_string());
    let victim_corp_id = killmail.victim.corporation_id.unwrap_or(0);
    // let victim_char_id = killmail.victim.character_id.unwrap_or(0);
    // let victim_name = get_name(app_state, victim_char_id)
    //     .await
    //     .or_else(|| futures::executor::block_on(get_name(app_state, victim_corp_id)))
    //     .unwrap_or_else(|| "N/A".to_string());
    let mut victim_details = String::new();
    if let Some(alliance_id) = killmail.victim.alliance_id {
        if let Some(name) = get_name(app_state, alliance_id).await {
            victim_details.push_str(&format!(
                "[{}]({})",
                &name[0..name.len().min(40)],
                str_alliance_zk(alliance_id)
            ));
        }
    }
    if victim_corp_id != 0 && victim_details.is_empty() {
        if let Some(name) = get_name(app_state, victim_corp_id).await {
            victim_details.push_str(&format!(
                "[{}]({})",
                &name[0..name.len().min(40)],
                str_corp_zk(victim_corp_id)
            ));
        }
    }

    // Title + Author Text
    let title;
    let mut author_text;
    if filter_result.min_pilots.is_some() {
        author_text = format!(
            "{}+ ships in {} ({})",
            killmail.attackers.len(),
            system_name,
            region_name
        );
        title = if let Some((type_id, count)) = &most_common_ship {
            let ship_name = get_name(app_state, *type_id)
                .await
                .unwrap_or_else(|| "Unknown Ship".to_string());
            format!("`{}` died to {}x `{}`", victim_ship_name, count, ship_name)
        } else {
            "Missing 0".to_string()
        };
    } else if let Some(ref matched_ship) = best_match {
        author_text = format!(
            "{} in {} ({})",
            matched_ship.ship_name, system_name, region_name
        );
        if let Some((type_id, count)) = most_common_ship {
            match matched_ship.color {
                Color::Green => {
                    title = format!("`{}` destroyed", victim_ship_name);
                }
                Color::Red => {
                    let ship_name = get_name(app_state, type_id)
                        .await
                        .unwrap_or_else(|| "Unknown Ship".to_string());
                    title = format!("`{}` died to {}x `{}`", victim_ship_name, count, ship_name)
                }
            }
        } else {
            title = relative_time.clone();
        }
    } else {
        title = if let Some((type_id, count)) = &most_common_ship {
            let ship_name = get_name(app_state, *type_id)
                .await
                .unwrap_or_else(|| "Unknown Ship".to_string());
            format!("`{}` died to {}x `{}`", victim_ship_name, count, ship_name)
        } else {
            format!("`{}` died", victim_ship_name)
        };
        author_text = format!("Killmail in {} ({})", system_name, region_name);
    }
    author_text += &format!("\nPosted {}", relative_time);

    // console.log('attackerparams.data');

    // Find the attacker who made the final blow
    let last_hit_attacker: Option<&Attacker> = killmail.attackers.iter().find(|a| a.final_blow);

    // Determine the primary icon to render for the embed thumbnail and the affiliation icon for the author field.
    // The logic follows a specific priority:
    // 1. Matched ship from the filter
    // 2. Victim's ship
    // 3. Final blow attacker's ship
    // 4. Final blow attacker's weapon
    let (id_of_icon_to_render, affiliation_icon_url_to_render) = {
        if let Some(ref matched_ship) = &best_match {
            let type_id = matched_ship.type_id;
            let url = if let Some(alliance_id) = matched_ship.alliance_id {
                str_alliance_icon(alliance_id)
            } else if let Some(corp_id) = matched_ship.corp_id {
                str_corp_icon(corp_id)
            } else {
                str_ship_icon(type_id)
            };
            (type_id, url)
        } else if killmail.victim.ship_type_id != 0 {
            let url = if let Some(alliance_id) = killmail.victim.alliance_id {
                str_alliance_icon(alliance_id)
            } else if let Some(corporation_id) = killmail.victim.corporation_id {
                str_corp_icon(corporation_id)
            } else {
                str_ship_icon(killmail.victim.ship_type_id)
            };
            (killmail.victim.ship_type_id, url)
        } else if let Some(attacker) = last_hit_attacker {
            if let Some(ship_type_id) = attacker.ship_type_id {
                let url = if let Some(alliance_id) = attacker.alliance_id {
                    str_alliance_icon(alliance_id)
                } else if let Some(corporation_id) = attacker.corporation_id {
                    str_corp_icon(corporation_id)
                } else {
                    str_ship_icon(ship_type_id)
                };
                (ship_type_id, url)
            } else if let Some(weapon_type_id) = attacker.weapon_type_id {
                let url = if let Some(alliance_id) = attacker.alliance_id {
                    str_alliance_icon(alliance_id)
                } else if let Some(corporation_id) = attacker.corporation_id {
                    str_corp_icon(corporation_id)
                } else {
                    str_ship_icon(weapon_type_id)
                };
                (weapon_type_id, url)
            } else {
                (0, String::new()) // Fallback
            }
        } else {
            (0, String::new()) // Fallback
        }
    };
    if id_of_icon_to_render == 0 {
        warn!(
            "Could not determine an icon to render for killmail {}",
            zk_data.kill_id
        );
    }
    trace!("Rendering icon: {}", str_ship_icon(id_of_icon_to_render));

    // --- Attacker Affiliation List ---

    // Concurrently, fetch names for all unique alliance and corporation IDs involved.
    let mut ids_to_fetch: Vec<u64> = killmail
        .attackers
        .iter()
        .filter_map(|a| a.alliance_id.or(a.corporation_id))
        .collect();
    ids_to_fetch.sort_unstable();
    ids_to_fetch.dedup();

    let name_futures = ids_to_fetch.iter().map(|id| get_name(app_state, *id));
    let name_results = join_all(name_futures).await;

    let id_to_name: HashMap<u64, String> = ids_to_fetch
        .into_iter()
        .zip(name_results)
        .filter_map(|(id, name_opt)| name_opt.map(|name| (id, name)))
        .collect();

    // Count attackers by their affiliation name.
    let mut alliance_counts: HashMap<String, u32> = HashMap::new();
    for attacker in &killmail.attackers {
        if let Some(id) = attacker.alliance_id.or(attacker.corporation_id) {
            let name = id_to_name
                .get(&id)
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string());
            *alliance_counts.entry(name).or_insert(0) += 1;
        }
    }

    // Sort affiliations by the number of attackers.
    let mut sorted_attackers: Vec<(String, u32)> = alliance_counts.into_iter().collect();
    sorted_attackers.sort_by(|a, b| b.1.cmp(&a.1));

    // --- Format Attacker List for Embed ---

    let mut attacker_alliances = "```\n".to_string();
    let mut displayed_entries: Vec<(String, u32)> = Vec::new();
    let mut others_count = 0;
    const DISPLAY_THRESHOLD: u32 = 10;

    // Separate entries into "displayed" and "others".
    for (i, (name, count)) in sorted_attackers.into_iter().enumerate() {
        if count >= DISPLAY_THRESHOLD || i == 0 {
            displayed_entries.push((name[0..name.len().min(26)].to_string(), count));
        } else {
            others_count += count;
        }
    }

    // Calculate padding for alignment.
    let mut max_name_length = 0;
    const OTHERS_STR: &str = "...others";
    for (name, _) in &displayed_entries {
        max_name_length = max_name_length.max(name.len());
    }
    if others_count > 0 {
        max_name_length = max_name_length.max(OTHERS_STR.len());
    }
    max_name_length = max_name_length.min(26);
    const PADDING: usize = 3;

    // Build the formatted string.
    for (key, value) in displayed_entries {
        let spaces = " ".repeat(max_name_length - key.len().min(26) + PADDING);
        let formatted_key = if key.len() > 26 {
            format!("{}-\n{}", &key[..26], &key[26..])
        } else {
            key
        };
        attacker_alliances.push_str(&format!("{}{}{}{}\n", formatted_key, spaces, "x", value));
    }

    if others_count > 0 {
        let spaces = " ".repeat(max_name_length - OTHERS_STR.len() + PADDING);
        attacker_alliances.push_str(&format!(
            "{}{}{}{}\n",
            OTHERS_STR, spaces, "x", others_count
        ));
    }

    attacker_alliances = format!("{}```", attacker_alliances);

    let range_details = if let Some(matched_system_range) = &filter_result.light_year_range {
        let matched_base_system_name = get_system(app_state, matched_system_range.system_id)
            .await
            .map_or_else(|| "Unknown System".to_string(), |s| s.name);
        if matched_system_range.range > 0.0 {
            format!(
                "{:.1} LY from {} ([Supers]({})|[FAX]({})|[Blops]({}))",
                matched_system_range.range,
                matched_base_system_name,
                str_jump_dotlan(
                    &matched_base_system_name,
                    system_name,
                    DotlanJumpType::Super
                ),
                str_jump_dotlan(&matched_base_system_name, system_name, DotlanJumpType::Fax),
                str_jump_dotlan(
                    &matched_base_system_name,
                    system_name,
                    DotlanJumpType::Blops
                )
            )
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let affiliation = format!(
        "{}victim: {}\nin: [{}]({}) ([{}]({})){}\n{}",
        attacker_alliances,
        victim_details,
        system_name,
        str_system_dotlan(system_id),
        region_name,
        str_region_dotlan(region_id),
        range_details,
        location_details,
    );

    // Build the embed
    embed.title(title);
    embed.url(killmail_url.clone());
    embed.author(|a| {
        a.name(author_text)
            .url(related_br)
            .icon_url(affiliation_icon_url_to_render)
    });
    embed.thumbnail(str_ship_icon(id_of_icon_to_render));
    embed.color(match best_match.map(|bm| bm.color).unwrap_or_default() {
        Color::Green => Colour::DARK_GREEN,
        Color::Red => Colour::RED,
    });
    embed.field(
        format!("({}) Attackers Involved", killmail.attackers.len()),
        affiliation,
        false,
    );
    embed.footer(|f| {
        f.text(format!(
            "Value: {} • EVETime: {}",
            total_value_str,
            killmail_time.format("%d/%m/%Y, %H:%M"),
        ))
    });
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(&killmail.killmail_time) {
        embed.timestamp(timestamp.to_rfc3339());
    }

    embed
}
