import {SlashCommandBuilder} from '@discordjs/builders';
import {CommandInteraction} from 'discord.js';
import {AbstractCommand} from './abstractCommand';
import {ZKillSubscriber} from '../zKillSubscriber';

export class HelpCommand extends AbstractCommand {
    protected name = 'zk-activity-diag';

    executeCommand(interaction: CommandInteraction): void {
        const sub = ZKillSubscriber.getInstance();
        if (!interaction.inGuild()) {
            // @ts-ignore
            interaction.reply('Diagnostics is not possible in PM!');
            return;
        }
        const content = JSON.stringify(sub.listGuildChannelSubscriptions(interaction.guildId, interaction.channelId), null, 2);
        interaction.reply({
            content: content,
            ephemeral: true
        });
    }

    getCommand(): SlashCommandBuilder {
        return new SlashCommandBuilder().setName(this.name)
            .setDescription('Help and diagnostics');

    }

}

