# zk-activity
Post EVE Online killmails from zkillboard.com to a discord channel. Options apply a variety of filters for incoming mails.

The bot was forked from [hazardous](https://github.com/SvenBrnn/hazardous-killbot).

## Commands

| key                                              | description                                                                                      |
|--------------------------------------------------|--------------------------------------------------------------------------------------------------|
| /zkill-subscribe public [id]                     | Subscribe to the public feed with various filtering options. Parameters:                         |
|                                                  | - `id`: ID for public feed (required)                                                             |
|                                                  | - `min_value`: Minimum ISK to show the entry (optional)                                           |
|                                                  | - `limit_included_ship_ids`: Limit to certain ship IDs (comma-separated, optional)               |
|                                                  | - `limit_excluded_ship_ids`: Exclude certain ship IDs (comma-separated, optional)                |
|                                                  | - `limit_character_ids`: Limit to certain character IDs (comma-separated, optional)              |
|                                                  | - `limit_corporation_ids`: Limit to certain corporation IDs (comma-separated, optional)          |
|                                                  | - `limit_alliance_ids`: Limit to certain alliance IDs (comma-separated, optional)                |
|                                                  | - `limit_region_ids`: Limit to certain region IDs (comma-separated, optional)                    |
|                                                  | - `limit_security_max`: Limit to a maximum security (optional)                                   |
|                                                  | - `limit_security_min`: Limit to a minimum security (optional)                                   |
|                                                  | - `required_name_fragment`: Require a name fragment in the name of the matched type IDs (optional) |
|                                                  | - `inclusion_limit_compares_attackers`: Consider attackers when sending mails (optional)         |
|                                                  | - `inclusion_limit_compares_attacker_weapons`: Consider attackers' weapons when sending mails (optional) |
|                                                  | - `exclusion_limit_compares_attackers`: Consider attackers when rejecting mails (optional)       |
|                                                  | - `exclusion_limit_compares_attacker_weapons`: Consider attackers' weapons when rejecting mails (optional) |
| /zkill-unsubscribe all                           | Make the bot not post any on this channel anymore                                                |
| /zk-activity-diag                                | Display the current channel's list of subscriptions                                              |

Where to find the id?
![](./docs/id.png)

Open your corp/char/alliance page on zkillboard and copy the number at the end of the link.


## Development

Written in Typescript. Uses the zkillboard webhook endpoint and discord.js.

### Requirements:

- docker
- docker-compose

### Start up:

- Copy the env.sample to `.env` and fill out params
- Run `docker-compose up -d`

### Build:
 
- run `docker build -t zk-activity:latest`

### Config:

#### Environment

| key                  | description                        |
|----------------------|------------------------------------|
| DISCORD_BOT_TOKEN    | Your discord bot token             |
| DISCORD_CLIENT_ID    | Your discord application client id |

## Licence 
Copyright 2022 ocn

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
