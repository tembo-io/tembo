use crate::db;
use crate::error_handler::CustomError;
use crate::schema::items;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, AsChangeset, Insertable)]
#[table_name = "items"]
pub struct Item {
    pub id: i64,
    pub title: String,
}

#[derive(Serialize, Deserialize, Queryable)]
pub struct Items {
    pub id: i64,
    pub title: String,
}

impl Items {
    pub fn find_all() -> Result<Vec<Self>, CustomError> {
        let conn = db::connection()?;
        let items = items::table.load::<Items>(&conn)?;
        Ok(items)
    }

}

impl Item {
    fn from(item: Item) -> Item {
        Item {
            id: item.id,
            title: item.title,
        }
    }
}