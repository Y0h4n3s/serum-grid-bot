use mongodb::sync::Client;
use crate::mongodb::models::{ Trader};

pub struct MongoClient {
    pub database: mongodb::sync::Database,
    pub traders: mongodb::sync::Collection<Trader>,
}

impl MongoClient {
    pub fn new() -> Self {
        let mut mongodb_url = "mongodb://localhost:27017/gridbot".to_string();
        match std::env::var("MONGODB_URL") {
            Ok(url) => { mongodb_url = url }
            Err(_) => {}
        }

        let client = Client::with_uri_str(&mongodb_url).unwrap();
        let database = client.database("gridbot");
        let traders = database.collection::<Trader>("traders");

        MongoClient {
            database,
            traders,
        }
    }
}