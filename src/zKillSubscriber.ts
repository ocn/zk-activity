import {
    Client,
    ColorResolvable,
    DiscordAPIError,
    MessageEmbed,
    MessageEmbedOptions,
    MessageOptions,
    TextChannel
} from 'discord.js';
import {MessageEvent, WebSocket} from 'ws';
import {REST} from '@discordjs/rest';
import AsyncLock from 'async-lock';
import MemoryCache from 'memory-cache';
import ogs from 'open-graph-scraper';
import {APIEmbed} from 'discord-api-types/v10';
import * as fs from 'fs';
import {EsiClient} from './lib/esiClient';

export enum SubscriptionType {
    PUBLIC = 'public',
}

export enum LimitType {
    REGION = 'region',
    CONSTELLATION = 'constellation',
    SYSTEM = 'system',
    SHIP_INCLUSION_TYPE_ID = 'type',
    SHIP_EXCLUSION_TYPE_ID = 'excludedType',
    SECURITY_MAX = 'securityMax',
    SECURITY_MIN = 'securityMin',
    ALLIANCE = 'alliance',
    CORPORATION = 'corporation',
    CHARACTER = 'character',
    // A partial name of the entity type to require for sending
    NAME_FRAGMENT = 'nameFragment',
    MIN_NUM_INVOLVED = 'minNumInvolved',
    TIME_RANGE_START = 'startingTime',
    TIME_RANGE_END = 'endingTime',
}

interface SubscriptionGuild {
    channels: Map<string, SubscriptionChannel>;
}

interface SubscriptionChannel {
    subscriptions: Map<string, Subscription>;
}

interface Subscription {
    subType: SubscriptionType
    id?: number
    minValue: number,
    // Mapping of LimitType to the value(s) to compare against
    limitTypes: Map<LimitType, string>,
    // If true, the limitTypes will be compared against the attacker's ship
    inclusionLimitAlsoComparesAttacker: boolean
    // If true, the limitTypes will be compared against the weapon type IDs on the attacker's ship
    // zKillboard will sometimes list weapon type IDs as the attacking ship, instead of the actual ship type ID
    inclusionLimitAlsoComparesAttackerWeapons: boolean
    // If true, the limitTypes will be compared against the attacker's ship
    exclusionLimitAlsoComparesAttacker: boolean
    // If true, the limitTypes will be compared against the weapon type IDs on the attacker's ship
    // zKillboard will sometimes list weapon type IDs as the attacking ship, instead of the actual ship type ID
    exclusionLimitAlsoComparesAttackerWeapons: boolean
}

class Attacker {
    alliance_id: number;
    corporation_id: number;
    damage_done: number;
    final_blow: boolean;
    security_status: number;
    ship_type_id?: number;
    weapon_type_id?: number;
    character_id?: number;

    constructor(
        alliance_id: number,
        corporation_id: number,
        damage_done: number,
        final_blow: boolean,
        security_status: number,
        weapon_type_id: number,
        ship_type_id?: number,
        character_id?: number
    ) {
        this.alliance_id = alliance_id;
        this.corporation_id = corporation_id;
        this.damage_done = damage_done;
        this.final_blow = final_blow;
        this.security_status = security_status;
        this.weapon_type_id = weapon_type_id;
        this.ship_type_id = ship_type_id;
        this.character_id = character_id;
    }
}

type Position = {
    x: number;
    y: number;
    z: number;
};

type Victim = {
    alliance_id: number;
    corporation_id: number;
    damage_taken: number;
    items: VictimItem[];
    position: Position;
    ship_type_id?: number; // ship_type_id is now optional
    character_id?: number; // character_id is optional and may be present instead of ship_type_id
};

type VictimItem = {
    item_type_id: number;
    singleton: number;
    flag: number;
    quantity_destroyed?: number;
    quantity_dropped?: number;
}

type Zkb = {
    locationID: number;
    hash: string;
    fittedValue: number;
    droppedValue: number;
    destroyedValue: number;
    totalValue: number;
    points: number;
    npc: boolean;
    solo: boolean;
    awox: boolean;
    esi: string;
    url: string;
};

type ZkData = {
    attackers: Attacker[];
    killmail_id: number;
    killmail_time: string;
    solar_system_id: number;
    victim: Victim;
    zkb: Zkb;
};

function hasLimitType(subscription: Subscription, limitType: LimitType): boolean {
    if (subscription.limitTypes instanceof Map) {
        return subscription.limitTypes.has(limitType);
    } else {
        console.log('subscription is not of type Map, exiting');
        console.log(`subscription.limitTypes: ${subscription.limitTypes}`);
        console.log(`subscription.limitTypes type: ${typeof subscription.limitTypes}`);
        process.exit(1);
    }
}

function getLimitType(subscription: Subscription, limitType: LimitType): string | undefined {
    if (subscription.limitTypes instanceof Map) {
        return subscription.limitTypes.get(limitType) as string | undefined;
    } else {
        console.log('subscription is not of type Map, exiting');
        console.log(`subscription.limitTypes: ${subscription.limitTypes}`);
        console.log(`subscription.limitTypes type: ${typeof subscription.limitTypes}`);
        process.exit(2);
        // Object.keys(subscription.limitTypes).forEach(key => {
        //     console.log(`key: ${key} limitType: ${limitType}`);
        //     if (key === limitType) {
        //         // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        //         // @ts-ignore
        //         console.log(`key: ${key} limitType: ${limitType} value: ${subscription.limitTypes[key]}`);
        //         // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        //         // @ts-ignore
        //         const ret = subscription.limitTypes[key] as string | undefined;
        //         console.log(`ret: ${ret}, typeof ret: ${typeof ret}`);
        //         return ret;
        //     }
        // });
        return undefined;
    }
}

export interface ClosestCelestial {
    distance: number;
    itemId: number;
    typeId: number;
    itemName: string;
}

export interface SolarSystem {
    id: number;
    systemName: string;
    regionId: number;
    regionName: string;
    constellationId: number;
    constellationName: string;
    securityStatus: number;
}

export class ZKillSubscriber {
    protected static instance: ZKillSubscriber;
    protected doClient: Client;

    protected subscriptions: Map<string, SubscriptionGuild>;
    // Mapping of a solar system type ID to a description
    protected systems: Map<number, SolarSystem>;
    // Mapping of ship type ID to group ID
    protected ships: Map<number, number>;
    // Mapping of ship type ID to name
    protected names: Map<number, string>;
    protected rest: REST;

    protected asyncLock: AsyncLock;
    protected esiClient: EsiClient;

    protected constructor(client: Client) {
        this.asyncLock = new AsyncLock();
        this.esiClient = new EsiClient();
        this.subscriptions = new Map<string, SubscriptionGuild>();
        this.systems = new Map<number, SolarSystem>();
        this.ships = new Map<number, number>();
        this.names = new Map<number, string>();
        this.doClient = client;
        this.rest = new REST({version: '9'}).setToken(process.env.DISCORD_BOT_TOKEN || '');
        ZKillSubscriber.connect(this);
    }

    protected static connect(sub: ZKillSubscriber) {
        const websocket = new WebSocket('wss://zkillboard.com/websocket/');
        websocket.onmessage = sub.onMessage.bind(sub);
        websocket.onopen = () => {
            websocket.send(JSON.stringify({
                'action': 'sub',
                'channel': 'killstream'
            }));
        };
        websocket.onclose = (e) => {
            console.log('Socket is closed. Reconnect will be attempted in 1 second.', e.reason);
            setTimeout(function () {
                ZKillSubscriber.connect(sub);
            }, 1000);
        };
        websocket.onerror = (error) => {
            console.error('Socket encountered error: ', error.message, 'Closing socket');
            websocket.close();
        };
    }

    protected async onMessage(event: MessageEvent) {
        const data: ZkData = JSON.parse(event.data.toString());
        this.subscriptions.forEach((guild, guildId) => {
            const log_prefix = `["${data.killmail_id}"][${new Date()}] `;
            console.log(log_prefix);
            guild.channels.forEach((channel, channelId) => {
                channel.subscriptions.forEach(async (subscription) => {
                    try {
                        await this.process_subscription(subscription, data, guildId, channelId);
                    } catch (e) {
                        console.log(e);
                    }
                });
            });
        });
    }

    private async process_subscription(
        subscription: Subscription,
        data: ZkData,
        guildId: string,
        channelId: string,
    ) {
        let color: ColorResolvable = 'GREEN';
        let requireSend = false;
        let matchedShipName: string | null = null;
        let matchedShipId: number | null = null;

        if (subscription.minValue > data.zkb.totalValue) {
            return; // Do not send if below the min value
        }

        if (subscription.limitTypes.size === 0) {
            await this.sendMessageToDiscord(guildId, channelId, subscription.subType, data);
            return;
        }
        if (hasLimitType(subscription, LimitType.SHIP_INCLUSION_TYPE_ID)) {
            let nameFragment = '';
            if (hasLimitType(subscription, LimitType.NAME_FRAGMENT)) {
                nameFragment = <string>getLimitType(subscription, LimitType.NAME_FRAGMENT);
            }
            const __ret = await this.sendIfAnyShipsMatchLimitFilter(
                data,
                <string>getLimitType(subscription, LimitType.SHIP_INCLUSION_TYPE_ID),
                nameFragment,
                subscription.inclusionLimitAlsoComparesAttacker,
                subscription.inclusionLimitAlsoComparesAttackerWeapons,
            );
            requireSend = __ret.requireSend;
            color = __ret.color;
            matchedShipName = __ret.matchedShipName;
            matchedShipId = __ret.matchedTypeId;
            if (!requireSend) return;
        }
        if (hasLimitType(subscription, LimitType.SECURITY_MAX)) {
            const systemData = await this.getSystemData(data.solar_system_id);
            const maximumSecurityStatus = Number(<string>getLimitType(subscription, LimitType.SECURITY_MAX));
            if (maximumSecurityStatus <= systemData.securityStatus) {
                console.log(`limiting kill due to maximum security status filter: ${systemData.securityStatus} >= ${maximumSecurityStatus}`);
                return;
            }
        }
        if (hasLimitType(subscription, LimitType.SECURITY_MIN)) {
            const systemData = await this.getSystemData(data.solar_system_id);
            const minimumSecurityStatus = Number(<string>getLimitType(subscription, LimitType.SECURITY_MIN));
            if (minimumSecurityStatus > systemData.securityStatus) {
                console.log(`limiting kill due to minimum security status filter: ${systemData.securityStatus} < ${minimumSecurityStatus}`);
                return;
            }
        }
        if (hasLimitType(subscription, LimitType.CHARACTER)) {
            const characterIds = <string>getLimitType(subscription, LimitType.CHARACTER);
            for (const characterId of characterIds.split(',')) {
                if (data.victim.character_id === Number(characterId)) {
                    requireSend = true;
                    color = 'RED';
                }
                if (!requireSend) {
                    for (const attacker of data.attackers) {
                        if (attacker.character_id === Number(characterId)) {
                            requireSend = true;
                            break;
                        }
                    }
                }
            }
            if (!requireSend) return;
        }
        if (hasLimitType(subscription, LimitType.CORPORATION)) {
            const corporationIds = <string>getLimitType(subscription, LimitType.CORPORATION);
            for (const corporationId of corporationIds.split(',')) {
                if (data.victim.corporation_id === Number(corporationId)) {
                    requireSend = true;
                    color = 'RED';
                }
                if (!requireSend) {
                    for (const attacker of data.attackers) {
                        if (attacker.corporation_id === Number(corporationId)) {
                            requireSend = true;
                            break;
                        }
                    }
                }
            }
            if (!requireSend) return;
        }
        if (hasLimitType(subscription, LimitType.ALLIANCE)) {
            const allianceIds = <string>getLimitType(subscription, LimitType.ALLIANCE);
            for (const allianceId of allianceIds.split(',')) {
                if (data.victim.alliance_id === Number(allianceId)) {
                    requireSend = true;
                    color = 'RED';
                }
                if (!requireSend) {
                    for (const attacker of data.attackers) {
                        if (attacker.alliance_id === Number(allianceId)) {
                            requireSend = true;
                            break;
                        }
                    }
                }
            }
            if (!requireSend) return;
        }
        if (hasLimitType(subscription, LimitType.REGION) ||
            hasLimitType(subscription, LimitType.CONSTELLATION) ||
            hasLimitType(subscription, LimitType.SYSTEM)) {
            requireSend = await this.isInLocationLimit(subscription, data.solar_system_id);
            if (!requireSend) return;
        }
        if (hasLimitType(subscription, LimitType.MIN_NUM_INVOLVED)) {
            const minNumInvolved = Number(<string>getLimitType(subscription, LimitType.MIN_NUM_INVOLVED));
            const numInvolved = data.attackers.length + 1;
            if (numInvolved < minNumInvolved) {
                console.log(`limiting kill due to minimum number of involved parties filter: ${numInvolved} < ${minNumInvolved}`);
                return;
            }
        }
        if (hasLimitType(subscription, LimitType.TIME_RANGE_START)) {
            const startTime = Number(<string>getLimitType(subscription, LimitType.TIME_RANGE_START));
            const killmailTime = new Date(data.killmail_time);
            if (killmailTime.getHours() < startTime) {
                console.log(`limiting kill due to time range start filter: ${killmailTime} < ${startTime}`);
                return;
            }
        }
        if (hasLimitType(subscription, LimitType.TIME_RANGE_END)) {
            const endTime = Number(<string>getLimitType(subscription, LimitType.TIME_RANGE_END));
            const killmailTime = new Date(data.killmail_time);
            if (killmailTime.getHours() > endTime) {
                console.log(`limiting kill due to time range end filter: ${killmailTime} > ${endTime}`);
                return;
            }
        }
        if (requireSend) {
            console.log('sending filtered kill');
            await this.sendMessageToDiscord(
                guildId,
                channelId,
                subscription.subType,
                data,
                subscription.id,
                matchedShipName,
                matchedShipId,
                color
            );
        }
    }

    // private async sendIfAnyShipsMatchLimitFilter(
    //     data: ZkData,
    //     limitIds: string,
    //     nameFragment: string,
    //     alsoCompareAttackers: boolean,
    //     alsoCompareAttackerWeapons: boolean
    // ) {
    //     let color: ColorResolvable = 'GREEN';
    //     let requireSend = false;
    //     let groupId: number | string | null = null;
    //     let matchedShipOrWeaponName: string | null = null;
    //     const shouldCheckNameFragment = nameFragment != null && nameFragment != '';
    //
    //     const limitGroupOfShipIds = limitIds?.split(',') || [];
    //     let victimShipNameByTypeId = '';
    //     for (const permittedGroupOfShipIds of limitGroupOfShipIds) {
    //         const permittedGroupOfShipId = await this.getGroupIdForEntityId(Number(permittedGroupOfShipIds));
    //
    //         // Determine if the victim has a matching ship type.
    //         const shipTypeId = data.victim.ship_type_id;
    //         if (shipTypeId != null) {
    //             groupId = await this.getGroupIdForEntityId(shipTypeId);
    //             if (shouldCheckNameFragment) {
    //                 victimShipNameByTypeId = await this.getNameForEntityId(shipTypeId);
    //                 if (victimShipNameByTypeId.includes(nameFragment)) {
    //                     console.log('victim ship name: ' + victimShipNameByTypeId);
    //                     matchedShipOrWeaponName = victimShipNameByTypeId;
    //                     requireSend = true;
    //                     break;
    //                 } else {
    //                     // console.log('victim ship name: ' + victimShipNameByTypeId + ' does not contain ' + nameFragment);
    //                     continue;
    //                 }
    //             }
    //         } else if (!alsoCompareAttackers) {
    //             break;
    //         }
    //         if (groupId === permittedGroupOfShipId) {
    //             requireSend = true;
    //             color = 'RED';
    //             break;
    //         }
    //
    //         // Victim is not permitted ship type. Check attackers for any matching.
    //         let attackerShipNameByTypeId = '';
    //         if (!requireSend && alsoCompareAttackers) {
    //             for (const attacker of data.attackers) {
    //                 if (attacker.ship_type_id) {
    //                     groupId = await this.getGroupIdForEntityId(attacker.ship_type_id);
    //                     if (groupId === permittedGroupOfShipId) {
    //                         if (shouldCheckNameFragment) {
    //                             attackerShipNameByTypeId = await this.getNameForEntityId(attacker.ship_type_id);
    //                             if (attackerShipNameByTypeId.includes(nameFragment)) {
    //                                 console.log('attacker ship name: ' + attackerShipNameByTypeId);
    //                                 requireSend = true;
    //                                 break;
    //                             } else {
    //                                 // console.log('attacker ship name: ' + attackerShipNameByTypeId + ' does not contain ' + nameFragment);
    //                                 continue;
    //                             }
    //                         }
    //                         console.log('attacker ship groupID: ' + groupId);
    //                         requireSend = true;
    //                         break;
    //                     }
    //                 }
    //                 if (alsoCompareAttackerWeapons && attacker.weapon_type_id) {
    //                     groupId = await this.getGroupIdForEntityId(attacker.weapon_type_id);
    //                     if (groupId === permittedGroupOfShipId) {
    //                         if (shouldCheckNameFragment) {
    //                             attackerShipNameByTypeId = await this.getNameForEntityId(attacker.weapon_type_id);
    //                             if (attackerShipNameByTypeId.includes(nameFragment)) {
    //                                 console.log('attacker weapon name: ' + attackerShipNameByTypeId);
    //                                 requireSend = true;
    //                                 break;
    //                             } else {
    //                                 // console.log('attacker weapon name: ' + attackerShipNameByTypeId + ' does not contain ' + nameFragment);
    //                                 continue;
    //                             }
    //                         }
    //                         console.log('attacker weapon groupId: ' + groupId);
    //                         requireSend = true;
    //                         break;
    //                     }
    //                 }
    //             }
    //         }
    //
    //         if (requireSend) {
    //             break;
    //         }
    //     }
    //
    //     return {requireSend, color, matched_type_id: matchedShipOrWeaponName};
    // }
    private async sendIfAnyShipsMatchLimitFilter(
        data: ZkData,
        limitIds: string,
        nameFragment: string,
        alsoCompareAttackers: boolean,
        alsoCompareAttackerWeapons: boolean
    ) {
        const limitGroupOfShipIds = limitIds?.split(',') || [];
        const shouldCheckNameFragment = nameFragment != null && nameFragment != '';
        const shipTypeId = data.victim.ship_type_id;
        if (shipTypeId == null) {
            console.log('WARNING: shipTypeId is null');
            return {
                requireSend: false,
                color: <ColorResolvable>'GREEN',
                matchedShipName: null,
                matchedTypeId: null,
            };
        }

        for (const permittedGroupOfShipIds of limitGroupOfShipIds) {
            const permittedGroupOfShipId = await this.getGroupIdForEntityId(Number(permittedGroupOfShipIds));

            // Check if the victim's ship matches the criteria
            if (await this.isShipMatch(shipTypeId, permittedGroupOfShipId, shouldCheckNameFragment, nameFragment)) {
                return {
                    requireSend: true,
                    color: <ColorResolvable>'RED',
                    matchedShipName: await this.getNameForEntityId(shipTypeId),
                    matchedTypeId: shipTypeId,
                };
            }

            // If the victim's ship doesn't match, check the attackers' ships
            if (alsoCompareAttackers) {
                for (const attacker of data.attackers) {
                    if (await this.isShipMatch(attacker.ship_type_id, permittedGroupOfShipId, shouldCheckNameFragment, nameFragment)) {
                        const id = attacker.ship_type_id;
                        if (id == null) {
                            console.log('WARNING: attacker.ship_type_id is null but matched?');
                            continue;
                        }
                        return {
                            requireSend: true,
                            color: <ColorResolvable>'GREEN',
                            matchedShipName: await this.getNameForEntityId(id),
                            matchedTypeId: id,
                        };
                    }
                    if ((alsoCompareAttackerWeapons && await this.isShipMatch(attacker.weapon_type_id, permittedGroupOfShipId, shouldCheckNameFragment, nameFragment))) {
                        const id = attacker.weapon_type_id;
                        if (id == null) {
                            console.log('WARNING: attacker.weapon_type_id is null but matched?');
                            continue;
                        }
                        return {
                            requireSend: true,
                            color: <ColorResolvable>'GREEN',
                            matchedShipName: await this.getNameForEntityId(id),
                            matchedTypeId: id,
                        };
                    }
                }
            }
        }

        return {
            requireSend: false,
            color: <ColorResolvable>'RED',
            matchedShipName: null,
            matchedTypeId: null,
        };
    }

    private async isShipMatch(shipTypeId: number | undefined, permittedGroupOfShipId: number, shouldCheckNameFragment: boolean, nameFragment: string) {
        if (shipTypeId != null) {
            const groupId = await this.getGroupIdForEntityId(shipTypeId);
            if (groupId === permittedGroupOfShipId) {
                if (shouldCheckNameFragment) {
                    const shipName = await this.getNameForEntityId(shipTypeId);
                    return shipName.includes(nameFragment);
                }
                return true;
            }
        }
        return false;
    }

    public async sendMessageToDiscord(
        guildId: string,
        channelId: string,
        subType: SubscriptionType,
        data: ZkData,
        subId?: number,
        matchedShipName: string | null = null,
        matchedShipId: number | null = null,
        messageColor: ColorResolvable = 'GREY',
    ) {
        await this.asyncLock.acquire('sendKill', async (done) => {
            const cacheKey = `${channelId}_${data.killmail_id}`;
            if (MemoryCache.get(cacheKey)) {
                // Mail was already sent, prevent from sending twice
                done();
                return;
            }

            const channel = <TextChannel>this.doClient.channels.cache.get(channelId);
            if (!channel) {
                await this.unsubscribe(subType, guildId, channelId, subId);
                done();
                return;
            }

            const embedding = await ogs({url: data.zkb.url}).catch(() => null);
            const content: MessageOptions = await this.prepareMessageContent(embedding, data, matchedShipName, matchedShipId, messageColor);

            try {
                await channel.send(content);
                MemoryCache.put(cacheKey, 'send', 60000); // Prevent from sending again, cache it for 1 min
            } catch (e) {
                if (e instanceof DiscordAPIError && e.httpStatus === 403) {
                    await this.handlePermissionError(channel);
                } else {
                    console.log(e);
                }
            }
            done();
        });
    }

    private async prepareMessageContent(embedding: any, data: ZkData, matchedShipName: string | null, matchedShipId: number | null, messageColor: ColorResolvable): Promise<MessageOptions> {
        if (embedding?.error === false) {
            if (matchedShipName != null && matchedShipId != null) {
                // Use a custom embed format that highlights the specific ship detected
                return {
                    embeds: await this.prepareEmbedFields(embedding, data, matchedShipName, matchedShipId, messageColor)
                };
            } else {
                // Default to the standard zkill embedding
                return {
                    embeds: [{
                        title: embedding?.result.ogTitle,
                        description: embedding?.result.ogDescription,
                        thumbnail: {
                            // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                            // @ts-ignore
                            url: embedding?.result.ogImage?.url,
                            // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                            // @ts-ignore
                            height: embedding?.result.ogImage?.height,
                            // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                            // @ts-ignore
                            width: embedding?.result.ogImage?.width
                        },
                        url: data.zkb.url,
                        color: messageColor,
                    }]
                };
            }
        } else {
            return {content: data.zkb.url};
        }
    }

    private async prepareEmbedFields(embedding: any, data: ZkData, matchedShipName: string, matchedShipId: number, messageColor: ColorResolvable): Promise<(MessageEmbed | MessageEmbedOptions | APIEmbed)[]> {
        console.log('prepareEmbedFields');
        const systemRegion = await this.getSystemData(data.solar_system_id);
        let victimDetails = '';
        let attackerDetails = '';
        let killmailDetails = '';


        if (data.victim.ship_type_id != null) {
            try {
                const victimShipName = await this.getNameForEntityId(data.victim.ship_type_id);
                victimDetails += `Ship: [${victimShipName}](${this.strShipZk(data.victim.ship_type_id)})\n`;
            } catch (e) {
                console.log(e);
            }
        }
        if (data.victim.character_id != null) {
            try {
                const victimCharacterName = await this.getNameForCharacter(data.victim.character_id);
                victimDetails += `Pilot: [${victimCharacterName}](${this.strPilotZk(data.victim.character_id)})\n`;
            } catch (e) {
                console.log(e);
            }
        }
        if (data.victim.corporation_id != null) {
            try {
                const victimCorporationName = await this.getNameForCorporation(data.victim.corporation_id);
                victimDetails += `Corp: [${victimCorporationName}](${this.strCorpZk(data.victim.corporation_id)})\n`;
            } catch (e) {
                console.log(e);
            }
        }
        if (data.victim.alliance_id != null) {
            try {
                const victimAllianceName = await this.getNameForAlliance(data.victim.alliance_id);
                victimDetails += `Alliance: [${victimAllianceName}](${this.strAllianceZk(data.victim.alliance_id)})\n`;
            } catch (e) {
                console.log(e);
            }
        }
        console.log('victimDataDone');


        console.log('attackerData');
        let lastHitAttacker = null;
        for (const attacker of data.attackers) {
            if (attacker.final_blow) {
                lastHitAttacker = attacker;
                break;
            }
        }
        if (lastHitAttacker == null) {
            console.log('No final blow attacker found, using first attacker as last hit attacker');
            lastHitAttacker = data.attackers[0];
        }
        if (lastHitAttacker.ship_type_id != null) {
            try {
                const attackerShipName = await this.getNameForEntityId(lastHitAttacker.ship_type_id);
                attackerDetails += `Ship: [${attackerShipName}](${this.strShipZk(lastHitAttacker.ship_type_id)})\n`;
            } catch (e) {
                console.log(e);
            }
        }
        if (lastHitAttacker.character_id != null) {
            try {
                const attackerCharacterName = await this.getNameForCharacter(lastHitAttacker.character_id);
                attackerDetails += `Pilot: [${attackerCharacterName}](${this.strPilotZk(lastHitAttacker.character_id)})\n`;
            } catch (e) {
                console.log(e);
            }
        }
        if (lastHitAttacker.corporation_id != null) {
            try {
                const attackerCorporationName = await this.getNameForCorporation(lastHitAttacker.corporation_id);
                attackerDetails += `Corp: [${attackerCorporationName}](${this.strCorpZk(lastHitAttacker.corporation_id)})\n`;
            } catch (e) {
                console.log(e);
            }
        }
        if (lastHitAttacker.alliance_id != null) {
            try {
                const attackerAllianceName = await this.getNameForAlliance(lastHitAttacker.alliance_id);
                attackerDetails += `Alliance: [${attackerAllianceName}](${this.strAllianceZk(lastHitAttacker.alliance_id)})\n`;
            } catch (e) {
                console.log(e);
            }
        }

        let affiliation = '```';
        const allianceCountMap = new Map<string, number>();
        for (const attacker of data.attackers) {
            const id = attacker.alliance_id ? attacker.alliance_id : attacker.corporation_id;
            let name = '';
            if (attacker.alliance_id) {
                try {
                    name = await this.getNameForAlliance(id);
                } catch (e) {
                    console.log(`Error getting alliance name for id ${id}: ${e}`);
                    name = 'Unknown';
                }
            } else {
                try {
                    name = await this.getNameForCorporation(id);
                } catch (e) {
                    console.log(`Error getting corporation name for id ${id}: ${e}`);
                    name = 'Unknown';
                }
            }
            if (allianceCountMap.has(name)) {
                const value = allianceCountMap.get(name);
                if (value == null) {
                    continue;
                }
                allianceCountMap.set(name, value + 1);
            } else {
                allianceCountMap.set(name, 1);
            }
        }
        let maxNameLength = 0;
        Array.from(allianceCountMap.keys()).forEach((name: string) => {
            if (name.length > maxNameLength) {
                maxNameLength = name.length;
            }
        });
        console.log('maxNameLength: ' + maxNameLength);
        const sortedEntries = Array.from(allianceCountMap.entries()).sort((a, b) => b[1] - a[1]);
        const padding = 5;
        let othersCount = 0;
        let firstEntry = true;
        for (const [key, value] of sortedEntries) {
            if (value > 1 || firstEntry) {
                const spaces = maxNameLength - key.length + padding;
                affiliation += `${key}${' '.repeat(spaces)}x${value}\n`;
                firstEntry = false;
            } else {
                othersCount += value;
            }
        }
        if (othersCount > 0) {
            const others = '...others';
            const spaces = maxNameLength - others.length + padding;
            affiliation += `${others}${' '.repeat(spaces)}x${othersCount}\n`;
        }
        affiliation += '```';
        console.log('attackerDataDone');


        console.log(systemRegion);
        killmailDetails += `System: [${systemRegion.systemName}](${this.strSystemDotlan(systemRegion.id)}) ([${systemRegion.regionName}](${this.strRegionDotlan(systemRegion.regionId)}))\n`;
        const closestCelestial = await this.getClosestCelestial(systemRegion.id, data.victim.position.x, data.victim.position.y, data.victim.position.z);
        const distanceInKm = (closestCelestial.distance / 1000).toFixed(2);
        killmailDetails += `Celestial: [${closestCelestial.itemName}](${this.strLocation(closestCelestial.itemId)}) (${distanceInKm} km)\n`;
        // convert data.killmail_time into a relative time
        const killmailTime = new Date(data.killmail_time);
        const now = new Date();
        const diff = now.getTime() - killmailTime.getTime();
        const seconds = Math.floor(diff / 1000);
        const minutes = Math.floor(seconds / 60);
        const hours = Math.floor(minutes / 60);
        const days = Math.floor(hours / 24);
        const weeks = Math.floor(days / 7);
        const months = Math.floor(weeks / 4);
        const years = Math.floor(months / 12);
        let relativeTime = '';
        if (years > 1) {
            relativeTime = years + ' years ago';
        } else if (years === 1) {
            relativeTime = '1 year ago';
        } else if (months > 1) {
            relativeTime = months + ' months ago';
        } else if (months === 1) {
            relativeTime = '1 month ago';
        } else if (weeks > 1) {
            relativeTime = weeks + ' weeks ago';
        } else if (weeks === 1) {
            relativeTime = '1 week ago';
        } else if (days > 1) {
            relativeTime = days + ' days ago';
        } else if (days === 1) {
            relativeTime = '1 day ago';
        } else if (hours > 1) {
            relativeTime = hours + ' hours ago';
        } else if (hours === 1) {
            relativeTime = '1 hour ago';
        } else if (minutes > 1) {
            relativeTime = minutes + ' minutes ago';
        } else if (minutes === 1) {
            relativeTime = '1 minute ago';
        } else if (seconds > 1) {
            relativeTime = seconds + ' seconds ago';
        } else {
            relativeTime = '1 second ago';
        }
        // convert the killmail_time `2023-01-17T01:53:02Z` to YYYY/MM/DD HH:MM
        const killmailTimeFormatted = killmailTime.toISOString().replace(/T/, ' ').replace(/\..+/, '');

        const fields: { inline: boolean; name: string; value: string }[] = [];
        [
            {
                name: `__Attackers__ - ${data.attackers.length} `,
                value: affiliation,
                inline: false,
            },
            {
                name: '__Victim__',
                value: victimDetails,
                inline: true
            },
            {
                name: '__Attacker__',
                value: attackerDetails,
                inline: true
            },
            {
                name: '__Details__',
                value: killmailDetails,
                inline: false
            },
        ].forEach((field) => fields.push(field));
        return [{
            title: `\`${matchedShipName}\` activity in ${systemRegion.systemName} (${systemRegion.regionName})`,
            thumbnail: {
                url: this.strItemRenderById(matchedShipId),
                height: embedding?.result.ogImage?.height,
                width: embedding?.result.ogImage?.width
            },
            url: data.zkb.url,
            color: messageColor,
            fields: fields,
            footer: {
                text: `${killmailTimeFormatted} - ${relativeTime}`
            }
        }];
    }

    private async handlePermissionError(channel: TextChannel) {
        const owner = await channel.guild.fetchOwner();
        await owner.send(`The bot unsubscribed from channel ${channel.name} on ${channel.guild.name} because it was not able to write in it! Fix the permissions and subscribe again!`);
        const subscriptionsInChannel = this.subscriptions.get(channel.guild.id)?.channels.get(channel.id);
        if (subscriptionsInChannel) {
            subscriptionsInChannel.subscriptions.forEach((subscription) => {
                this.unsubscribe(subscription.subType, channel.guild.id, channel.id, subscription.id);
            });
        }
    }

    public static getInstance(client?: Client) {
        if (!this.instance && client)
            this.instance = new ZKillSubscriber(client);
        else if (!this.instance) {
            throw new Error('Instance needs to be created with a client once.');
        }

        return this.instance;
    }

    public subscribe(
        subType: SubscriptionType,
        guildId: string,
        channel: string,
        limitTypes: Map<LimitType, string>,
        inclusionLimitAlsoComparesAttacker: boolean,
        inclusionLimitAlsoComparesAttackerWeapons: boolean,
        exclusionLimitAlsoComparesAttacker: boolean,
        exclusionLimitAlsoComparesAttackerWeapons: boolean,
        id?: number,
        minValue = 0,
    ) {
        if (!this.subscriptions.has(guildId)) {
            this.subscriptions.set(guildId, {channels: new Map<string, SubscriptionChannel>()});
        }
        const guild = this.subscriptions.get(guildId);
        if (!guild?.channels.has(channel)) {
            guild?.channels.set(channel, {subscriptions: new Map<string, Subscription>()});
        }
        const guildChannel = guild?.channels.get(channel);
        const ident = `${subType}${id ? id : ''}`;
        if (!guildChannel?.subscriptions.has(ident)) {
            guildChannel?.subscriptions.set(ident, {
                subType,
                id,
                minValue,
                limitTypes,
                inclusionLimitAlsoComparesAttacker,
                inclusionLimitAlsoComparesAttackerWeapons,
                exclusionLimitAlsoComparesAttacker,
                exclusionLimitAlsoComparesAttackerWeapons,
            });
        }
        fs.writeFileSync('./config/' + guildId + '.json', JSON.stringify(this.generateObject(guild)), 'utf8');
    }

    public async unsubscribe(subType: SubscriptionType, guildId: string, channel: string, id?: number) {
        if (!this.subscriptions.has(guildId)) {
            return;
        }
        const guild = this.subscriptions.get(guildId);
        if (!guild?.channels.has(channel)) {
            return;
        }
        const guildChannel = guild.channels.get(channel);
        const ident = `${subType}${id ? id : ''}`;
        if (!guildChannel?.subscriptions.has(ident)) {
            return;
        }
        guildChannel.subscriptions.delete(ident);
        fs.writeFileSync('./config/' + guildId + '.json', JSON.stringify(this.generateObject(guild)), 'utf8');
    }

    public async unsubscribeGuild(guildId: string) {
        if (this.subscriptions.has(guildId)) {
            this.subscriptions.delete(guildId);
            fs.unlinkSync('./config/' + guildId + '.json');
            return;
        }
    }

    public async listGuildChannelSubscriptions(guildId: string, channel: string) {
        if (this.subscriptions.has(guildId)) {
            const guild = this.subscriptions.get(guildId);
            if (guild?.channels.has(channel)) {
                return guild.channels.get(channel);
            }
        }
    }

    private generateObject(object: any): any {
        const keys = Object.keys(object);
        const newObject: any = {};
        for (const key of keys) {
            if (object[key] instanceof Map) {
                newObject[key] = this.generateObject(Object.fromEntries(object[key]));
            } else if (Array.isArray(object[key])) {
                newObject[key] = this.generateObject(object[key]);
            } else if (typeof object[key] === 'object') {
                newObject[key] = this.generateObject(object[key]);
            } else {
                newObject[key] = object[key];
            }
        }
        return newObject;
    }

    private createChannelMap(object: any): Map<string, SubscriptionChannel> {
        const map = new Map<string, SubscriptionChannel>();
        const keys = Object.keys(object);
        for (const key of keys) {
            map.set(key, {subscriptions: this.createSubscriptionMap(object[key].subscriptions)});
        }
        return map;
    }

    private createSubscriptionMap(object: any): Map<string, Subscription> {
        console.log('Creating subscription map');
        const map = new Map<string, Subscription>();
        const keys = Object.keys(object);
        for (const key of keys) {
            console.log('Creating subscription for ' + key);
            if (object[key].limitTypes === undefined) {
                object[key].limitTypes = new Map<LimitType, string>;
            }
            if (object[key].limitTypes instanceof Object) {
                console.log('Converting limitTypes from Object to Map');
                const properties = Object.entries(object[key].limitTypes);
                object[key].limitTypes = new Map(properties);
                console.log('LimitTypes = ' + object[key].limitTypes);
            }
            map.set(key, object[key]);
        }
        return map;
    }

    private async getSystemData(systemId: number): Promise<SolarSystem> {
        return await this.asyncLock.acquire('fetchSystem', async (done) => {

            let system = this.systems.get(systemId);
            if (system) {
                done(undefined, system);
                return;
            }
            console.log('found undefined system with id ' + systemId);
            system = await this.esiClient.getSystemInfo(systemId);
            this.systems.set(systemId, system);
            fs.writeFileSync('./config/systems.json', JSON.stringify(Object.fromEntries(this.systems)), 'utf8');

            done(undefined, system);
        });
    }

    private async isInLocationLimit(subscription: Subscription, solar_system_id: number) {
        const systemData = await this.getSystemData(solar_system_id);
        if (hasLimitType(subscription, LimitType.SYSTEM) &&
            (getLimitType(subscription, LimitType.SYSTEM)?.split(',') || []).indexOf(systemData.id.toString()) !== -1) {
            return true;
        }
        if (subscription.limitTypes.has(LimitType.CONSTELLATION) &&
            (getLimitType(subscription, LimitType.CONSTELLATION)?.split(',') || []).indexOf(systemData.constellationId.toString()) !== -1) {
            return true;
        }
        if (subscription.limitTypes.has(LimitType.REGION) &&
            (getLimitType(subscription, LimitType.REGION)?.split(',') || []).indexOf(systemData.regionId.toString()) !== -1) {
            return true;
        }
        return false;
    }

    private async getGroupIdForEntityId(shipId: number): Promise<number> {
        return await this.asyncLock.acquire('fetchShip', async (done) => {

            let group = this.ships.get(shipId);
            if (group) {
                done(undefined, group);
                return;
            }
            group = await this.esiClient.getTypeGroupId(shipId);
            this.ships.set(shipId, group);
            fs.writeFileSync('./config/ships.json', JSON.stringify(Object.fromEntries(this.ships)), 'utf8');

            done(undefined, group);
        });
    }

    private async getNameForEntityId(shipId: number): Promise<string> {
        return await this.asyncLock.acquire('fetchName', async (done) => {

            let name = this.names.get(shipId);
            if (name) {
                done(undefined, name);
                return;
            }
            name = await this.esiClient.getTypeName(shipId);
            this.names.set(shipId, name);
            fs.writeFileSync('./config/names.json', JSON.stringify(Object.fromEntries(this.names)), 'utf8');

            done(undefined, name);
        });
    }

    private async getNameForAlliance(allianceId: number): Promise<string> {
        return await this.asyncLock.acquire('fetchName', async (done) => {

            let name = this.names.get(allianceId);
            if (name) {
                done(undefined, name);
                return;
            }
            name = await this.esiClient.getAllianceName(allianceId);
            this.names.set(allianceId, name);
            fs.writeFileSync('./config/names.json', JSON.stringify(Object.fromEntries(this.names)), 'utf8');

            done(undefined, name);
        });
    }

    private async getNameForCorporation(corporationId: number): Promise<string> {
        return await this.asyncLock.acquire('fetchName', async (done) => {

            let name = this.names.get(corporationId);
            if (name) {
                done(undefined, name);
                return;
            }
            name = await this.esiClient.getCorporationName(corporationId);
            this.names.set(corporationId, name);
            fs.writeFileSync('./config/names.json', JSON.stringify(Object.fromEntries(this.names)), 'utf8');

            done(undefined, name);
        });
    }

    private async getNameForCharacter(characterId: number): Promise<string> {
        return await this.asyncLock.acquire('fetchName', async (done) => {

            let name = this.names.get(characterId);
            if (name) {
                done(undefined, name);
                return;
            }
            name = await this.esiClient.getCharacterName(characterId);
            this.names.set(characterId, name);
            fs.writeFileSync('./config/names.json', JSON.stringify(Object.fromEntries(this.names)), 'utf8');

            done(undefined, name);
        });
    }

    private async getClosestCelestial(systemId: number, x: number, y: number, z: number): Promise<ClosestCelestial> {
        return await this.esiClient.getCelestial(systemId, x, y, z);
    }

    public withConfig(base_dir = './config/'): ZKillSubscriber {
        const files = fs.readdirSync(base_dir, {withFileTypes: true});
        for (const file of files) {
            if (file.name.match(/\d+\.json$/)) {
                const guildId = file.name.match(/(\d*)\.json$/);
                if (guildId && guildId.length > 0 && guildId[0]) {
                    const fileContent = fs.readFileSync(base_dir + file.name, 'utf8');
                    const parsedFileContent = JSON.parse(fileContent);
                    this.subscriptions.set(guildId[1], {channels: this.createChannelMap(parsedFileContent.channels)});
                }
            }
        }
        return this;
    }

    public withSystems(base_dir = './config/'): ZKillSubscriber {
        if (fs.existsSync(base_dir + 'systems.json')) {
            const fileContent = fs.readFileSync(base_dir + 'systems.json', 'utf8');
            try {
                const data = JSON.parse(fileContent);
                for (const key in data) {
                    this.systems.set(Number.parseInt(key), data[key] as SolarSystem);
                }
            } catch (e) {
                console.log('failed to parse systems.json');
            }
        }
        return this;
    }

    public withShips(base_dir = './config/'): ZKillSubscriber {
        if (fs.existsSync(base_dir + 'ships.json')) {
            const fileContent = fs.readFileSync(base_dir + 'ships.json', 'utf8');
            try {
                const data = JSON.parse(fileContent);
                for (const key in data) {
                    this.ships.set(Number.parseInt(key), data[key]);
                }
            } catch (e) {
                console.log('failed to parse ships.json');
            }
        }
        return this;
    }

    public withNames(base_dir = './config/'): ZKillSubscriber {
        if (fs.existsSync(base_dir + 'names.json')) {
            const fileContent = fs.readFileSync(base_dir + 'names.json', 'utf8');
            try {
                const data = JSON.parse(fileContent);
                for (const key in data) {
                    this.names.set(Number.parseInt(key), data[key]);
                }
            } catch (e) {
                console.log('failed to parse names.json');
            }
        }
        return this;
    }

    strPilotZk(characterId: number): string {
        try {
            return `https://zkillboard.com/character/${characterId.toString()}/`;
        } catch {
            return '';
        }
    }

    strCorpZk(corporationId: number): string {
        try {
            return `https://zkillboard.com/corporation/${corporationId.toString()}/`;
        } catch {
            return '';
        }
    }

    strAllianceZk(allianceId: number): string {
        try {
            return `https://zkillboard.com/alliance/${allianceId.toString()}/`;
        } catch {
            return '';
        }
    }

    strShipZk(shipTypeId: number): string {
        try {
            return `https://zkillboard.com/ship/${shipTypeId.toString()}/`;
        } catch {
            return '';
        }
    }

    strLocation(locationId: number): string {
        try {
            return `https://zkillboard.com/location/${locationId.toString()}/`;
        } catch {
            return '';
        }
    }

    strSystemDotlan(systemId: number): string {
        try {
            return `http://evemaps.dotlan.net/system/${systemId.toString()}`;
        } catch {
            return '';
        }
    }

    strRegionDotlan(regionId: number): string {
        try {
            return `http://evemaps.dotlan.net/region/${regionId.toString()}`;
        } catch {
            return '';
        }
    }

    strItemRenderById(itemId: number): string {
        try {
            return `https://images.evetech.net/types/${itemId.toString()}/render`;
        } catch {
            return '';
        }
    }
}

