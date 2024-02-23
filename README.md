# zk-activity
Post EVE Online killmails from zkillboard.com to a discord channel. Options apply a variety of filters for incoming mails.

The bot was forked from [hazardous](https://github.com/SvenBrnn/hazardous-killbot).

## Commands

| key                          | description                                                                                                |
|------------------------------|------------------------------------------------------------------------------------------------------------|
| /zkill-subscribe public [id] | Subscribe to the public feed with various filtering options. Parameters:                                   |
|                              | - `id`: ID for public feed (required)                                                                      |
|                              | - `min_value`: Minimum ISK to show the entry (optional)                                                    |
|                              | - `limit_included_ship_ids`: Limit to certain ship IDs (comma-separated, optional)                         |
|                              | - `limit_excluded_ship_ids`: Exclude certain ship IDs (comma-separated, optional)                          |
|                              | - `limit_character_ids`: Limit to certain character IDs (comma-separated, optional)                        |
|                              | - `limit_corporation_ids`: Limit to certain corporation IDs (comma-separated, optional)                    |
|                              | - `limit_alliance_ids`: Limit to certain alliance IDs (comma-separated, optional)                          |
|                              | - `limit_region_ids`: Limit to certain region IDs (comma-separated, optional)                              |
|                              | - `limit_security_max_inclusive`: Inclusive limit to a maximum security (optional)                         |
|                              | - `limit_security_max_exclusive`: Exclusive limit to a maximum security (optional)                         |
|                              | - `limit_security_min_inclusive`: Inclusive limit to a minimum security (optional)                         |
|                              | - `limit_security_min_exclusive`: Exclusive limit to a minimum security (optional)                         |
|                              | - `required_name_fragment`: Require a name fragment in the name of the matched type IDs (optional)         |
|                              | - `inclusion_limit_compares_attackers`: Consider attackers when sending mails (optional)                   |
|                              | - `inclusion_limit_compares_attacker_weapons`: Consider attackers' weapons when sending mails (optional)   |
|                              | - `exclusion_limit_compares_attackers`: Consider attackers when rejecting mails (optional)                 |
|                              | - `exclusion_limit_compares_attacker_weapons`: Consider attackers' weapons when rejecting mails (optional) |
| /zkill-unsubscribe all       | Make the bot not post any on this channel anymore                                                          |
| /zk-activity-diag            | Display the current channel's list of subscriptions                                                        |

## Examples 

Here's how an example embed notification looks when filtering for a ship group:

<img src="https://cdn.discordapp.com/attachments/972047941390987264/1210723339044393001/image.png?ex=65eb98fa&is=65d923fa&hm=e1aa3edc6723671aeeda0103e1fb4df9a11c62e4163675c9948f92989678dc9c&">

When not filtering for a ship group by a ship ID, the default https://zkillboard.org embed format is used.

### Where to find the id?
Open your corp/char/alliance page on https://zkillboard.com and copy the number at the end of the link.

Or, search for the item, group, or market group on https://everef.net.

<img src="./docs/id.png" width=900>

### Filtering by region of space

#### Highsec Only Limit:

- Set SECURITY_MIN_INCLUSIVE to '0.5'

#### Lowsec Only Limit:

- Set SECURITY_MIN_EXCLUSIVE to '0.0'
- Set SECURITY_MAX_EXCLUSIVE to '0.5'

#### Nullsec Only Limit:

- Set SECURITY_MAX_INCLUSIVE to '0.0'

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

## License 
Copyright 2024 ocn

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
