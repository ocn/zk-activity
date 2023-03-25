import {SlashCommandBuilder, SlashCommandSubcommandBuilder} from '@discordjs/builders';
import {CommandInteraction} from 'discord.js';
import {AbstractCommand} from './abstractCommand';
import {LimitType, SubscriptionType, ZKillSubscriber} from '../zKillSubscriber';

export class SubscribeCommand extends AbstractCommand {
    protected name = 'zkill-subscribe';

    executeCommand(interaction: CommandInteraction): void {
        const sub = ZKillSubscriber.getInstance();
        if(!interaction.inGuild()) {
            interaction.reply('Subscription is not possible in PM!');
            return;
        }
        const subCommand = interaction.options.getSubcommand(true) as SubscriptionType;
        const id = interaction.options.getNumber('id', false);
        const minValue = interaction.options.getNumber('min-value');
        const limitRegion = interaction.options.getString('limit-region-ids');
        const limitConstellation = interaction.options.getString('limit-constellation-ids');
        const limitSystem = interaction.options.getString('limit-system-ids');
        const limitShipsIncluded = interaction.options.getString('limit-included-ship-ids');
        const limitShipsExcluded = interaction.options.getString('limit-excluded-ship-ids');
        const limitSecurity = interaction.options.getString('limit-security');
        let inclusionLimitComparesAttackers = interaction.options.getBoolean('inclusion-limit-compares-attackers');
        let inclusionLimitComparesAttackerWeapons = interaction.options.getBoolean('inclusion-limit-compares-attacker-weapons');
        let exclusionLimitComparesAttackers = interaction.options.getBoolean('exclusion-limit-compares-attackers');
        let exclusionLimitComparesAttackerWeapons = interaction.options.getBoolean('exclusion-limit-compares-attacker-weapons');
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
        if (limitSecurity) {
            limitTypes.set(LimitType.SECURITY, limitSecurity);
            reply += '\nSecurity filter: + ' + limitSecurity;
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
                option.setName('min-value')
                    .setDescription('Minimum isk to show the entry')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName('limit-included-ship-ids')
                    .setDescription('Limit to ship id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName('limit-excluded-ship-ids')
                    .setDescription('Limit to ship id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName('limit-region-ids')
                    .setDescription('Limit to region id, comma seperated ids')
                    .setRequired(false)
            )
            .addStringOption(option =>
                option.setName('limit-security')
                    .setDescription('Limit to a maximum security')
                    .setRequired(false)
            )
            .addBooleanOption(option =>
                option.setName('inclusion-limit-compares-attackers')
                    .setDescription('Enable if attackers should be considered when sending mails')
                    .setRequired(false)
            )
            .addBooleanOption(option =>
                option.setName('inclusion-limit-compares-attacker-weapons')
                    .setDescription('Enable if attackers should be considered when sending mails')
                    .setRequired(false)
            )
            .addBooleanOption(option =>
                option.setName('exclusion-limit-compares-attackers')
                    .setDescription('Enable if attackers should be considered when rejecting mails')
                    .setRequired(false)
            )
            .addBooleanOption(option =>
                option.setName('exclusion-limit-compares-attacker-weapons')
                    .setDescription('Enable if attackers should be considered when rejecting mails')
                    .setRequired(false)
            )
            .setDescription('Subscribe public feed to channel'));

        return slashCommand;

    }

}
