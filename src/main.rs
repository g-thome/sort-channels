use std::collections::{HashSet, HashMap};
use std::env;
use std::sync::{Arc, RwLock};

use serenity::async_trait;
use serenity::client::{Client, Context, EventHandler};
use serenity::framework::standard::{
    macros::{command, group, hook},
    CommandResult, StandardFramework, Args
};
use serenity::model::channel::{Channel, ChannelType, GuildChannel, Message};
use serenity::model::guild::Guild;
use serenity::prelude::*;

use dotenv::dotenv;

use db::DB;
mod db;

extern crate alphanumeric_sort;

struct LockedGuilds;

impl TypeMapKey for LockedGuilds {
    type Value = HashSet<u64>;
}

impl TypeMapKey for DB {
    type Value = DB;
}

struct PrefixMap;

impl TypeMapKey for PrefixMap {
    type Value = HashMap<u64, String>;
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

#[hook]
async fn dynamic_prefix(ctx: &Context, msg: &Message) -> Option<String> {
    let default_prefix = ".";

    let prefixes = {
        let data = ctx.data.read().await;
        data.get::<PrefixMap>().unwrap().clone()
    };

    let guild_id = msg.guild_id.unwrap();
    match prefixes.get(&guild_id.0) {
        Some(prefix) => Some(prefix.to_string()),
        None => Some(default_prefix.to_string())
    }
}

#[group]
#[commands(ping, sort, prefix)]
struct General;

struct Handler;

async fn sort_channels(ctx: &Context, channels: &mut Vec<GuildChannel>) -> serenity::Result<()> {
    let mut channel_names = channels
        .iter()
        .cloned()
        .map(|c| c.name)
        .collect::<Vec<String>>();
    alphanumeric_sort::sort_str_slice(&mut channel_names);

    for channel in channels.into_iter() {
        let old_position = channel.position;
        let new_position = channel_names
            .iter()
            .cloned()
            .position(|c| c == channel.name)
            .unwrap();

        if new_position as i64 != old_position {
            let r = channel
                .edit(&ctx, |c| c.position(new_position as u64))
                .await;
            match r {
                Ok(_) => println!("channel sorted"),
                Err(e) => println!("error on edit: {}", e),
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
            None => return,
        };

        let guild_channels = match g.guild_id.channels(&ctx).await {
            Ok(x) => x,
            _ => return,
        };

        let locked_guilds = {
            let data = ctx.data.read().await;
            data.get::<LockedGuilds>().unwrap().clone()
        };

        if let Some(_) = locked_guilds.get(&g.guild_id.0) {
            return;
        }

        let mut filtered_channels: Vec<GuildChannel> = guild_channels
            .values()
            .into_iter()
            .cloned()
            .filter(|c| c.kind == ChannelType::Text)
            .collect();
        lock_guild(&ctx, g.guild_id.0).await;
        match sort_channels(&ctx, &mut filtered_channels).await {
            Ok(_) => {}
            Err(_) => println!("failed sorting channels"),
        }
        unlock_guild(&ctx, g.guild_id.0).await;
    }

    async fn channel_create(&self, ctx: Context, channel: &GuildChannel) {
        let g = match channel.guild(&ctx).await {
            Some(g) => g,
            None => return,
        };

        let guild_channels = match g.channels(&ctx).await {
            Ok(x) => x,
            _ => return,
        };

        let locked_guilds = {
            let data = ctx.data.read().await;
            data.get::<LockedGuilds>().unwrap().clone()
        };

        if let Some(_) = locked_guilds.get(&g.id.0) {
            return;
        }

        let mut filtered_channels: Vec<GuildChannel> = guild_channels
            .values()
            .into_iter()
            .cloned()
            .filter(|c| c.kind == ChannelType::Text)
            .collect();
        lock_guild(&ctx, g.id.0).await;
        match sort_channels(&ctx, &mut filtered_channels).await {
            Ok(_) => {}
            Err(_) => println!("failed sorting channels"),
        }
        unlock_guild(&ctx, g.id.0).await;
    }

    async fn guild_create(&self, ctx: Context, guild: Guild, is_new: bool) {
        if is_new {
            println!("creating new guild {}", guild.name);
            let data = ctx.data.read().await;
            let db = data.get::<DB>().unwrap();
            db.add_guild(&guild.id.0).await.unwrap();
        }

        println!("guild already known");
    }
}

// fn get_dynamic_prefix(_ctx: &Context, msg: Message) -> {}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db = DB::init().await.unwrap();
    let prefixes = db.get_prefixes().await.unwrap();

    let framework = StandardFramework::new()
        .configure(|c| {
            c.dynamic_prefix(dynamic_prefix)
        })
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
        data.insert::<DB>(db);
        data.insert::<PrefixMap>(prefixes);
    }

    if let Err(why) = client.start().await {
        println!("An error occurred while running the client: {:?}", why);
    }

    println!("end of main");
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
        None => return Ok(()),
    };

    let guild_channels = match g.channels(&ctx).await {
        Ok(x) => x,
        _ => return Ok(()),
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

    let mut filtered_channels: Vec<GuildChannel> = guild_channels
        .values()
        .into_iter()
        .cloned()
        .filter(|c| c.kind == ChannelType::Text)
        .collect();

    lock_guild(&ctx, g.id.0).await;
    match sort_channels(&ctx, &mut filtered_channels).await {
        Ok(_) => {}
        Err(_) => println!("failed sorting channels"),
    }
    unlock_guild(&ctx, g.id.0).await;

    Ok(())
}

#[command]
async fn prefix(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    println!("entering prefix command");
    let default_prefix = ".";

    let guild_id = msg.guild_id.unwrap().0;

    let prefixes = {
        println!("getting prefixes mutex");
        let data = ctx.data.read().await;
        data.get::<PrefixMap>().unwrap().clone()
    };

    if args.is_empty() {
        let current_prefix = match prefixes.get(&guild_id) {
            Some(prefix) => prefix.to_string(),
            None => default_prefix.to_string()
        };

        println!("sending message");
        msg.channel_id.say(ctx, format!("My prefix for this server is {}\n use the command prefix <NEWONE> to set a new prefix", current_prefix)).await?;

        return Ok(());
    }

    let new_prefix = args.single::<String>().unwrap();

    {
        let data = ctx.data.read().await;
        let db = data.get::<DB>().unwrap();
        db.set_prefix(&guild_id, &new_prefix).await.unwrap();
    }

    {
        let mut data = ctx.data.write().await;
        let prefixes = data.get_mut::<PrefixMap>().unwrap();
        
        let entry = prefixes.entry(guild_id).or_insert("".to_string());
        *entry = new_prefix.clone();
    }
    
    msg.channel_id.say(ctx, format!("Done! My prefix for this server is now {}", new_prefix)).await?;

    Ok(())
}
