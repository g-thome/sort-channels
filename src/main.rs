use std::collections::HashSet;
use std::env;

use serenity::prelude::*;
use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{command, group},
    CommandResult, StandardFramework
};
use serenity::model::channel::{Message, MessageType, Channel, ChannelType, GuildChannel};

use dotenv::dotenv;

extern crate alphanumeric_sort;

struct LockedGuilds;

impl TypeMapKey for LockedGuilds {
    type Value = HashSet<u64>;
}

async fn lock_guild(ctx: &Context, guild_id: u64) {
    let mut data = ctx.data.write().await;
    let locked_guilds = data.get_mut::<LockedGuilds>().unwrap();
    locked_guilds.insert(guild_id);
}

async fn unlock_guild(ctx: &Context, guild_id: u64) {
    let mut data = ctx.data.write().await;
    let locked_guilds = data.get_mut::<LockedGuilds>().unwrap();
    locked_guilds.remove(&guild_id);
}

#[group]
#[commands(ping,sort)]
struct General;

struct Handler;

async fn sort_channels(ctx: &Context, channels: &mut Vec<GuildChannel>) -> serenity::Result<()> {
    let mut channel_names = channels.iter().cloned().map(|c| c.name).collect::<Vec<String>>();
    alphanumeric_sort::sort_str_slice(&mut channel_names);

    for channel in channels.into_iter() {
        let old_position = channel.position;
        let new_position = channel_names.iter().cloned().position(|c| c == channel.name).unwrap();

        if new_position as i64 != old_position {
            let r = channel.edit(&ctx, |c| c.position(new_position as u64)).await;
            match r {
                Ok(_) => println!("channel sorted"),
                Err(e) => println!("error on edit: {}", e)
            }
        }
    }

    Ok(())
}

#[async_trait]
impl EventHandler for Handler {
    async fn channel_update(&self, ctx: Context, _old: Option<Channel>, new: Channel) {
        let g = match new.guild() {
            Some(g) => g,
            None => return
        };

        let guild_channels = match g.guild_id.channels(&ctx).await {
            Ok(x) => x,
            _ => return
        };

        let locked_guilds = {
            let data = ctx.data.read().await;
            data.get::<LockedGuilds>().unwrap().clone()
        };

        if let Some(_) = locked_guilds.get(&g.guild_id.0) {
            return;
        }

        let mut filtered_channels: Vec<GuildChannel> = guild_channels.values().into_iter().cloned().filter(|c| c.kind == ChannelType::Text).collect();
        
        lock_guild(&ctx, g.guild_id.0).await;
        match sort_channels(&ctx, &mut filtered_channels).await {
            Ok(_) => {},
            Err(_) => println!("failed sorting channels")
        }
        unlock_guild(&ctx, g.guild_id.0).await;
    }

    async fn channel_create(&self, ctx: Context, channel: &GuildChannel) {
        let g = match channel.guild(&ctx).await {
            Some(g) => g,
            None => return
        };

        let guild_channels = match g.channels(&ctx).await {
            Ok(x) => x,
            _ => return
        };

        let locked_guilds = {
            let data = ctx.data.read().await;
            data.get::<LockedGuilds>().unwrap().clone()
        };

        if let Some(_) = locked_guilds.get(&g.id.0) {
            return;
        }

        let mut filtered_channels: Vec<GuildChannel> = guild_channels.values().into_iter().cloned().filter(|c| c.kind == ChannelType::Text).collect();
        
        lock_guild(&ctx, g.id.0).await;
        match sort_channels(&ctx, &mut filtered_channels).await {
            Ok(_) => {},
            Err(_) => println!("failed sorting channels")
        }
        unlock_guild(&ctx, g.id.0).await;
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("."))
        .group(&GENERAL_GROUP);

    
    let token = env::var("DISCORD_TOKEN").expect("discord token missing");

    let mut client = Client::builder(token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Error creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<LockedGuilds>(HashSet::default());
    }

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }
}

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
    msg.reply(ctx, "Pong!").await?;

    Ok(())
}

#[command]
async fn sort(ctx: &Context, msg: &Message) -> CommandResult {
    if msg.is_private() {
        return Ok(());
    }
    
    let g = match msg.guild(&ctx).await {
        Some(g) => g,
        None => return Ok(())
    };

    let guild_channels = match g.channels(&ctx).await {
        Ok(x) => x,
        _ => return Ok(())
    };

    let locked_guilds = {
        let data = ctx.data.read().await;
        data.get::<LockedGuilds>().unwrap().clone()
    };

    if let Some(_) = locked_guilds.get(&g.id.0) {
        if let Err(e) = msg.reply(&ctx, "sorting already in action").await {
            println!("error sending message {}", e);
        };
        return Ok(());
    }

    let mut filtered_channels: Vec<GuildChannel> = guild_channels.values().into_iter().cloned().filter(|c| c.kind == ChannelType::Text).collect();

    lock_guild(&ctx, g.id.0).await;
    match sort_channels(&ctx, &mut filtered_channels).await {
        Ok(_) => {},
        Err(_) => println!("failed sorting channels")
    }
    unlock_guild(&ctx, g.id.0).await;

    Ok(())
}