import {Client, Intents} from 'discord.js';
import {SubscriptionType, ZKillSubscriber} from '../zKillSubscriber';

jest.setTimeout(30000);

describe('ZK Subscriber', () => {
    it('should send message to discord', async () => {
        const client = new Client({intents: [Intents.FLAGS.GUILDS]});
        await client.login(process.env.DISCORD_BOT_TOKEN);
        console.log('logged in');
        const sub = ZKillSubscriber.getInstance(client)
            .withSystems('../config/')
            .withShips('../config/')
            .withNames('../config/');
        client.once('ready', async () => {
            console.log('ready');
            const zk_data = {
                'attackers': [
                    {
                        'alliance_id': 1411711376,
                        'character_id': 2117381670,
                        'corporation_id': 98462540,
                        'damage_done': 17841,
                        'final_blow': true,
                        'security_status': 5,
                        'ship_type_id': 11377,
                        'weapon_type_id': 27351
                    },
                    {
                        'alliance_id': 99011181,
                        'character_id': 2114153944,
                        'corporation_id': 98678227,
                        'damage_done': 0,
                        'final_blow': false,
                        'security_status': 0.7,
                        'ship_type_id': 621,
                        'weapon_type_id': 1877
                    },
                    {
                        'alliance_id': 1411711376,
                        'character_id': 2118301372,
                        'corporation_id': 98382886,
                        'damage_done': 0,
                        'final_blow': false,
                        'security_status': 5,
                        'ship_type_id': 28352,
                        'weapon_type_id': 8105
                    },
                    {
                        'alliance_id': 99011260,
                        'character_id': 2113631998,
                        'corporation_id': 98651532,
                        'damage_done': 0,
                        'final_blow': false,
                        'security_status': 5,
                        'ship_type_id': 670,
                        'weapon_type_id': 2420
                    }
                ],
                'killmail_id': 106140056,
                'killmail_time': '2023-01-17T01:53:02Z',
                'solar_system_id': 30000594,
                'victim': {
                    'alliance_id': 99005338,
                    'corporation_id': 98636576,
                    'damage_taken': 17841,
                    'items': [],
                    'position': {
                        'x': -81529320662.88177,
                        'y': 14731476610.582806,
                        'z': -180418401167.63428
                    },
                    'ship_type_id': 26892
                },
                'zkb': {
                    'locationID': 50002711,
                    'hash': 'e6ad2b2b40536d05ad96428e24795af21ce9a9d7',
                    'fittedValue': 3616323.29,
                    'droppedValue': 0,
                    'destroyedValue': 3616323.29,
                    'totalValue': 3616323.29,
                    'points': 1,
                    'npc': false,
                    'solo': false,
                    'awox': false,
                    'esi': 'https://esi.evetech.net/latest/killmails/106140056/e6ad2b2b40536d05ad96428e24795af21ce9a9d7/',
                    'url': 'https://zkillboard.com/kill/106140056/'
                }
            };
            sub.subscribe(SubscriptionType.PUBLIC, '888224317991706685', '1115807643748012072', new Map(), true, true, true, true, 1);
            // eslint-disable-next-line @typescript-eslint/ban-ts-comment
            // @ts-ignore
            await sub.sendMessageToDiscord('888224317991706685', '1115807643748012072', null, zk_data, 1, 'Mobile Small Warp Disruptor II', 'GREEN');
            console.log('done');
        });
        await new Promise(resolve => setTimeout(resolve, 7000));
    });
});
