import {Client, Intents} from 'discord.js';
import {LimitType, Subscription, SubscriptionType, ZkData, ZKillSubscriber} from '../zKillSubscriber';

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
            await sub.sendMessageToDiscord('888224317991706685', '1115807643748012072', sub, zk_data, 'Mobile Small Warp Disruptor II', 26892, 30, 'RED');
            console.log('done');
        });
        await new Promise(resolve => setTimeout(resolve, 7000));
    });
    it('should compare sec status correctly', async () => {
        const client = new Client({intents: [Intents.FLAGS.GUILDS]});
        await client.login(process.env.DISCORD_BOT_TOKEN);
        console.log('logged in');
        const sub = ZKillSubscriber.getInstance(client)
            .withSystems('../config/')
            .withShips('../config/')
            .withNames('../config/');
        const data: ZkData = {
            attackers: [],
            killmail_id: 0,
            killmail_time: '',
            solar_system_id: 0,
            victim: {
                alliance_id: 0,
                corporation_id: 0,
                damage_taken: 0,
                items: [],
                position: { x: 0, y: 0, z: 0 }
            }, zkb: {
                locationID: 0,
                hash: '',
                fittedValue: 0,
                droppedValue: 0,
                destroyedValue: 0,
                totalValue: 0,
                points: 0,
                npc: false,
                solo: false,
                awox: false,
                esi: '',
                url: '',
            }
        };
        const lowsec0point1 = [
            // {'solarSystemID': '30001442', 'solarSystemName': 'Saranen', 'security': 0.0962943885,'ls': true},
            // {'solarSystemID': '30002080', 'solarSystemName': 'Arifsdald', 'security': 0.0927691123,'ls': true},
            // {'solarSystemID': '30002412', 'solarSystemName': 'Ennur', 'security': 0.0583488141,'ls': true},
            // {'solarSystemID': '30002420', 'solarSystemName': 'Egbinger', 'security': 0.0345551972,'ls': true},
            // {'solarSystemID': '30003560', 'solarSystemName': 'Hoshoun', 'security': 0.0851373893,'ls': true},
            // {'solarSystemID': '30003562', 'solarSystemName': 'Ziriert', 'security': 0.0595832109,'ls': true},
            // {'solarSystemID': '30003563', 'solarSystemName': 'Misaba', 'security': 0.0640079707,'ls': true},
            // {'solarSystemID': '30003564', 'solarSystemName': 'Rephirib', 'security': 0.0956335663,'ls': true},
            // {'solarSystemID': '30003600', 'solarSystemName': 'Agaullores', 'security': 0.0712008819,'ls': true},
            // {'solarSystemID': '30003789', 'solarSystemName': 'Brarel', 'security': 0.0943159914,'ls': true},
            // {'solarSystemID': '30003791', 'solarSystemName': 'Annancale', 'security': 0.0940761659,'ls': true},
            // {'solarSystemID': '30003793', 'solarSystemName': 'Harroule', 'security': 0.0850605913,'ls': true},
            // {'solarSystemID': '30003804', 'solarSystemName': 'Pain', 'security': 0.0940808457,'ls': true},
            // {'solarSystemID': '30003819', 'solarSystemName': 'Barleguet', 'security': 0.0682008642,'ls': true},
            // {'solarSystemID': '30003820', 'solarSystemName': 'Vestouve', 'security': 0.0425947655,'ls': true},
            // {'solarSystemID': '30003821', 'solarSystemName': 'Ausmaert', 'security': 0.0748431865,'ls': true},
            // {'solarSystemID': '30003822', 'solarSystemName': 'Espigoure', 'security': 0.0499266216,'ls': true},
            // {'solarSystemID': '30003856', 'solarSystemName': 'Athounon', 'security': 0.0762059116,'ls': true},
            // {'solarSystemID': '30003934', 'solarSystemName': 'Cabeki', 'security': 0.0897861682,'ls': true},
            // {'solarSystemID': '30003935', 'solarSystemName': 'Irmalin', 'security': 0.062148134,'ls': true},
            // {'solarSystemID': '30004256', 'solarSystemName': 'Edilkam', 'security': 0.0720004564,'ls': true},
            // {'solarSystemID': '30004258', 'solarSystemName': 'Khnar', 'security': 0.0916075577,'ls': true},
            // {'solarSystemID': '30004260', 'solarSystemName': 'Yiratal', 'security': 0.0379865815,'ls': true},
            // {'solarSystemID': '30004261', 'solarSystemName': 'Balas', 'security': 0.0436347183,'ls': true},
            // {'solarSystemID': '30004262', 'solarSystemName': 'Pemsah', 'security': 0.0479459662,'ls': true},
            // {'solarSystemID': '30004263', 'solarSystemName': 'Feshur', 'security': 0.0355186657,'ls': true},
            // {'solarSystemID': '30004264', 'solarSystemName': 'Hoseen', 'security': 0.0757257082,'ls': true},
            // {'solarSystemID': '30004265', 'solarSystemName': 'Yekh', 'security': 0.0444337419,'ls': true},
            // {'solarSystemID': '30004278', 'solarSystemName': 'Sheri', 'security': 0.0568531743,'ls': true},
            // {'solarSystemID': '30004297', 'solarSystemName': 'Efu', 'security': 0.0923133982,'ls': true},
            // {'solarSystemID': '30004298', 'solarSystemName': 'Tisot', 'security': 0.0898759449,'ls': true},
            // {'solarSystemID': '30004299', 'solarSystemName': 'Sakht', 'security': 0.0414994338,'ls': true},
            // {'solarSystemID': '30004300', 'solarSystemName': 'Naga', 'security': 0.0336844625,'ls': true},
            // {'solarSystemID': '30004301', 'solarSystemName': 'Anath', 'security': 0.0413269226,'ls': true},
            // {'solarSystemID': '30004306', 'solarSystemName': 'Karan', 'security': 0.044736305,'ls': true},
            {'solarSystemID': '30004309', 'solarSystemName': 'Hophib', 'security': 0.0291474894,'ls': true},
            {'solarSystemID': '30004706', 'solarSystemName': 'UHKL-N', 'security': -0.0052409493, 'ls': false}
        ];
        const highsec0point45 = [
            {'solarSystemID': '30002634', 'solarSystemName': 'Balle', 'security': 0.4608890986, 'hs': true},
            {'solarSystemID': '30002645', 'solarSystemName': 'Carrou', 'security': 0.4405891678, 'hs': false},
            {'solarSystemID': '30002647', 'solarSystemName': 'Ignoitton', 'security': 0.4387547559, 'hs': false},
            {'solarSystemID': '30002650', 'solarSystemName': 'Ney', 'security': 0.4567548925, 'hs': true},
            {'solarSystemID': '30002651', 'solarSystemName': 'Fasse', 'security': 0.4257427311, 'hs': false},
        ];

        const highsecOnlyLimit = new Map<LimitType, string>([
            [LimitType.SECURITY_MIN_INCLUSIVE, '0.5'],
        ]);
        const lowsecOnlyLimit = new Map<LimitType, string>([
            [LimitType.SECURITY_MIN_EXCLUSIVE, '0.0'],
            [LimitType.SECURITY_MAX_EXCLUSIVE, '0.5'],
        ]);
        const nullsecOnlyLimit = new Map<LimitType, string>([
            [LimitType.SECURITY_MAX_INCLUSIVE, '0.0'],
        ]);

        const highsecSub: Subscription = {
            exclusionLimitAlsoComparesAttacker: true,
            exclusionLimitAlsoComparesAttackerWeapons: true,
            inclusionLimitAlsoComparesAttacker: true,
            inclusionLimitAlsoComparesAttackerWeapons: true,
            limitTypes: highsecOnlyLimit,
            minValue: 0,
            subType: SubscriptionType.PUBLIC
        };
        for (const system of highsec0point45) {
            data.solar_system_id = Number(system.solarSystemID);
            const result = await sub.checkSecurityMinInclusive(highsecSub, data);
            console.log(system.solarSystemName + ' ' + result);
            expect(result).toBe(system['hs']);
        }

        const lowsecSub: Subscription = {
            exclusionLimitAlsoComparesAttacker: true,
            exclusionLimitAlsoComparesAttackerWeapons: true,
            inclusionLimitAlsoComparesAttacker: true,
            inclusionLimitAlsoComparesAttackerWeapons: true,
            limitTypes: lowsecOnlyLimit,
            minValue: 0,
            subType: SubscriptionType.PUBLIC
        };
        for (const system of lowsec0point1) {
            data.solar_system_id = Number(system.solarSystemID);
            let result = await sub.checkSecurityMinExclusive(lowsecSub, data);
            expect(result).toBe(system['ls']);
            result = await sub.checkSecurityMaxExclusive(lowsecSub, data);
            expect(result).toBe(true);
        }

        const nullsecSub: Subscription = {
            exclusionLimitAlsoComparesAttacker: true,
            exclusionLimitAlsoComparesAttackerWeapons: true,
            inclusionLimitAlsoComparesAttacker: true,
            inclusionLimitAlsoComparesAttackerWeapons: true,
            limitTypes: nullsecOnlyLimit,
            minValue: 0,
            subType: SubscriptionType.PUBLIC
        };
        for (const system of lowsec0point1) {
            data.solar_system_id = Number(system.solarSystemID);
            const result = await sub.checkSecurityMaxInclusive(nullsecSub, data);
            expect(result).toBe(!system['ls']);
        }
    });
});
