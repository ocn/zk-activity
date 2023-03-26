import {SlashCommandBuilder, SlashCommandSubcommandBuilder} from '@discordjs/builders';
import {CommandInteraction} from 'discord.js';
import {AbstractCommand} from './abstractCommand';
import {LimitType, SubscriptionType, ZKillSubscriber} from '../zKillSubscriber';

export class SubscribeCommand extends AbstractCommand {
    protected name = 'zkill-subscribe';

    protected MIN_VALUE = 'min-value';
    protected LIMIT_REGION_IDS = 'limit-region-ids';
    protected LIMIT_CONSTELLATION_IDS = 'limit-constellation-ids';
    protected LIMIT_SYSTEM_IDS = 'limit-system-ids';
    protected LIMIT_INCLUDED_SHIP_IDS = 'limit-included-ship-ids';
    protected LIMIT_EXCLUDED_SHIP_IDS = 'limit-excluded-ship-ids';
    protected LIMIT_SECURITY_MAX = 'limit-security-max';
    protected LIMIT_SECURITY_MIN = 'limit-security-min';
    protected INCLUSION_LIMIT_COMPARES_ATTACKERS = 'in-limit-compares-attackers';
    protected INCLUSION_LIMIT_COMPARES_ATTACKER_WEAPONS = 'in-limit-compares-attacker-weps';
    protected EXCLUSION_LIMIT_COMPARES_ATTACKERS = 'ex-limit-compares-attackers';
    protected EXCLUSION_LIMIT_COMPARES_ATTACKER_WEAPONS = 'ex-limit-compares-attacker-weps';

    executeCommand(interaction: CommandInteraction): void {
        const sub = ZKillSubscriber.getInstance();
        if(!interaction.inGuild()) {
            interaction.reply('Subscription is not possible in PM!');
            return;
        }
        const subCommand = interaction.options.getSubcommand(true) as SubscriptionType;
        const id = interaction.options.getNumber('id', false);
        const minValue = interaction.options.getNumber(this.MIN_VALUE);
        const limitRegion = interaction.options.getString(this.LIMIT_REGION_IDS);
        const limitConstellation = interaction.options.getString(this.LIMIT_CONSTELLATION_IDS);
        const limitSystem = interaction.options.getString(this.LIMIT_SYSTEM_IDS);
        const limitShipsIncluded = interaction.options.getString(this.LIMIT_INCLUDED_SHIP_IDS);
        const limitShipsExcluded = interaction.options.getString(this.LIMIT_EXCLUDED_SHIP_IDS);
        const limitSecurityMax = interaction.options.getString(this.LIMIT_SECURITY_MAX);
        const limitSecurityMin = interaction.options.getString(this.LIMIT_SECURITY_MIN);
        let inclusionLimitComparesAttackers = interaction.options.getBoolean(this.INCLUSION_LIMIT_COMPARES_ATTACKERS);
        let inclusionLimitComparesAttackerWeapons = interaction.options.getBoolean(this.INCLUSION_LIMIT_COMPARES_ATTACKER_WEAPONS);
        let exclusionLimitComparesAttackers = interaction.options.getBoolean(this.EXCLUSION_LIMIT_COMPARES_ATTACKERS);
        let exclusionLimitComparesAttackerWeapons = interaction.options.getBoolean(this.EXCLUSION_LIMIT_COMPARES_ATTACKER_WEAPONS);
        if (inclusionLimitComparesAttackers == null) {
            inclusionLimitComparesAttackers = true;
        }
        if (inclusionLimitComparesAttackerWeapons == null) {
            inclusionLimitComparesAttackerWeapons = true;
        }
        if (exclusionLimitComparesAttackers == null) {
            exclusionLimitComparesAttackers = true;
        }
        if (exclusionLimitComparesAttackerWeapons == null) {
            exclusionLimitComparesAttackerWeapons = true;
        }
        let reply = 'We subscribed to zkillboard channel: ' + interaction.options.getSubcommand();
        const limitTypes = new Map<LimitType, string>();
        if (limitRegion) {
            limitTypes.set(LimitType.REGION, limitRegion);
            reply += '\nRegion filter: + ' + limitRegion;
        }
        if (limitConstellation) {
            limitTypes.set(LimitType.CONSTELLATION, limitConstellation);
            reply += '\nConstellation filter: + ' + limitRegion;
        }
        if (limitSystem) {
            limitTypes.set(LimitType.SYSTEM, limitSystem);
            reply += '\nSystem filter: + ' + limitRegion;
        }
        if (limitShipsIncluded) {
            limitTypes.set(LimitType.SHIP_INCLUSION_TYPE_ID, limitShipsIncluded);
            reply += '\nShip ID Inclusion filter: + ' + limitShipsIncluded;
        }
        if (limitShipsExcluded) {
            limitTypes.set(LimitType.SHIP_EXCLUSION_TYPE_ID, limitShipsExcluded);
            reply += '\nShip ID Exclusion filter: - ' + limitShipsExcluded;
        }
        if (limitSecurityMax) {
            limitTypes.set(LimitType.SECURITY_MAX, limitSecurityMax);
            reply += '\nMax Security filter: + ' + limitSecurityMax;
        }
        if (limitSecurityMin) {
            limitTypes.set(LimitType.SECURITY_MIN, limitSecurityMin);
            reply += '\nMin Security filter: + ' + limitSecurityMin;
        }
        sub.subscribe(
            subCommand, 
            interaction.guildId, 
            interaction.channelId, 
            limitTypes,
            inclusionLimitComparesAttackers,
            inclusionLimitComparesAttackerWeapons,
            exclusionLimitComparesAttackers,
            exclusionLimitComparesAttackerWeapons,
            id ? id : undefined,
            minValue ? minValue : 0,
        );

        if(id) {
            reply += ' ID: ' + id;
        }
        if(minValue) {
            reply += ' Min Value: ' + minValue.toLocaleString('en');
        }
        interaction.reply({content: reply, ephemeral: true});
    }

    getCommand(): SlashCommandBuilder {
        const slashCommand = new SlashCommandBuilder().setName(this.name)
            .setDescription('Subscribe to zkill');


        slashCommand.addSubcommand( new SlashCommandSubcommandBuilder().setName('corporation')
            .setDescription('Subscribe corporation to channel')
            .addNumberOption(option =>
                option.setName('id')
                    .setDescription('ID for the corporation')
                    .setRequired(true)
            )
            .addStringOption(option =>
                option.setName('limit-region-ids')
                    .setDescription('Limit to region id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName('limit-constellation-ids')
                    .setDescription('Limit to constellation id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName('limit-system-ids')
                    .setDescription('Limit to system id, comma seperated ids')
                    .setRequired(false)
            )
            .addNumberOption(option =>
                option.setName('min-value')
                    .setDescription('Minimum isk to show the entry')
                    .setRequired(false)
            ));

        slashCommand.addSubcommand( new SlashCommandSubcommandBuilder().setName('alliance')
            .setDescription('Subscribe alliance to channel')
            .addNumberOption(option =>
                option.setName('id')
                    .setDescription('ID for the alliance')
                    .setRequired(true)
            )
            .addStringOption(option =>
                option.setName('limit-region-ids')
                    .setDescription('Limit to region id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName('limit-constellation-ids')
                    .setDescription('Limit to constellation id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName('limit-system-ids')
                    .setDescription('Limit to system id, comma seperated ids')
                    .setRequired(false)
            )
            .addNumberOption(option =>
                option.setName('min-value')
                    .setDescription('Minimum isk to show the entry')
                    .setRequired(false)
            ));

        slashCommand.addSubcommand( new SlashCommandSubcommandBuilder().setName('character')
            .setDescription('Subscribe character to channel')
            .addNumberOption(option =>
                option.setName('id')
                    .setDescription('ID for the character')
                    .setRequired(true)

            )
            .addNumberOption(option =>
                option.setName('min-value')
                    .setDescription('Minimum isk to show the entry')
                    .setRequired(false)
            ));

        slashCommand.addSubcommand( new SlashCommandSubcommandBuilder().setName('group')
            .setDescription('Subscribe group to channel')
            .addNumberOption(option =>
                option.setName('id')
                    .setDescription('ID for the group')
                    .setRequired(true)

            )
            .addNumberOption(option =>
                option.setName('min-value')
                    .setDescription('Minimum isk to show the entry')
                    .setRequired(false)
            ));

        slashCommand.addSubcommand( new SlashCommandSubcommandBuilder().setName('public')
            .addNumberOption(option =>
                option.setName(this.MIN_VALUE)
                    .setDescription('Minimum isk to show the entry')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName(this.LIMIT_INCLUDED_SHIP_IDS)
                    .setDescription('Limit to ship id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName(this.LIMIT_EXCLUDED_SHIP_IDS)
                    .setDescription('Limit to ship id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName(this.LIMIT_REGION_IDS)
                    .setDescription('Limit to region id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName(this.LIMIT_SECURITY_MAX)
                    .setDescription('Limit to a maximum security')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName(this.LIMIT_SECURITY_MIN)
                    .setDescription('Limit to a minimum security')
                    .setRequired(false)
            )
            .addBooleanOption(option =>
                option.setName(this.INCLUSION_LIMIT_COMPARES_ATTACKERS)
                    .setDescription('Enable if attackers should be considered when sending mails')
                    .setRequired(false)
            )
            .addBooleanOption(option =>
                option.setName(this.INCLUSION_LIMIT_COMPARES_ATTACKER_WEAPONS)
                    .setDescription('Enable if attackers should be considered when sending mails')
                    .setRequired(false)
            )
            .addBooleanOption(option =>
                option.setName(this.EXCLUSION_LIMIT_COMPARES_ATTACKERS)
                    .setDescription('Enable if attackers should be considered when rejecting mails')
                    .setRequired(false)
            )
            .addBooleanOption(option =>
                option.setName(this.EXCLUSION_LIMIT_COMPARES_ATTACKER_WEAPONS)
                    .setDescription('Enable if attackers should be considered when rejecting mails')
                    .setRequired(false)
            )
            .setDescription('Subscribe public feed to channel'));

        return slashCommand;

    }

}
