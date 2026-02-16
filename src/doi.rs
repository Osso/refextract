use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::Deserialize;

use crate::types::ParsedReference;

pub struct DoiCache {
    conn: Connection,
}

#[derive(Deserialize)]
struct CrossRefResponse {
    message: CrossRefMessage,
}

#[derive(Deserialize)]
struct CrossRefMessage {
    items: Vec<CrossRefItem>,
}

#[derive(Deserialize)]
struct CrossRefItem {
    #[serde(rename = "DOI")]
    doi: String,
}

enum LookupOutcome {
    Found(String),
    NotFound,
    Skipped, // transient error, don't cache
}

impl DoiCache {
    pub fn open() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .context("Could not determine cache directory")?
            .join("refextract");
        std::fs::create_dir_all(&cache_dir)?;
        let db_path = cache_dir.join("doi_cache.db");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS doi_cache (
                key TEXT PRIMARY KEY,
                doi TEXT,
                created_at INTEGER NOT NULL
            )",
        )?;
        Ok(Self { conn })
    }

    /// None = not cached, Some(None) = negative hit, Some(Some(doi)) = cached DOI.
    pub fn get(&self, key: &str) -> Result<Option<Option<String>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT doi FROM doi_cache WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn put(&self, key: &str, doi: Option<&str>) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        self.conn.execute(
            "INSERT OR REPLACE INTO doi_cache (key, doi, created_at) VALUES (?1, ?2, ?3)",
            params![key, doi, now],
        )?;
        Ok(())
    }
}

fn query_crossref(terms: &str) -> LookupOutcome {
    let url = format!(
        "https://api.crossref.org/works?query.bibliographic={}&rows=1&select=DOI&mailto=adeiana@gmail.com",
        terms.replace(' ', "+")
    );
    let resp = match ureq::get(&url).call() {
        Ok(resp) => resp,
        Err(_) => return LookupOutcome::Skipped,
    };
    if resp.status() == 429 {
        return LookupOutcome::Skipped;
    }
    if resp.status() != 200 {
        return LookupOutcome::NotFound;
    }
    let body = match resp.into_body().read_to_string() {
        Ok(b) => b,
        Err(_) => return LookupOutcome::Skipped,
    };
    deserialize_crossref(&body)
}

fn deserialize_crossref(body: &str) -> LookupOutcome {
    match serde_json::from_str::<CrossRefResponse>(body) {
        Ok(data) => match data.message.items.into_iter().next() {
            Some(item) => LookupOutcome::Found(item.doi),
            None => LookupOutcome::NotFound,
        },
        Err(_) => LookupOutcome::NotFound,
    }
}

fn lookup_cached_or_fetch(cache: &DoiCache, key: &str, terms: &str) -> Option<String> {
    if let Ok(Some(cached)) = cache.get(key) {
        return cached;
    }
    match query_crossref(terms) {
        LookupOutcome::Found(doi) => {
            let _ = cache.put(key, Some(&doi));
            Some(doi)
        }
        LookupOutcome::NotFound => {
            let _ = cache.put(key, None);
            None
        }
        LookupOutcome::Skipped => None,
    }
}

pub fn enrich_dois(refs: &mut [ParsedReference], cache: &DoiCache) {
    let total = refs.iter().filter(|r| r.doi.is_none()).count();
    let mut done = 0;
    for r in refs.iter_mut() {
        if r.doi.is_some() {
            continue;
        }
        done += 1;
        eprint!("\rLooking up DOIs: {done}/{total}");
        if try_journal_lookup(r, cache) {
            continue;
        }
        try_arxiv_lookup(r, cache);
    }
    if total > 0 {
        eprintln!();
    }
}

fn try_journal_lookup(r: &mut ParsedReference, cache: &DoiCache) -> bool {
    let (Some(journal), Some(volume), Some(page)) =
        (&r.journal_title, &r.journal_volume, &r.journal_page)
    else {
        return false;
    };
    let key = format!("j:{journal}|v:{volume}|p:{page}");
    let terms = format!("{journal} {volume} {page}");
    if let Some(doi) = lookup_cached_or_fetch(cache, &key, &terms) {
        r.doi = Some(doi);
        return true;
    }
    false
}

fn try_arxiv_lookup(r: &mut ParsedReference, cache: &DoiCache) -> bool {
    let Some(arxiv_id) = &r.arxiv_id else {
        return false;
    };
    let key = format!("arxiv:{arxiv_id}");
    let terms = format!("arXiv {arxiv_id}");
    if let Some(doi) = lookup_cached_or_fetch(cache, &key, &terms) {
        r.doi = Some(doi);
        return true;
    }
    false
}
