use std::fs::read_to_string;
use std::{borrow::Cow, fmt::Write};

use chrono::{DateTime, Utc};
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
    pub total_count: u64,
    pub incomplete_results: bool,
    pub items: Vec<RepoSearchResultItem>,
}

/// "Repo Search Result Item"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RepoSearchResultItem {
    pub id: u64,
    pub node_id: String,
    pub name: String,
    pub full_name: String,

    pub owner: Option<SimpleUser>,

    pub private: bool,
    pub html_url: String,
    pub description: Option<String>,
    pub fork: bool,
    pub url: String,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub pushed_at: DateTime<Utc>,

    pub homepage: Option<String>,
    pub size: u64,
    pub stargazers_count: u64,
    pub watchers_count: u64,
    pub language: Option<String>,
    pub forks_count: u64,
    pub open_issues_count: u64,

    pub master_branch: Option<String>,
    pub default_branch: String,
    pub score: f64,

    pub forks_url: String,
    pub keys_url: String,
    pub collaborators_url: String,
    pub teams_url: String,
    pub hooks_url: String,
    pub issue_events_url: String,
    pub events_url: String,
    pub assignees_url: String,
    pub branches_url: String,
    pub tags_url: String,
    pub blobs_url: String,
    pub git_tags_url: String,
    pub git_refs_url: String,
    pub trees_url: String,
    pub statuses_url: String,
    pub languages_url: String,
    pub stargazers_url: String,
    pub contributors_url: String,
    pub subscribers_url: String,
    pub subscription_url: String,
    pub commits_url: String,
    pub git_commits_url: String,
    pub comments_url: String,
    pub issue_comment_url: String,
    pub contents_url: String,
    pub compare_url: String,
    pub merges_url: String,
    pub archive_url: String,
    pub downloads_url: String,
    pub issues_url: String,
    pub pulls_url: String,
    pub milestones_url: String,
    pub notifications_url: String,
    pub labels_url: String,
    pub releases_url: String,
    pub deployments_url: String,

    pub git_url: String,
    pub ssh_url: String,
    pub clone_url: String,
    pub svn_url: String,

    pub forks: u64,
    pub open_issues: u64,
    pub watchers: u64,

    pub topics: Vec<String>,
    pub mirror_url: Option<String>,

    pub has_issues: bool,
    pub has_projects: bool,
    pub has_pages: bool,
    pub has_wiki: bool,
    pub has_downloads: bool,
    pub has_discussions: bool,

    pub archived: bool,
    pub disabled: bool,

    /// "public" | "private" | "internal"
    pub visibility: String,

    pub license: Option<LicenseSimple>,

    pub permissions: Option<RepoPermissions>,

    pub text_matches: Option<Vec<SearchResultTextMatch>>,

    pub temp_clone_token: Option<String>,
    pub allow_merge_commit: Option<bool>,
    pub allow_squash_merge: Option<bool>,
    pub allow_rebase_merge: Option<bool>,
    pub allow_auto_merge: Option<bool>,
    pub delete_branch_on_merge: Option<bool>,
    pub allow_forking: Option<bool>,
    pub is_template: Option<bool>,
    pub web_commit_signoff_required: Option<bool>,
}

/// "Simple User"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SimpleUser {
    pub name: Option<String>,
    pub email: Option<String>,

    pub login: String,
    pub id: u64,
    pub node_id: String,

    pub avatar_url: String,
    pub gravatar_id: Option<String>,

    pub url: String,
    pub html_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub gists_url: String,
    pub starred_url: String,
    pub subscriptions_url: String,
    pub organizations_url: String,
    pub repos_url: String,
    pub events_url: String,
    pub received_events_url: String,

    /// "User" / "Organization" etc.
    #[serde(rename = "type")]
    pub type_field: String,

    pub site_admin: bool,

    pub starred_at: Option<String>,

    pub user_view_type: Option<String>,
}

/// "License Simple"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LicenseSimple {
    pub key: String,
    pub name: String,
    pub url: Option<String>,
    pub spdx_id: Option<String>,
    pub node_id: String,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RepoPermissions {
    pub admin: bool,
    pub maintain: Option<bool>,
    pub push: bool,
    pub triage: Option<bool>,
    pub pull: bool,
}

/// "Search Result Text Matches"
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SearchResultTextMatch {
    pub object_url: String,
    pub object_type: Option<String>,
    pub property: String,
    pub fragment: String,
    pub matches: Vec<TextMatchSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TextMatchSegment {
    pub text: String,
    pub indices: Vec<u64>,
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

    let mut url: String = String::new();

    let nb_star: ProgressBar =
        ProgressBar::new(NB_PAGES as u64 * NB_PER_PAGE as u64).with_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )?
            .progress_chars("##-"),
        );

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
            if repo.owner.is_none() {
                eprintln!("url: {}", repo.url);
                continue;
            }
            url.clear();
            write!(
                &mut url,
                "https://api.github.com/user/starred/{}/{}",
                repo.owner.expect("Wsh il y a pas de personne").login,
                repo.name
            )?;
            let res: reqwest::Response = client.put(&url).send().await?;
            if res.status() != StatusCode::NO_CONTENT {
                eprintln!("{}", res.text().await?);
            } else {
                nb_star.inc(1);
            }
        }
    }

    Ok(())
}
