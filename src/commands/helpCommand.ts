import {SlashCommandBuilder} from '@discordjs/builders';
import {CommandInteraction} from 'discord.js';
import * as util from 'util';
import {AbstractCommand} from './abstractCommand';
import {ZKillSubscriber} from '../zKillSubscriber';

export class HelpCommand extends AbstractCommand {
    protected name = 'zk-activity-diag';

    async executeCommand(interaction: CommandInteraction): Promise<void> {
        const sub = ZKillSubscriber.getInstance();
        if (!interaction.inGuild()) {
            // @ts-ignore
            await interaction.reply('Diagnostics is not possible in PM!');
            return;
        }
        const subs = await sub.listGuildChannelSubscriptions(interaction.guildId, interaction.channelId);

        const subs_str = util.inspect(subs, { showHidden: false, depth: 5 } );
        const log = [
            'List of subscriptions for guild',
            interaction.guildId,
            'channel',
            interaction.channelId,
            'is',
            subs_str
        ].join(' ');

        console.log(log);

        await interaction.reply({
            content: log,
            ephemeral: true
        });
    }

    getCommand(): SlashCommandBuilder {
        return new SlashCommandBuilder().setName(this.name)
            .setDescription('Help and diagnostics');

    }

}



