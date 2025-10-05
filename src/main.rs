use std::fs::read_to_string;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::StatusCode;
use reqwest::{
    Client,
    header::{ACCEPT, AUTHORIZATION, HeaderValue, USER_AGENT},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    token: String,
}

/// /search/repositories
#[derive(Debug, Deserialize)]
pub struct RepoSearchResponse {
    pub items: Vec<RepoSearchResultItem>,
}

/// "Repo Search Result Item"
#[derive(Debug, Deserialize)]
pub struct RepoSearchResultItem {
    pub name: String,
    pub owner: Option<SimpleUser>,
}

/// "Simple User"
#[derive(Debug, Deserialize)]
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

    const NB_PER_PAGE: u8 = 100;

    let query: [(&str, &str); 3] = [
        ("language", "Rust"),
        ("per_page", &NB_PER_PAGE.to_string()),
        ("q", "Rust language"),
    ];

    let mut search_tasks = FuturesUnordered::new();
    let mut star_tasks = FuturesUnordered::new();

    println!("Scraping repos...");

    const NB_PAGES: u8 = 10;

    for page in 0..NB_PAGES {
        let client = client.clone();
        search_tasks.push(async move {
            let res: RepoSearchResponse = client
                .get("https://api.github.com/search/repositories")
                .query(&query)
                .query(&[("page", page.to_string())])
                .send()
                .await?
                .json()
                .await?;
            Ok::<_, reqwest::Error>(res.items)
        });
    }

    while let Some(result) = search_tasks.next().await {
        match result {
            Ok(repos) => {
                for repo in repos {
                    if let Some(owner) = repo.owner {
                        let client = client.clone();
                        let url = format!(
                            "https://api.github.com/user/starred/{}/{}",
                            owner.login, repo.name
                        );
                        star_tasks.push(async move {
                            let res = client.put(&url).send().await?;
                            Ok::<_, reqwest::Error>(res)
                        });
                    }
                }
            }
            Err(e) => eprintln!("Search error: {e}"),
        }
    }

    println!("Starring repos...");

    let nb_star: ProgressBar =
        ProgressBar::new(NB_PAGES as u64 * NB_PER_PAGE as u64).with_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )?
            .progress_chars("##-"),
        );

    nb_star.inc(0);

    while let Some(res) = star_tasks.next().await {
        match res {
            Ok(r) if r.status() == StatusCode::NO_CONTENT => nb_star.inc(1),
            Ok(r) => eprintln!("{}", r.text().await?),
            Err(e) => eprintln!("Star error: {e}"),
        }
    }

    Ok(())
}
