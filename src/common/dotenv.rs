use std::path::PathBuf;

use crate::common::types::DatabaseType;
use url::{Url};
use dotenv;

#[derive(Clone, Debug)]
pub struct Dotenv {
  pub db_type: Option<DatabaseType>,
  pub db_user: Option<String>,
  pub db_host: Option<String>,
  pub db_port: Option<u16>,
  pub db_pass: Option<String>,
  pub db_name: Option<String>,
  pub pg_search_path: Option<String>,
}

impl Default for Dotenv {
  fn default() -> Self {
    Self::new(None)
  }
}

impl Dotenv {

  fn get_var(key: &str) -> Option<String> {
    dotenv::var(key).ok()
  }

  fn get_db_type<T: AsRef<str>>(t: Option<T>) -> Option<DatabaseType> {
    t.map(|val| match val.as_ref() {
        "mysql" => DatabaseType::Mysql,
        _ => DatabaseType::Postgres,
    })
  }

  pub fn new(path_to_dotenv: Option<std::path::PathBuf>) -> Dotenv {
    if let Some(value) = path_to_dotenv {
      dotenv::from_path(PathBuf::from(value)).ok();
    }

    if let Some(url_str) = Self::get_var("DATABASE_URL") {
      let url = Url::parse(&url_str).expect("Invalid DATABASE_URL");

      return Dotenv {
        db_type: Self::get_db_type(Some(url.scheme())),
        db_user: Some(url.username().to_string()).filter(|s| !s.is_empty()),
        db_pass: url.password().map(|s| s.to_string()),
        db_host: url.host_str().map(|s| s.to_string()),
        db_port: Some(url.port()).expect("DB_PORT is missing"),
        db_name: Some(url.path().trim_start_matches('/').to_string()),
        pg_search_path: Self::get_var("PG_SEARCH_PATH"),
      };
    }

    Dotenv {
      db_type: Self::get_db_type(Self::get_var("DB_TYPE")),
      db_user: Self::get_var("DB_USER"),
      db_host: Self::get_var("DB_HOST"),
      db_port: Self::get_var("DB_PORT")
        .map(|val| val.parse::<u16>()
        .expect("DB_PORT is not a valid integer")),
      db_pass: Self::get_var("DB_PASS"),
      db_name: Self::get_var("DB_NAME"),
      pg_search_path: Self::get_var("PG_SEARCH_PATH"),
    }
  }
}
