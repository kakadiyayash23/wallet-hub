//! Markets per sport: The Odds API when ODDS_API_KEY is set (cached 5 minutes
//! per sport, free-tier friendly), built-in fixtures otherwise.

use domain::traits::{Market, Selection};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// The sports we expose, in rail order. Each maps to one or more Odds API keys.
pub const SPORTS: &[(&str, &str, &[&str])] = &[
    (
        "soccer",
        "Soccer",
        &[
            "soccer_epl",
            "soccer_spain_la_liga",
            "soccer_germany_bundesliga",
            "soccer_italy_serie_a",
        ],
    ),
    (
        "cricket",
        "Cricket",
        &[
            "cricket_ipl",
            "cricket_international_t20",
            "cricket_test_match",
            "cricket_big_bash",
        ],
    ),
    (
        "basketball",
        "Basketball",
        &["basketball_nba", "basketball_euroleague"],
    ),
    (
        "tennis",
        "Tennis",
        &[
            "tennis_atp_us_open",
            "tennis_wta_us_open",
            "tennis_atp_french_open",
            "tennis_wta_french_open",
        ],
    ),
];

pub fn sport_label(key: &str) -> Option<&'static str> {
    SPORTS
        .iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, l, _)| *l)
}

pub struct OddsCache {
    api_key: Option<String>,
    inner: Mutex<HashMap<String, (Instant, Vec<Market>)>>,
}

#[derive(Deserialize)]
struct OddsEvent {
    id: String,
    sport_key: String,
    sport_title: String,
    commence_time: String,
    home_team: String,
    away_team: String,
    #[serde(default)]
    bookmakers: Vec<Bookmaker>,
}

#[derive(Deserialize)]
struct Bookmaker {
    #[serde(default)]
    markets: Vec<BkMarket>,
}

#[derive(Deserialize)]
struct BkMarket {
    key: String,
    #[serde(default)]
    outcomes: Vec<Outcome>,
}

#[derive(Deserialize)]
struct Outcome {
    name: String,
    price: f64,
}

impl OddsCache {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub async fn markets(
        &self,
        http: &reqwest::Client,
        sport: &str,
    ) -> anyhow::Result<Vec<Market>> {
        if sport_label(sport).is_none() {
            anyhow::bail!("unknown sport {sport}");
        }
        let ttl = if self.api_key.is_some() {
            Duration::from_secs(300)
        } else {
            Duration::from_secs(60)
        };
        {
            let guard = self.inner.lock().await;
            if let Some((at, cached)) = guard.get(sport) {
                if at.elapsed() < ttl {
                    return Ok(cached.clone());
                }
            }
        }
        let mut markets = match &self.api_key {
            Some(key) => match self.fetch(http, key, sport).await {
                Ok(m) if !m.is_empty() => m,
                Ok(_) => fixtures(sport),
                Err(e) => {
                    tracing::warn!(error = %e, sport, "odds api failed, using fixtures");
                    fixtures(sport)
                }
            },
            None => fixtures(sport),
        };
        markets.sort_by(|a, b| a.starts_at.cmp(&b.starts_at));
        self.inner
            .lock()
            .await
            .insert(sport.to_string(), (Instant::now(), markets.clone()));
        Ok(markets)
    }

    /// Find one event across every sport. Used when a bet comes in.
    pub async fn find(&self, http: &reqwest::Client, event_id: &str) -> Option<Market> {
        for (sport, _, _) in SPORTS {
            if let Ok(markets) = self.markets(http, sport).await {
                if let Some(m) = markets.into_iter().find(|m| m.event_id == event_id) {
                    return Some(m);
                }
            }
        }
        None
    }

    async fn fetch(
        &self,
        http: &reqwest::Client,
        key: &str,
        sport: &str,
    ) -> anyhow::Result<Vec<Market>> {
        let keys = SPORTS
            .iter()
            .find(|(k, _, _)| *k == sport)
            .map(|(_, _, keys)| *keys)
            .unwrap_or(&[]);
        let mut out = Vec::new();
        for sport_key in keys {
            let url = format!("https://api.the-odds-api.com/v4/sports/{sport_key}/odds");
            let res = http
                .get(url)
                .query(&[
                    ("apiKey", key),
                    ("regions", "eu"),
                    ("markets", "h2h"),
                    ("oddsFormat", "decimal"),
                ])
                .send()
                .await?;
            if !res.status().is_success() {
                // Out of season or unknown key: skip this league, keep the others.
                tracing::debug!(sport_key, status = %res.status(), "odds api league skipped");
                continue;
            }
            let events: Vec<OddsEvent> = res.json().await?;
            out.extend(events.into_iter().filter_map(|e| to_market(e, sport)));
        }
        out.truncate(40);
        Ok(out)
    }
}

fn to_market(e: OddsEvent, sport: &str) -> Option<Market> {
    let h2h = e
        .bookmakers
        .iter()
        .flat_map(|b| b.markets.iter())
        .find(|m| m.key == "h2h")?;
    let selections = h2h
        .outcomes
        .iter()
        .map(|o| {
            let key = if o.name == e.home_team {
                "home"
            } else if o.name == e.away_team {
                "away"
            } else {
                "draw"
            };
            Selection {
                key: key.into(),
                label: o.name.clone(),
                odds_milli: (o.price * 1000.0).round() as i64,
            }
        })
        .collect();
    let _ = e.sport_key;
    Some(Market {
        event_id: e.id,
        sport: sport.to_string(),
        league: e.sport_title,
        home: e.home_team,
        away: e.away_team,
        starts_at: e.commence_time,
        selections,
    })
}

/// Built-in upcoming matches, three-way for soccer and two-way elsewhere.
pub fn fixtures(sport: &str) -> Vec<Market> {
    let now = chrono::Utc::now();
    let starts = |hours: i64| (now + chrono::Duration::hours(hours)).to_rfc3339();
    let three =
        |id: &str, league: &str, home: &str, away: &str, h: i64, d: i64, a: i64, hours: i64| {
            Market {
                event_id: format!("fx_{sport}_{id}"),
                sport: sport.to_string(),
                league: league.into(),
                home: home.into(),
                away: away.into(),
                starts_at: starts(hours),
                selections: vec![
                    Selection {
                        key: "home".into(),
                        label: home.into(),
                        odds_milli: h,
                    },
                    Selection {
                        key: "draw".into(),
                        label: "Draw".into(),
                        odds_milli: d,
                    },
                    Selection {
                        key: "away".into(),
                        label: away.into(),
                        odds_milli: a,
                    },
                ],
            }
        };
    let two = |id: &str, league: &str, home: &str, away: &str, h: i64, a: i64, hours: i64| Market {
        event_id: format!("fx_{sport}_{id}"),
        sport: sport.to_string(),
        league: league.into(),
        home: home.into(),
        away: away.into(),
        starts_at: starts(hours),
        selections: vec![
            Selection {
                key: "home".into(),
                label: home.into(),
                odds_milli: h,
            },
            Selection {
                key: "away".into(),
                label: away.into(),
                odds_milli: a,
            },
        ],
    };
    match sport {
        "soccer" => vec![
            three(
                "1",
                "Premier League",
                "Arsenal",
                "Liverpool",
                2100,
                3400,
                3300,
                26,
            ),
            three(
                "2",
                "Premier League",
                "Chelsea",
                "Tottenham",
                1950,
                3600,
                3700,
                28,
            ),
            three(
                "3",
                "Premier League",
                "Manchester City",
                "Newcastle",
                1400,
                4800,
                7000,
                50,
            ),
            three(
                "4",
                "Bundesliga",
                "Bayern München",
                "Borussia Dortmund",
                1600,
                4200,
                5000,
                30,
            ),
            three(
                "5",
                "Bundesliga",
                "Bayer Leverkusen",
                "RB Leipzig",
                2050,
                3500,
                3400,
                54,
            ),
            three(
                "6",
                "La Liga",
                "Real Madrid",
                "Sevilla",
                1450,
                4600,
                6500,
                52,
            ),
            three(
                "7",
                "La Liga",
                "Barcelona",
                "Atlético Madrid",
                1800,
                3700,
                4200,
                76,
            ),
            three("8", "Serie A", "Inter", "Juventus", 2250, 3100, 3200, 55),
            three("9", "Ligue 1", "PSG", "Marseille", 1350, 5200, 7500, 74),
        ],
        "cricket" => vec![
            two(
                "1",
                "IPL",
                "Mumbai Indians",
                "Chennai Super Kings",
                1850,
                1950,
                8,
            ),
            two(
                "2",
                "IPL",
                "Royal Challengers Bengaluru",
                "Kolkata Knight Riders",
                1800,
                2000,
                32,
            ),
            two(
                "3",
                "IPL",
                "Rajasthan Royals",
                "Gujarat Titans",
                2050,
                1750,
                56,
            ),
            two(
                "4",
                "T20 Internationals",
                "India",
                "Australia",
                1650,
                2250,
                20,
            ),
            two(
                "5",
                "T20 Internationals",
                "England",
                "Pakistan",
                1750,
                2050,
                44,
            ),
            two(
                "6",
                "Test Matches",
                "South Africa",
                "New Zealand",
                1900,
                1900,
                70,
            ),
        ],
        "basketball" => vec![
            two(
                "1",
                "NBA",
                "Los Angeles Lakers",
                "Golden State Warriors",
                1750,
                2100,
                10,
            ),
            two("2", "NBA", "Boston Celtics", "Miami Heat", 1450, 2800, 12),
            two("3", "NBA", "Denver Nuggets", "Phoenix Suns", 1650, 2300, 34),
            two(
                "4",
                "EuroLeague",
                "Real Madrid",
                "Olympiacos",
                1600,
                2350,
                22,
            ),
            two("5", "EuroLeague", "Fenerbahçe", "Barcelona", 1900, 1900, 46),
        ],
        "tennis" => vec![
            two(
                "1",
                "ATP Masters",
                "Carlos Alcaraz",
                "Jannik Sinner",
                1900,
                1900,
                6,
            ),
            two(
                "2",
                "ATP Masters",
                "Novak Djokovic",
                "Daniil Medvedev",
                1550,
                2450,
                9,
            ),
            two(
                "3",
                "WTA 1000",
                "Iga Świątek",
                "Aryna Sabalenka",
                1800,
                2000,
                7,
            ),
            two(
                "4",
                "WTA 1000",
                "Coco Gauff",
                "Elena Rybakina",
                2100,
                1750,
                31,
            ),
        ],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sport_has_fixtures_with_valid_odds() {
        for (sport, _, _) in SPORTS {
            let f = fixtures(sport);
            assert!(!f.is_empty(), "{sport}");
            for m in f {
                assert_eq!(m.sport, *sport);
                assert!(m.selections.len() == 2 || m.selections.len() == 3);
                assert!(m.selections.iter().all(|s| s.odds_milli > 1000));
            }
        }
    }
}
