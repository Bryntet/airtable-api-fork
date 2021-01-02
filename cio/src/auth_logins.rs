use std::collections::HashMap;
use std::env;
use std::{thread, time};

use chrono::naive::NaiveDateTime;
use chrono::offset::Utc;
use chrono::DateTime;
use chrono_humanize::HumanTime;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::db::Database;
use crate::models::{NewAuthUser, NewAuthUserLogin};
use crate::utils::{DOMAIN, GSUITE_DOMAIN};

/// The data type for an Auth0 user.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub user_id: String,
    pub email: String,
    #[serde(default)]
    pub email_verified: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub username: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub given_name: String,
    pub name: String,
    pub nickname: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub picture: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub phone_number: String,
    #[serde(default)]
    pub phone_verified: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub locale: String,
    pub identities: Vec<Identity>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login: DateTime<Utc>,
    pub last_ip: String,
    pub logins_count: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub blog: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub company: String,
}

impl User {
    /// Convert an auth0 user into a NewAuthUser.
    #[instrument]
    #[inline]
    pub fn to_auth_user(&self) -> NewAuthUser {
        let mut company: &str = &self.company;
        // Check if we have an Oxide email address.
        if self.email.ends_with(&format!("@{}", GSUITE_DOMAIN)) || self.email.ends_with(&format!("@{}", DOMAIN)) || *self.company.trim() == *"Oxide Computer Company" {
            company = "@oxidecomputer";
        } else if self.email.ends_with("@bench.com") {
            // Check if we have a Benchmark Manufacturing email address.
            company = "@bench";
        } else if *self.company.trim() == *"Algolia" {
            // Cleanup algolia.
            company = "@algolia";
        } else if *self.company.trim() == *"0xF9BA143B95FF6D82" || self.company.trim().is_empty() || *self.company.trim() == *"TBD" {
            // Cleanup David Tolnay and other weird empty parses
            company = "";
        }

        NewAuthUser {
            user_id: self.user_id.to_string(),
            name: self.name.to_string(),
            nickname: self.nickname.to_string(),
            username: self.username.to_string(),
            email: self.email.to_string(),
            email_verified: self.email_verified,
            picture: self.picture.to_string(),
            company: company.trim().to_string(),
            blog: self.blog.to_string(),
            phone: self.phone_number.to_string(),
            phone_verified: self.phone_verified,
            locale: self.locale.to_string(),
            login_provider: self.identities[0].provider.to_string(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            last_login: self.last_login,
            last_ip: self.last_ip.to_string(),
            logins_count: self.logins_count,
            link_to_people: Default::default(),
            last_application_accessed: Default::default(),
            link_to_auth_user_logins: Default::default(),
            link_to_page_views: Default::default(),
        }
    }
}

/// The data type for an Auth0 identity.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Identity {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub access_token: String,
    pub provider: String,
    pub user_id: String,
    pub connection: String,
    #[serde(rename = "isSocial")]
    pub is_social: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Token {
    pub access_token: String,
    pub token_type: String,
}

/// List users.
#[instrument(skip(db))]
#[inline]
pub async fn get_auth_users(domain: String, db: &Database) -> Vec<NewAuthUser> {
    let client = Client::new();
    // Get our token.
    let client_id = env::var("CIO_AUTH0_CLIENT_ID").unwrap();
    let client_secret = env::var("CIO_AUTH0_CLIENT_SECRET").unwrap();

    let mut map = HashMap::new();
    map.insert("client_id", client_id);
    map.insert("client_secret", client_secret);
    map.insert("audience", format!("https://{}.auth0.com/api/v2/", domain));
    map.insert("grant_type", "client_credentials".to_string());

    let resp = client.post(&format!("https://{}.auth0.com/oauth/token", domain)).json(&map).send().await.unwrap();

    let token: Token = resp.json().await.unwrap();

    let mut users: Vec<User> = Default::default();

    let rate_limit_sleep = time::Duration::from_millis(2000);

    let mut i: i32 = 0;
    let mut has_records = true;
    while has_records {
        let mut u = get_auth_users_page(&token.access_token, &domain, &i.to_string()).await;
        // We need to sleep here for a half second so we don't get rate limited.
        // https://auth0.com/docs/policies/rate-limit-policy
        // https://auth0.com/docs/policies/rate-limit-policy/management-api-endpoint-rate-limits
        thread::sleep(rate_limit_sleep);

        has_records = !u.is_empty();
        i += 1;

        users.append(&mut u);
    }

    let mut auth_users: Vec<NewAuthUser> = Default::default();
    for user in users {
        // Convert the user to an AuthUser.
        let mut auth_user = user.to_auth_user();

        // Get the application they last accessed.
        let auth_user_logins = get_auth_logs_for_user(&token.access_token, &domain, &user.user_id).await;

        // Get the first result.
        if !auth_user_logins.is_empty() {
            let first_result = auth_user_logins.get(0).unwrap();
            auth_user.last_application_accessed = first_result.client_name.to_string();
        }

        auth_users.push(auth_user);

        // We need to sleep here for a half second so we don't get rate limited.
        // https://auth0.com/docs/policies/rate-limit-policy
        // https://auth0.com/docs/policies/rate-limit-policy/management-api-endpoint-rate-limits
        thread::sleep(rate_limit_sleep);

        // Update our database with all the auth_user_logins.
        for mut auth_user_login in auth_user_logins {
            auth_user_login.email = user.email.to_string();
            db.upsert_auth_user_login(&auth_user_login);
        }
    }

    auth_users
}

// TODO: clean this all up to be an auth0 api library.
#[instrument]
#[inline]
async fn get_auth_logs_for_user(token: &str, domain: &str, user_id: &str) -> Vec<NewAuthUserLogin> {
    let client = Client::new();
    let resp = client
        .get(&format!("https://{}.auth0.com/api/v2/users/{}/logs", domain, user_id))
        .bearer_auth(token)
        .query(&[("sort", "date:-1"), ("per_page", "100")])
        .send()
        .await
        .unwrap();

    match resp.status() {
        StatusCode::OK => (),
        StatusCode::TOO_MANY_REQUESTS => {
            // Get the rate limit headers.
            let headers = resp.headers();
            let limit = headers.get("x-ratelimit-limit").unwrap().to_str().unwrap();
            let remaining = headers.get("x-ratelimit-remaining").unwrap().to_str().unwrap();
            let reset = headers.get("x-ratelimit-reset").unwrap().to_str().unwrap();
            let reset_int = reset.parse::<i64>().unwrap();

            // Convert the reset to a more sane number.
            let ts = DateTime::from_utc(NaiveDateTime::from_timestamp(reset_int, 0), Utc);
            let mut dur = ts - Utc::now();
            if dur.num_seconds() > 0 {
                dur = -dur;
            }
            let time = HumanTime::from(dur);

            println!("getting auth0 user logs failed because of rate limit: {}, remaining: {}, reset: {}", limit, remaining, time);

            return vec![];
        }
        s => {
            println!("getting auth0 user logs failed, status: {} | resp: {}", s, resp.text().await.unwrap(),);

            return vec![];
        }
    };

    resp.json::<Vec<NewAuthUserLogin>>().await.unwrap()
}

#[instrument]
#[inline]
async fn get_auth_users_page(token: &str, domain: &str, page: &str) -> Vec<User> {
    let client = Client::new();
    let resp = client
        .get(&format!("https://{}.auth0.com/api/v2/users", domain))
        .bearer_auth(token)
        .query(&[("per_page", "20"), ("page", page), ("sort", "last_login:-1")])
        .send()
        .await
        .unwrap();

    match resp.status() {
        StatusCode::OK => (),
        s => {
            println!("getting auth0 users failed, status: {} | resp: {}", s, resp.text().await.unwrap());

            return vec![];
        }
    };

    resp.json::<Vec<User>>().await.unwrap()
}

// Sync the auth_users with our database.
#[instrument]
#[inline]
pub async fn refresh_db_auth() {
    // Initialize our database.
    let db = Database::new();

    let auth_users = get_auth_users("oxide".to_string(), &db).await;

    // Sync auth users.
    for auth_user in auth_users {
        db.upsert_auth_user(&auth_user);
    }
}

#[cfg(test)]
mod tests {
    use crate::analytics::PageViews;
    use crate::auth_logins::refresh_db_auth;
    use crate::db::Database;
    use crate::models::{AuthUserLogins, AuthUsers};

    #[ignore]
    #[tokio::test(threaded_scheduler)]
    async fn test_cron_auth_refresh_db() {
        refresh_db_auth().await;
    }

    #[ignore]
    #[tokio::test(threaded_scheduler)]
    async fn test_cron_auth_users_airtable() {
        // Initialize our database.
        let db = Database::new();

        let auth_users = db.get_auth_users();
        // Update auth users in airtable.
        AuthUsers(auth_users).update_airtable().await;
    }

    #[ignore]
    #[tokio::test(threaded_scheduler)]
    async fn test_cron_auth_user_logins_airtable() {
        // Initialize our database.
        let db = Database::new();

        let auth_user_logins = db.get_auth_user_logins();
        // Update auth user logins in airtable.
        AuthUserLogins(auth_user_logins).update_airtable().await;

        // Update the auth users again after.
        let auth_users = db.get_auth_users();
        // Update auth users in airtable.
        AuthUsers(auth_users).update_airtable().await;

        let page_views = db.get_page_views();
        // Update auth user logins in airtable.
        PageViews(page_views).update_airtable().await;
    }
}
