use mongodb::{Client, error::Result as MResult};
use mongodb::bson::{doc, Bson};
use mongodb::options::{ClientOptions, FindOptions};

use serenity::http::client::Http;
use serenity::futures::StreamExt;

use std::env;
use std::collections::HashMap;

pub struct DB {
    pub mongodb_client: Client,
    pub http_client: Http,
}

pub struct GuildConfig {
    pub always_on_top: Vec<u64>,
    pub always_on_bottom: Vec<u64>,
    pub ignore: Vec<u64>,
    pub prefix: String,
    pub name: String
}

const DB_NAME: &str = "sort-channels";
const GUILDS_COLLECTION: &str = "guilds";
const CONNECTION_STRING: &str = "mongodb://localhost:27017";

impl DB {
    pub async fn init() -> MResult<Self> {
        let mut options = ClientOptions::parse(CONNECTION_STRING).await?;
        options.app_name = Some("sort-channels".to_string());

        let token = env::var("DISCORD_TOKEN").expect("discord token missing");

        let http_client = Http::new_with_token(&token);

        Ok(Self {
            mongodb_client: Client::with_options(options)?,
            http_client: http_client
        })
    }

    pub async fn add_guild(&self, id: &u64) -> MResult<()> {
        let doc = doc! {
            "_id": id,
        };

        self.mongodb_client.database(DB_NAME).collection(GUILDS_COLLECTION)
            .insert_one(doc, None)
            .await?;

        Ok(())
    }

    pub async fn update_guild_config(&self, guild_id: &u64, new_config: &GuildConfig) -> MResult<()> {
        let doc = doc! {
            "name": new_config.name.clone(),
            "always_on_top": new_config.always_on_top.clone(),
            "always_on_bottom": new_config.always_on_bottom.clone(),
            "ignore": new_config.ignore.clone(),
            "prefix": new_config.prefix.clone(),
        };

        let query = doc! {
            "_id": guild_id
        };

        self.mongodb_client.database(DB_NAME).collection(GUILDS_COLLECTION)
            .update_one(query, doc, None)
            .await?;

        Ok(())
    }

    pub async fn set_prefix(&self, guild_id: &u64, new_prefix: &String) -> MResult<()> {
        let doc = doc! {
            "prefix": new_prefix
        };

        let query = doc! {
            "_id": guild_id
        };

        self.mongodb_client.database(DB_NAME).collection(GUILDS_COLLECTION)
            .update_one(query, doc, None)
            .await?;

        Ok(())
    }

    pub async fn get_prefixes(&self) -> MResult<HashMap<u64, String>> {
        let projection = doc! {
            "_id": 1,
            "prefix": 1
        };

        let query_options = FindOptions::builder().projection(projection).build();

        let mut cursor = self.mongodb_client.database(DB_NAME).collection(GUILDS_COLLECTION)
            .find(None, query_options)
            .await?;
        
        let mut prefixes = HashMap::new();

        while let Some(r) = cursor.next().await {
            match r {
                Ok(document) => {
                    let id = document.get("_id").and_then(Bson::as_i64).unwrap() as u64;
                    if let Some(prefix) = document.get("prefix").and_then(Bson::as_str) {
                        prefixes.insert(id, prefix.to_string());
                    }
                },
                Err(_) => {}
            }
        }

        Ok(prefixes)
    }
}