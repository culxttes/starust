use std::fs::read_to_string;
use std::{borrow::Cow, fmt::Write};

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::StatusCode;
use reqwest::{
    Client,
    header::{ACCEPT, AUTHORIZATION, HeaderValue, USER_AGENT},
};
use serde::{Deserialize, Serialize};

const NB_PAGES: u8 = 10;
const NB_PER_PAGE: u8 = 100;

#[derive(Debug, Deserialize)]
struct Config {
    token: String,
}

/// /search/repositories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSearchResponse {
    pub items: Vec<RepoSearchResultItem>,
}

/// "Repo Search Result Item"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RepoSearchResultItem {
    pub name: String,
    pub owner: Option<SimpleUser>,
}

/// "Simple User"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SimpleUser {
    pub login: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config: Config = toml::from_str(&read_to_string("config.toml")?)?;
    let client: Client = Client::builder()
        .default_headers(
            [
                (
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", config.token))?,
                ),
                (
                    ACCEPT,
                    HeaderValue::from_static("application/vnd.github+json"),
                ),
                (USER_AGENT, HeaderValue::from_static("starust")),
            ]
            .into_iter()
            .collect(),
        )
        .build()?;

    let mut query: [(&str, Cow<'static, str>); 4] = [
        ("language", "Rust".into()),
        ("per_page", NB_PER_PAGE.to_string().into()),
        ("page", String::new().into()),
        ("q", "Rust language".into()),
    ];

    let nb_star: ProgressBar =
        ProgressBar::new(NB_PAGES as u64 * NB_PER_PAGE as u64).with_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )?
            .progress_chars("##-"),
        );

    nb_star.inc(0);

    let mut tasks = FuturesUnordered::new();
    for page in 0..NB_PAGES {
        query[2].1.to_mut().clear();
        write!(&mut query[2].1.to_mut(), "{}", page)?;
        let res: RepoSearchResponse = client
            .get("https://api.github.com/search/repositories")
            .query(&query)
            .send()
            .await?
            .json()
            .await?;

        for repo in res.items {
            if let Some(owner) = repo.owner {
                let url = format!(
                    "https://api.github.com/user/starred/{}/{}",
                    owner.login, repo.name
                );
                let client = client.clone();
                tasks.push(async move {
                    let res = client.put(&url).send().await?;
                    Ok::<_, reqwest::Error>(res)
                });
            }
        }

        while let Some(res) = tasks.next().await {
            match res {
                Ok(r) if r.status() == StatusCode::NO_CONTENT => nb_star.inc(1),
                Ok(r) => eprintln!("{}", r.text().await.unwrap_or_default()),
                Err(e) => eprintln!("Error: {e}"),
            }
        }
    }

    Ok(())
}
