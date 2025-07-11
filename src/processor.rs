use crate::config::{
    AppState, Filter, FilterNode, SimpleFilter, Subscription, System, SystemRange, Target,
    TargetableCondition,
};
use crate::discord_bot::{get_name, get_ship_group_id, get_system};
use crate::models::ZkData;
use chrono::Timelike;
use futures::future::{BoxFuture, FutureExt};
use serenity::model::prelude::GuildId;
use std::ops::RangeInclusive;
use std::str::FromStr;
use std::sync::Arc;
use tracing::warn;

#[derive(Debug, Clone, Default)]
pub struct NamedFilterResult {
    pub name: String,
    pub filter_result: FilterResult,
}

#[derive(Debug, Clone, Default)]
pub struct FilterResult {
    pub(crate) matched_ship: Option<MatchedShip>,
    pub(crate) color: Option<Color>,
    pub(crate) min_pilots: Option<u32>,
    pub(crate) light_year_range: Option<SystemRange>,
}

#[derive(Debug, Copy, Clone, Default)]
pub(crate) enum Color {
    Green,
    #[default]
    Red,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MatchedShip {
    pub(crate) ship_name: String,
    pub(crate) type_id: u32,
    pub(crate) corp_id: Option<u64>,
    pub(crate) alliance_id: Option<u64>,
}

pub async fn process_killmail(
    app_state: &Arc<AppState>,
    zk_data: &ZkData,
) -> Vec<(GuildId, Subscription, NamedFilterResult)> {
    // Clone the subscriptions to release the lock before the async loop.
    // We flatten the map of guilds into a single list of all subscriptions.
    let all_subscriptions = {
        let subs_guard = app_state.subscriptions.read().unwrap();
        subs_guard.clone()
    };
    // The `subs_guard` is dropped here, releasing the lock.

    let mut matched_subscriptions = Vec::new();
    let _kill_id = zk_data.killmail.killmail_id;

    // Iterate over the cloned subscriptions. No lock is held here.
    for (guild_id, subscriptions) in all_subscriptions.iter() {
        for subscription in subscriptions {
            if let Some(result) =
                evaluate_filter_node(&subscription.root_filter, zk_data, app_state).await
            {
                matched_subscriptions.push((*guild_id, subscription.clone(), result));
            }
        }
    }

    matched_subscriptions
}

fn evaluate_filter_node<'a>(
    node: &'a FilterNode,
    zk_data: &'a ZkData,
    app_state: &'a Arc<AppState>,
) -> BoxFuture<'a, Option<NamedFilterResult>> {
    async move {
        let kill_id = zk_data.killmail.killmail_id;
        match node {
            FilterNode::Condition(filter) => evaluate_filter(filter, zk_data, app_state).await,
            FilterNode::And(nodes) => {
                let mut results = Vec::new();
                for n in nodes {
                    if let Some(result) = evaluate_filter_node(n, zk_data, app_state).await {
                        results.push(result);
                    } else {
                        tracing::trace!(
                            "[Kill: {}] Filter condition failed for node: {}",
                            kill_id,
                            n.name()
                        );
                        return None; // One failure means the whole And block fails
                    }
                }
                // Merge results
                let final_result = NamedFilterResult {
                    name: results
                        .iter()
                        .fold(String::new(), |acc, b| format!("{} + {}", acc, b.name)),
                    filter_result: FilterResult {
                        color: results
                            .iter()
                            .find(|r| r.filter_result.color.is_some())
                            .map_or(results[0].filter_result.color, |r| r.filter_result.color),
                        matched_ship: results
                            .iter()
                            .find(|r| r.filter_result.matched_ship.is_some())
                            .and_then(|r| r.filter_result.matched_ship.clone()),
                        min_pilots: results.iter().find_map(|r| r.filter_result.min_pilots),
                        // TODO: fold/accumulate here
                        light_year_range: results
                            .iter()
                            .find(|r| r.filter_result.light_year_range.is_some())
                            .and_then(|r| r.filter_result.light_year_range.clone()),
                    },
                };
                Some(final_result)
            }
            FilterNode::Or(nodes) => {
                for n in nodes {
                    if let Some(result) = evaluate_filter_node(n, zk_data, app_state).await {
                        return Some(result); // Return the first match
                    }
                }
                None
            }
            FilterNode::Not(node) => {
                if evaluate_filter_node(node, zk_data, app_state)
                    .await
                    .is_some()
                {
                    None
                } else {
                    Some(NamedFilterResult {
                        name: format!("Not({})", node.name()),
                        filter_result: Default::default(), // Default success
                    })
                }
            }
        }
    }
    .boxed()
}

fn parse_security_range(s: &str) -> Result<RangeInclusive<f64>, ()> {
    let parts: Vec<&str> = s.split("..=").collect();
    if parts.len() != 2 {
        return Err(());
    }
    let start = f64::from_str(parts[0]).map_err(|_| ())?;
    let end = f64::from_str(parts[1]).map_err(|_| ())?;
    Ok(start..=end)
}

fn distance(system1: &System, system2: &System) -> f64 {
    const LY_PER_M: f64 = 1.0 / 9_460_730_472_580_800.0;
    let dx = system1.x - system2.x;
    let dy = system1.y - system2.y;
    let dz = system1.z - system2.z;
    (dx * dx + dy * dy + dz * dz).sqrt() * LY_PER_M
}

async fn evaluate_filter(
    filter: &Filter,
    zk_data: &ZkData,
    app_state: &Arc<AppState>,
) -> Option<NamedFilterResult> {
    let killmail = &zk_data.killmail;

    let filter_result = match filter {
        Filter::Simple(sf) => {
            match sf {
                SimpleFilter::TotalValue { min, max } => {
                    let total_value = zk_data.zkb.total_value;
                    if min.is_none_or(|m| total_value >= m as f64)
                        && max.is_none_or(|m| total_value <= m as f64)
                    {
                        Some(Default::default())
                    } else {
                        None
                    }
                }
                SimpleFilter::DroppedValue { min, max } => {
                    let dropped_value = zk_data.zkb.dropped_value;
                    if min.is_none_or(|m| dropped_value >= m as f64)
                        && max.is_none_or(|m| dropped_value <= m as f64)
                    {
                        Some(Default::default())
                    } else {
                        None
                    }
                }
                SimpleFilter::Region(region_ids) => {
                    if get_system(app_state, killmail.solar_system_id)
                        .await
                        .is_some_and(|s| region_ids.contains(&s.region_id))
                    {
                        Some(Default::default())
                    } else {
                        None
                    }
                }
                SimpleFilter::System(system_ids) => {
                    if system_ids.contains(&killmail.solar_system_id) {
                        Some(Default::default())
                    } else {
                        None
                    }
                }
                SimpleFilter::Security(range_str) => {
                    if let (Some(system), Ok(range)) = (
                        get_system(app_state, killmail.solar_system_id).await,
                        parse_security_range(range_str),
                    ) {
                        let rounded_sec = (system.security_status * 10.0).round() / 10.0;
                        if range.contains(&rounded_sec) {
                            return Some(Default::default());
                        }
                    }
                    None
                }
                SimpleFilter::LyRangeFrom(system_ranges) => {
                    if let Some(killmail_system) =
                        get_system(app_state, killmail.solar_system_id).await
                    {
                        let mut matched_ranges: Vec<SystemRange> = vec![];
                        for system_range in system_ranges {
                            if let Some(target_system) =
                                get_system(app_state, system_range.system_id).await
                            {
                                let distance = distance(&killmail_system, &target_system);
                                println!(
                                    "[Kill: {}] Calculating distance between {} and {}: {} ly",
                                    killmail.killmail_id,
                                    killmail_system.name,
                                    target_system.name,
                                    distance,
                                );
                                if distance <= system_range.range {
                                    matched_ranges.push(SystemRange {
                                        system_id: target_system.id,
                                        range: distance,
                                    })
                                }
                            } else {
                                warn!(
                                    "Could not find target system {} for LY range check",
                                    system_range.system_id
                                );
                            }
                        }
                        if matched_ranges.is_empty() {
                            None
                        } else {
                            // Sort descending, pop from the back for the shortest range match
                            matched_ranges.sort_by(|a, b| b.range.total_cmp(&a.range));

                            Some(FilterResult {
                                matched_ship: None,
                                color: None,
                                min_pilots: None,
                                light_year_range: Some(
                                    matched_ranges.pop().expect("non-empty matched vec"),
                                ),
                            })
                        }
                    } else {
                        warn!(
                            "Could not find killmail system {} for LY range check",
                            killmail.solar_system_id
                        );
                        None
                    }
                }
                SimpleFilter::IsNpc(is_npc) => {
                    if zk_data.zkb.npc == *is_npc {
                        Some(Default::default())
                    } else {
                        None
                    }
                }
                SimpleFilter::IsSolo(is_solo) => {
                    if zk_data.zkb.solo == *is_solo {
                        Some(Default::default())
                    } else {
                        None
                    }
                }
                SimpleFilter::Pilots { min, max } => {
                    let num_pilots = (killmail.attackers.len() + 1) as u32;
                    if min.is_none_or(|m| num_pilots >= m) && max.is_none_or(|m| num_pilots <= m) {
                        Some(FilterResult {
                            color: None,
                            matched_ship: None,
                            min_pilots: Some(min.unwrap_or(0)),
                            light_year_range: None,
                        })
                    } else {
                        None
                    }
                }
                SimpleFilter::TimeRange { start, end } => {
                    let res = if let Ok(killmail_time) =
                        chrono::DateTime::parse_from_rfc3339(&killmail.killmail_time)
                    {
                        let killmail_hour = killmail_time.hour();
                        if start <= end {
                            // Simple range within the same day
                            killmail_hour >= *start && killmail_hour <= *end
                        } else {
                            // Range extends across midnight (e.g., 22:00 to 04:00)
                            killmail_hour >= *start || killmail_hour <= *end
                        }
                    } else {
                        warn!("Failed to parse killmail_time: {}", killmail.killmail_time);
                        false
                    };
                    if res {
                        Some(Default::default())
                    } else {
                        None
                    }
                }
                SimpleFilter::IgnoreHighStanding { synched_by_user_id, source, source_entity_id } => {
                    let standings_map = app_state.user_standings.read().unwrap();
                    if let Some(user_standings) = standings_map.get(&serenity::model::id::UserId(*synched_by_user_id)) {
                        // Determine the full list of implicitly blue IDs based on the sync source and context from stored tokens.
                        let mut implicit_blues: Vec<u64> = vec![*source_entity_id];
                        
                        // Find a token that gives context to the synced entity to discover its parent alliance, if any.
                        let context_token = user_standings.tokens.iter().find(|t| match source {
                            crate::config::StandingSource::Character => t.character_id == *source_entity_id,
                            crate::config::StandingSource::Corporation => t.corporation_id == *source_entity_id,
                            crate::config::StandingSource::Alliance => t.alliance_id == Some(*source_entity_id),
                        });

                        if let Some(token) = context_token {
                            if *source == crate::config::StandingSource::Character {
                                implicit_blues.push(token.corporation_id);
                                if let Some(id) = token.alliance_id {
                                    implicit_blues.push(id);
                                }
                            }
                            if *source == crate::config::StandingSource::Corporation {
                                if let Some(id) = token.alliance_id {
                                    implicit_blues.push(id);
                                }
                            }
                        }

                        let contacts = user_standings.contact_lists.contacts.get(source_entity_id);

                        for attacker in &killmail.attackers {
                            let attacker_ids = [attacker.character_id, attacker.corporation_id, attacker.alliance_id];

                            for id in attacker_ids.iter().flatten() {
                                // Case 1: Attacker is implicitly blue (self, own corp, own alliance).
                                if implicit_blues.contains(id) {
                                    return None;
                                }

                                // Case 2: Attacker is explicitly blue in the contact list.
                                if let Some(contact_list) = contacts {
                                    if contact_list.iter().any(|c| c.contact_id == *id && c.standing >= 5.0) {
                                        return None;
                                    }
                                }
                            }
                        }
                    }
                    // If we get here, no blue attackers were found. The filter passes.
                    Some(Default::default())
                }
            }
        }
        Filter::Targeted(tf) => {
            let mut result = FilterResult {
                matched_ship: None,
                color: None,
                min_pilots: None,
                light_year_range: None,
            };

            let victim_match = if tf.target.is_victim() {
                match &tf.condition {
                    TargetableCondition::Alliance(alliance_ids) => killmail
                        .victim
                        .alliance_id
                        .is_some_and(|id| alliance_ids.contains(&id)),
                    TargetableCondition::Corporation(corporation_ids) => killmail
                        .victim
                        .corporation_id
                        .is_some_and(|id| corporation_ids.contains(&id)),
                    TargetableCondition::Character(character_ids) => killmail
                        .victim
                        .character_id
                        .is_some_and(|id| character_ids.contains(&id)),
                    TargetableCondition::ShipType(ship_type_ids) => {
                        if ship_type_ids.contains(&killmail.victim.ship_type_id) {
                            let ship_name =
                                get_name(app_state, killmail.victim.ship_type_id as u64)
                                    .await
                                    .unwrap_or_default();
                            let matched_ship = MatchedShip {
                                ship_name,
                                type_id: killmail.victim.ship_type_id,
                                corp_id: killmail.victim.corporation_id,
                                alliance_id: killmail.victim.alliance_id,
                            };
                            result.matched_ship = Some(matched_ship);
                            true
                        } else {
                            false
                        }
                    }
                    TargetableCondition::ShipGroup(ship_group_ids_as_type_ids) => {
                        // It is implicit that these "ship group IDs" are actually ship type IDs, and thus
                        // must be converted to the proper ship group IDs before use.
                        let mut ship_group_ids = vec![];
                        for type_id in ship_group_ids_as_type_ids {
                            if let Some(group_id) = get_ship_group_id(app_state, *type_id).await {
                                ship_group_ids.push(group_id);
                            } else {
                                warn!("Failed to get ship group ID for type ID {}", type_id);
                            }
                        }

                        if let Some(victim_ship_group_id) =
                            get_ship_group_id(app_state, killmail.victim.ship_type_id).await
                        {
                            if ship_group_ids.contains(&victim_ship_group_id) {
                                let ship_name =
                                    get_name(app_state, killmail.victim.ship_type_id as u64)
                                        .await
                                        .unwrap_or_default();
                                let matched_ship = MatchedShip {
                                    ship_name,
                                    type_id: killmail.victim.ship_type_id,
                                    corp_id: killmail.victim.corporation_id,
                                    alliance_id: killmail.victim.alliance_id,
                                };
                                result.matched_ship = Some(matched_ship);
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    TargetableCondition::NameFragment(fragment) => {
                        let lower_fragment = fragment.to_lowercase();
                        if let Some(name) =
                            get_name(app_state, killmail.victim.ship_type_id as u64).await
                        {
                            name.to_lowercase().contains(&lower_fragment)
                        } else {
                            false
                        }
                    }
                }
            } else {
                false
            };

            // TODO: !victim_match prevents color mismatch, however there needs to be a ranking of
            // ship groups such that supercarrier kills appear rather than a dreadnought death
            let attacker_match = if tf.target.is_attacker() && !victim_match {
                match &tf.condition {
                    TargetableCondition::Alliance(alliance_ids) => killmail
                        .attackers
                        .iter()
                        .any(|a| a.alliance_id.is_some_and(|id| alliance_ids.contains(&id))),
                    TargetableCondition::Corporation(corporation_ids) => {
                        killmail.attackers.iter().any(|a| {
                            a.corporation_id
                                .is_some_and(|id| corporation_ids.contains(&id))
                        })
                    }
                    TargetableCondition::Character(character_ids) => killmail
                        .attackers
                        .iter()
                        .any(|a| a.character_id.is_some_and(|id| character_ids.contains(&id))),
                    TargetableCondition::ShipType(ship_type_ids) => {
                        let mut matched = false;
                        for attacker in &killmail.attackers {
                            if let Some(ship_id) = attacker.ship_type_id {
                                if ship_type_ids.contains(&ship_id) {
                                    let ship_name = get_name(app_state, ship_id as u64)
                                        .await
                                        .unwrap_or_default();
                                    let matched_ship = MatchedShip {
                                        ship_name,
                                        type_id: ship_id,
                                        corp_id: attacker.corporation_id,
                                        alliance_id: attacker.alliance_id,
                                    };
                                    result.matched_ship = Some(matched_ship);
                                    matched = true;
                                    break;
                                }
                            }
                        }
                        matched
                    }
                    TargetableCondition::ShipGroup(ship_group_ids_as_type_ids) => {
                        // It is implicit that these "ship group IDs" are actually ship type IDs, and thus
                        // must be converted to the proper ship group IDs before use.
                        let mut ship_group_ids = vec![];
                        for type_id in ship_group_ids_as_type_ids {
                            if let Some(group_id) = get_ship_group_id(app_state, *type_id).await {
                                ship_group_ids.push(group_id);
                            } else {
                                warn!("Failed to get ship group ID for type ID {}", type_id);
                            }
                        }
                        let mut matched = false;
                        for attacker in &killmail.attackers {
                            if let Some(attacker_ship_id) = attacker.ship_type_id {
                                if let Some(attacker_ship_group_id) =
                                    get_ship_group_id(app_state, attacker_ship_id).await
                                {
                                    if ship_group_ids.contains(&attacker_ship_group_id) {
                                        let ship_name =
                                            get_name(app_state, attacker_ship_id as u64)
                                                .await
                                                .unwrap_or_default();
                                        let matched_ship = MatchedShip {
                                            ship_name,
                                            type_id: attacker_ship_id,
                                            corp_id: attacker.corporation_id,
                                            alliance_id: attacker.alliance_id,
                                        };
                                        result.matched_ship = Some(matched_ship);
                                        matched = true;
                                        break;
                                    }
                                }
                            }
                        }
                        matched
                    }
                    TargetableCondition::NameFragment(fragment) => {
                        let lower_fragment = fragment.to_lowercase();
                        let mut matched = false;
                        for attacker in &killmail.attackers {
                            if let Some(ship_id) = attacker.ship_type_id {
                                if let Some(name) = get_name(app_state, ship_id as u64).await {
                                    if name.to_lowercase().contains(&lower_fragment) {
                                        matched = true;
                                        break;
                                    }
                                }
                            }
                        }
                        matched
                    }
                }
            } else {
                false
            };

            let is_match = match tf.target {
                Target::Any => victim_match || attacker_match,
                Target::Attacker => attacker_match,
                Target::Victim => victim_match,
            };

            if is_match {
                result.color = if victim_match {
                    Some(Color::Red)
                } else {
                    Some(Color::Green)
                };
                Some(result)
            } else {
                None
            }
        }
    };

    filter_result.map(|filter_result| NamedFilterResult {
        name: filter.name(),
        filter_result,
    })
}

#[cfg(test)]
mod tests {
    use crate::config::TargetedFilter;
use super::*;
    use crate::config::{AppConfig, Filter, FilterNode, System, SystemRange};
    use crate::models::{Attacker, KillmailData, Position, Victim, ZkData, Zkb};
    use moka::future::Cache;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};
    use tokio::sync::Mutex;

    // Helper to create a mock AppState
    fn mock_app_state() -> Arc<AppState> {
        let mut systems = HashMap::new();
        systems.insert(
            30000142, // Jita
            System {
                id: 30000142,
                name: "Jita".to_string(),
                region_id: 10000002, // The Forge
                region: "The Forge".to_string(),
                security_status: 0.9,
                x: -993254832640.0,
                y: 216484356096.0,
                z: -973193297920.0,
            },
        );
        systems.insert(
            31002222, // Amarr
            System {
                id: 31002222,
                name: "Amarr".to_string(),
                region_id: 10000043, // Domain
                region: "Domain".to_string(),
                security_status: 1.0,
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
        );

        let mut ships = HashMap::new();
        ships.insert(671, 27); // Catalyst, GroupID 27 (Destroyer)
        ships.insert(587, 25); // Rifter, GroupID 25 (Frigate)
        ships.insert(17738, 419); // Golem, GroupID 419 (Marauder)

        let mut names = HashMap::new();
        names.insert(671, "Catalyst".to_string());
        names.insert(587, "Rifter".to_string());
        names.insert(17738, "Golem".to_string());

        Arc::new(AppState {
            subscriptions: Default::default(),
            systems: Arc::new(RwLock::new(systems)),
            ships: Arc::new(RwLock::new(ships)),
            names: Arc::new(RwLock::new(names)),
            celestial_cache: Cache::new(10_000),
            esi_client: Default::default(),
            systems_file_lock: Mutex::new(()),
            ships_file_lock: Mutex::new(()),
            names_file_lock: Mutex::new(()),
            subscriptions_file_lock: Mutex::new(()),
            app_config: Arc::new(AppConfig {
                discord_bot_token: "".to_string(),
                discord_client_id: 0,
                eve_client_id: "".to_string(),
                eve_client_secret: "".to_string(),
            }),
            last_ping_times: Mutex::new(HashMap::new()),
            user_standings: Arc::new(Default::default()),
            user_standings_file_lock: Default::default(),
            sso_states: Arc::new(Default::default()),
        })
    }

    // Helper to create a default ZkData for testing
    fn default_zk_data() -> ZkData {
        ZkData {
            kill_id: 1,
            killmail: KillmailData {
                killmail_id: 1,
                killmail_time: "2025-07-08T12:00:00Z".to_string(),
                solar_system_id: 30000142, // Jita
                victim: Victim {
                    damage_taken: 1000,
                    ship_type_id: 587, // Rifter
                    character_id: Some(1),
                    corporation_id: Some(101),
                    alliance_id: Some(1001),
                    position: Some(Position {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    }),
                    faction_id: None,
                    items: vec![],
                },
                attackers: vec![Attacker {
                    final_blow: true,
                    damage_done: 1000,
                    ship_type_id: Some(671), // Catalyst
                    character_id: Some(2),
                    corporation_id: Some(102),
                    alliance_id: Some(1002),
                    weapon_type_id: Some(3),
                    security_status: 0.5,
                    faction_id: None,
                }],
            },
            zkb: Zkb {
                total_value: 10_000_000.0,
                dropped_value: 1_000_000.0,
                npc: false,
                solo: false,
                location_id: None,
                hash: "".to_string(),
                fitted_value: 0.0,
                destroyed_value: 0.0,
                points: 0,
                awox: false,
                esi: "".to_string(),
            },
        }
    }

    fn user_killmail_data() -> ZkData {
        let json_data = r#"
         {
             "killID": 128389930,
             "zkb": {
               "locationID": 40161548,
               "hash": "d00ad190e832f0ca2965c9946b15527c415a70e7",
               "fittedValue": 5148356869.79,
               "droppedValue": 515470667.87,
               "destroyedValue": 4722688524.39,
               "totalValue": 5238159192.26,
               "points": 1,
               "npc": false,
               "solo": false,
               "awox": false,
               "href": ""
             },
             "killmail": {
               "attackers": [],
               "killmail_id": 128389930,
               "killmail_time": "2025-07-06T23:32:26Z",
               "solar_system_id": 30002539,
               "victim": {
                 "alliance_id": 99009845,
                 "character_id": 2114058087,
                 "corporation_id": 98498670,
                 "damage_taken": 856144,
                 "items": [],
                 "position": {
                   "x": -30420382830.688633,
                   "y": 2662073916.025609,
                   "z": 309569446754.9493
                 },
                 "ship_type_id": 19720
               }
             }
         }"#;
        serde_json::from_str(json_data).expect("Failed to parse ZkData from JSON")
    }

    #[tokio::test]
    async fn test_ship_group_filter_uses_group_id_of_subscription_type_id_list() {
        let zk_data = user_killmail_data();
        let app_state = mock_app_state();

        let filter_node = FilterNode::And(vec![
            FilterNode::Condition(Filter::Simple(SimpleFilter::TotalValue {
                min: Some(5000000),
                max: None,
            })),
            FilterNode::Condition(Filter::Simple(SimpleFilter::Region(vec![10000030]))),
            // A list of type IDs of which we want to match based on their group ID
            FilterNode::Condition(Filter::Targeted(TargetedFilter {
                condition: TargetableCondition::ShipGroup(vec![
                    28352, 23919, 23757, 77283, 19722, 37604, 20183, 28850, 11567,
                ]),
                target: Default::default(),
            })),
            FilterNode::Condition(Filter::Simple(SimpleFilter::Security(
                "0.0001..=0.4999".to_string(),
            ))),
        ]);
        // Let's check each condition:
        // 1. TotalValue: 5.2b > 5m. PASS.
        // 2. Region: Siseide (30002539) is in Heimatar (10000030). PASS.
        // 3. Security: Siseide is 0.3, which is in the range 0.0001..=0.4999. PASS.
        // 4. ShipGroup: The victim's ship Revelation (type ID 19722) is the same group as a
        //               Naglfar (type ID 19720).
        //    The Naglfar's GROUP ID is 485 (Dreadnought).
        //    The filter list does NOT contain 485. It contains the TYPE ID 19720.
        //    Therefore, this condition must PASS.

        let result = evaluate_filter_node(&filter_node, &zk_data, &app_state).await;

        // Because one condition in the AND block fails, the entire node should fail.
        assert!(result.is_some(), "Filter should pass because the ShipGroup list contains a TypeID (19720), which has the required GroupID (485) to match the incoming killmail.");
    }

    // #[tokio::test]
    // async fn test_user_scenario_shipgroup_filter_passes_with_correct_group_id() {
    //     let zk_data = user_killmail_data();
    //     let app_state = mock_app_state();
    //
    //     // This filter is corrected to use the proper Ship Group ID for a Dreadnought (485).
    //     let filter_node = FilterNode::And(vec![
    //         FilterNode::Condition(Filter::TotalValue { min: Some(5000000), max: None }),
    //         FilterNode::Condition(Filter::Region(vec![10000030])),
    //         FilterNode::Condition(Filter::ShipGroup(vec![
    //             28352, 23919, 23757, 77283, 485, 37604, 20183, 28850, 11567, // Corrected: 19720 -> 485
    //         ])),
    //         FilterNode::Condition(Filter::Security("0.0001..=0.4999".to_string())),
    //     ]);
    //
    //     // Now, all four conditions should pass.
    //     let result = evaluate_filter_node(&filter_node, &zk_data, &app_state).await;
    //
    //     assert!(result.is_some(), "Filter should pass now that the correct ShipGroupID (485) is used");
    // }

    async fn test_filter(filter: Filter, zk_data: &ZkData, should_pass: bool) {
        let app_state = mock_app_state();
        let result = evaluate_filter(&filter, zk_data, &app_state).await;
        assert_eq!(
            result.is_some(),
            should_pass,
            "Filter test failed for: {:?}",
            filter
        );
    }

    #[tokio::test]
    async fn test_total_value_filter() {
        let zk_data = default_zk_data();
        test_filter(
            Filter::Simple(SimpleFilter::TotalValue {
                min: Some(5_000_000),
                max: None,
            }),
            &zk_data,
            true,
        )
        .await;
        test_filter(
            Filter::Simple(SimpleFilter::TotalValue {
                min: Some(15_000_000),
                max: None,
            }),
            &zk_data,
            false,
        )
        .await;
        test_filter(
            Filter::Simple(SimpleFilter::TotalValue {
                min: None,
                max: Some(15_000_000),
            }),
            &zk_data,
            true,
        )
        .await;
        test_filter(
            Filter::Simple(SimpleFilter::TotalValue {
                min: None,
                max: Some(5_000_000),
            }),
            &zk_data,
            false,
        )
        .await;
    }

    #[tokio::test]
    async fn test_region_filter() {
        let zk_data = default_zk_data();
        test_filter(
            Filter::Simple(SimpleFilter::Region(vec![10000002])),
            &zk_data,
            true,
        )
        .await; // The Forge
        test_filter(
            Filter::Simple(SimpleFilter::Region(vec![10000043])),
            &zk_data,
            false,
        )
        .await; // Domain
    }

    #[tokio::test]
    async fn test_system_filter() {
        let zk_data = default_zk_data();
        test_filter(
            Filter::Simple(SimpleFilter::System(vec![30000142])),
            &zk_data,
            true,
        )
        .await; // Jita
        test_filter(
            Filter::Simple(SimpleFilter::System(vec![31002222])),
            &zk_data,
            false,
        )
        .await; // Amarr
    }

    #[tokio::test]
    async fn test_security_filter() {
        let zk_data = default_zk_data(); // Jita is 0.9
        test_filter(
            Filter::Simple(SimpleFilter::Security("0.8..=1.0".to_string())),
            &zk_data,
            true,
        )
        .await;
        test_filter(
            Filter::Simple(SimpleFilter::Security("0.1..=0.5".to_string())),
            &zk_data,
            false,
        )
        .await;
    }

    #[tokio::test]
    async fn test_alliance_filter() {
        let zk_data = default_zk_data();
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::Alliance(vec![1001]),
            }),
            &zk_data,
            true,
        )
        .await; // Victim's alliance
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::Alliance(vec![1002]),
            }),
            &zk_data,
            true,
        )
        .await; // Attacker's alliance
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::Alliance(vec![9999]),
            }),
            &zk_data,
            false,
        )
        .await;
    }

    #[tokio::test]
    async fn test_corporation_filter() {
        let zk_data = default_zk_data();
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::Corporation(vec![101]),
            }),
            &zk_data,
            true,
        )
        .await; // Victim's corp
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::Corporation(vec![102]),
            }),
            &zk_data,
            true,
        )
        .await; // Attacker's corp
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::Corporation(vec![9999]),
            }),
            &zk_data,
            false,
        )
        .await;
    }

    #[tokio::test]
    async fn test_ship_type_filter() {
        let zk_data = default_zk_data();
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipType(vec![587]),
            }),
            &zk_data,
            true,
        )
        .await; // Victim's ship (Rifter)
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipType(vec![671]),
            }),
            &zk_data,
            true,
        )
        .await; // Attacker's ship (Catalyst)
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipType(vec![17738]),
            }),
            &zk_data,
            false,
        )
        .await; // Golem
    }

    #[tokio::test]
    async fn test_ship_group_filter() {
        let zk_data = default_zk_data();
        // Victim is a Rifter (Frigate, group 25)
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipGroup(vec![587]),
            }),
            &zk_data,
            true,
        )
        .await;
        // Attacker is a Catalyst (Destroyer, group 27)
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipGroup(vec![671]),
            }),
            &zk_data,
            true,
        )
        .await;
        // Neither is a Marauder (group 419)
        test_filter(
            Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipGroup(vec![28661]),
            }),
            &zk_data,
            false,
        )
        .await;
    }

    #[tokio::test]
    async fn test_ly_filter() {
        let mut zk_data = {
            ZkData {
                kill_id: 1,
                killmail: KillmailData {
                    killmail_id: 1,
                    killmail_time: "2025-07-08T12:00:00Z".to_string(),
                    solar_system_id: 30002086, // Turnur
                    victim: Victim {
                        damage_taken: 1000,
                        ship_type_id: 587, // Rifter
                        character_id: Some(1),
                        corporation_id: Some(101),
                        alliance_id: Some(1001),
                        position: Some(Position {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        }),
                        faction_id: None,
                        items: vec![],
                    },
                    attackers: vec![Attacker {
                        final_blow: true,
                        damage_done: 1000,
                        ship_type_id: Some(671), // Catalyst
                        character_id: Some(2),
                        corporation_id: Some(102),
                        alliance_id: Some(1002),
                        weapon_type_id: Some(3),
                        security_status: 0.5,
                        faction_id: None,
                    }],
                },
                zkb: Zkb {
                    total_value: 10_000_000.0,
                    dropped_value: 1_000_000.0,
                    npc: false,
                    solo: false,
                    location_id: None,
                    hash: "".to_string(),
                    fitted_value: 0.0,
                    destroyed_value: 0.0,
                    points: 0,
                    awox: false,
                    esi: "".to_string(),
                },
            }
        };
        zk_data.zkb.npc = true;
        test_filter(
            Filter::Simple(SimpleFilter::LyRangeFrom(vec![
                SystemRange {
                    system_id: 30002086,
                    range: 8.0,
                },
                SystemRange {
                    system_id: 30003067,
                    range: 8.0,
                },
            ])),
            &zk_data,
            true,
        )
        .await;
    }

    #[tokio::test]
    async fn test_is_npc_filter() {
        let mut zk_data = default_zk_data();
        zk_data.zkb.npc = true;
        test_filter(Filter::Simple(SimpleFilter::IsNpc(true)), &zk_data, true).await;
        test_filter(Filter::Simple(SimpleFilter::IsNpc(false)), &zk_data, false).await;
    }

    #[tokio::test]
    async fn test_pilots_filter() {
        let zk_data = default_zk_data(); // 2 pilots total
        test_filter(
            Filter::Simple(SimpleFilter::Pilots {
                min: Some(2),
                max: None,
            }),
            &zk_data,
            true,
        )
        .await;
        test_filter(
            Filter::Simple(SimpleFilter::Pilots {
                min: Some(3),
                max: None,
            }),
            &zk_data,
            false,
        )
        .await;
        test_filter(
            Filter::Simple(SimpleFilter::Pilots {
                min: None,
                max: Some(2),
            }),
            &zk_data,
            true,
        )
        .await;
        test_filter(
            Filter::Simple(SimpleFilter::Pilots {
                min: None,
                max: Some(1),
            }),
            &zk_data,
            false,
        )
        .await;
    }

    #[tokio::test]
    async fn test_time_range_filter() {
        let zk_data = default_zk_data(); // Time is 12:00:00
        test_filter(
            Filter::Simple(SimpleFilter::TimeRange { start: 11, end: 13 }),
            &zk_data,
            true,
        )
        .await;
        test_filter(
            Filter::Simple(SimpleFilter::TimeRange { start: 14, end: 16 }),
            &zk_data,
            false,
        )
        .await;
        // Test overnight range
        test_filter(
            Filter::Simple(SimpleFilter::TimeRange { start: 22, end: 4 }),
            &zk_data,
            false,
        )
        .await;
        let mut zk_data_night = default_zk_data();
        zk_data_night.killmail.killmail_time = "2025-07-08T23:00:00Z".to_string();
        test_filter(
            Filter::Simple(SimpleFilter::TimeRange { start: 22, end: 4 }),
            &zk_data_night,
            true,
        )
        .await;
    }

    #[tokio::test]
    async fn test_combined_and_filter_success() {
        // User's case: ShipGroup in a certain Region.
        // Test data: Rifter (Frigate, group 25) in Jita (The Forge, region 10000002)
        let zk_data = default_zk_data();
        let app_state = mock_app_state();

        let filter_node = FilterNode::And(vec![
            FilterNode::Condition(Filter::Simple(SimpleFilter::Region(vec![10000002]))), // The Forge
            FilterNode::Condition(Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipGroup(vec![587]), // group 25 for frigate
            })), // Frigate
        ]);

        let result = evaluate_filter_node(&filter_node, &zk_data, &app_state).await;
        assert!(
            result.is_some(),
            "Combined AND filter should pass when all conditions are met"
        );
    }

    #[tokio::test]
    async fn test_combined_and_filter_fail_region() {
        // Test data: Rifter (Frigate, group 25) in Jita
        // Filter: Wrong region, correct ship group
        let zk_data = default_zk_data();
        let app_state = mock_app_state();

        let filter_node = FilterNode::And(vec![
            FilterNode::Condition(Filter::Simple(SimpleFilter::Region(vec![10000043]))), // Domain (Wrong)
            FilterNode::Condition(Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipGroup(vec![25]),
            })), // Frigate (Correct)
        ]);

        let result = evaluate_filter_node(&filter_node, &zk_data, &app_state).await;
        assert!(
            result.is_none(),
            "Combined AND filter should fail when region is wrong"
        );
    }

    #[tokio::test]
    async fn test_combined_and_filter_fail_shipgroup() {
        // Test data: Rifter (Frigate, group 25) in Jita
        // Filter: Correct region, wrong ship group
        let zk_data = default_zk_data();
        let app_state = mock_app_state();

        let filter_node = FilterNode::And(vec![
            FilterNode::Condition(Filter::Simple(SimpleFilter::Region(vec![10000002]))), // The Forge (Correct)
            FilterNode::Condition(Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipGroup(vec![419]),
            })), // Marauder (Wrong)
        ]);

        let result = evaluate_filter_node(&filter_node, &zk_data, &app_state).await;
        assert!(
            result.is_none(),
            "Combined AND filter should fail when ship group is wrong"
        );
    }

    #[tokio::test]
    async fn test_combined_or_filter_success() {
        // Test data: Rifter (Frigate, group 25) in Jita
        // Filter: Wrong region OR correct ship group
        let zk_data = default_zk_data();
        let app_state = mock_app_state();

        let filter_node = FilterNode::Or(vec![
            FilterNode::Condition(Filter::Simple(SimpleFilter::Region(vec![10000043]))), // Domain (Wrong)
            FilterNode::Condition(Filter::Targeted(TargetedFilter {
                target: Target::Any,
                condition: TargetableCondition::ShipGroup(vec![587]), // group 25 for frigate
            })), // Frigate (Correct)
        ]);

        let result = evaluate_filter_node(&filter_node, &zk_data, &app_state).await;
        assert!(
            result.is_some(),
            "Combined OR filter should pass when one condition is met"
        );
    }

    #[tokio::test]
    async fn test_not_filter() {
        let zk_data = default_zk_data();
        let app_state = mock_app_state();

        // This filter should pass on its own
        let inner_filter =
            FilterNode::Condition(Filter::Simple(SimpleFilter::System(vec![30000142])));
        // So the NOT filter should fail
        let not_filter = FilterNode::Not(Box::new(inner_filter));

        let result = evaluate_filter_node(&not_filter, &zk_data, &app_state).await;
        assert!(
            result.is_none(),
            "NOT filter should fail when inner condition passes"
        );

        // This filter should fail on its own
        let inner_filter_fail =
            FilterNode::Condition(Filter::Simple(SimpleFilter::System(vec![999])));
        // So the NOT filter should pass
        let not_filter_pass = FilterNode::Not(Box::new(inner_filter_fail));

        let result_pass = evaluate_filter_node(&not_filter_pass, &zk_data, &app_state).await;
        assert!(
            result_pass.is_some(),
            "NOT filter should pass when inner condition fails"
        );
    }
}
