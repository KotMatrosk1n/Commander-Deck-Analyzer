//! Optional Commander Spellbook enrichment.
//!
//! This module models the public `/estimate-bracket` contract observed in API
//! schema 5.6.5. It deliberately keeps Spellbook facts separate from the local
//! analyzer's conclusions: producing an infinite or otherwise unbounded result
//! is not, by itself, evidence that a line directly wins a multiplayer game.
//! Callers must retain mana, zone, and prerequisite requirements when deciding
//! whether a line is available or capable of converting into a win attempt.

use std::collections::BTreeMap;
use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

pub const SPELLBOOK_API_SCHEMA_OBSERVED: &str = "5.6.5";
pub const SPELLBOOK_ESTIMATE_ENDPOINT: &str =
    "https://backend.commanderspellbook.com/estimate-bracket";
pub const SPELLBOOK_CACHE_KEY_VERSION: &str = "spellbook-estimate-v1";

const SPELLBOOK_HOST: &str = "backend.commanderspellbook.com";
const SPELLBOOK_ESTIMATE_PATH: &str = "/estimate-bracket";
const USER_AGENT_VALUE: &str = concat!("CommanderDeckAnalyzer/", env!("CARGO_PKG_VERSION"));
const MAX_MAIN_ENTRIES: usize = 600;
const MAX_COMMANDER_ENTRIES: usize = 12;
const MAX_CARD_NAME_CHARS: usize = 256;
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum SpellbookError {
    #[error("Commander Spellbook enrichment received an invalid deck: {0}")]
    InvalidRequest(String),
    #[error("The Commander Spellbook endpoint did not pass the HTTPS allowlist.")]
    InvalidEndpoint,
    #[error("Commander Spellbook returned HTTP {0}.")]
    ProviderStatus(reqwest::StatusCode),
    #[error("Commander Spellbook returned an unexpected content type.")]
    UnexpectedContentType,
    #[error("Commander Spellbook returned more data than the analyzer accepts.")]
    ResponseTooLarge,
    #[error("Could not reach Commander Spellbook: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Commander Spellbook returned data that does not match its API contract: {0}")]
    InvalidResponse(#[from] serde_json::Error),
    #[error("The built-in Commander Spellbook endpoint is invalid: {0}")]
    EndpointUrl(#[from] url::ParseError),
}

/// A card quantity in either the command zone or main deck.
///
/// The upstream API accepts card names rather than Scryfall IDs. Construct a
/// request with [`SpellbookDeckRequest::from_sections`] so all entries receive
/// validation before any deck data is sent over the network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpellbookDeckEntry {
    pub card: String,
    pub quantity: u16,
}

impl SpellbookDeckEntry {
    pub fn new(card: impl Into<String>, quantity: u16) -> Self {
        Self {
            card: card.into(),
            quantity,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpellbookDeckRequest {
    pub main: Vec<SpellbookDeckEntry>,
    pub commanders: Vec<SpellbookDeckEntry>,
}

impl SpellbookDeckRequest {
    pub fn from_sections(
        commanders: impl IntoIterator<Item = SpellbookDeckEntry>,
        main: impl IntoIterator<Item = SpellbookDeckEntry>,
    ) -> Result<Self, SpellbookError> {
        let request = Self {
            main: main.into_iter().collect(),
            commanders: commanders.into_iter().collect(),
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), SpellbookError> {
        if self.commanders.is_empty() && self.main.is_empty() {
            return Err(SpellbookError::InvalidRequest(
                "at least one card is required".into(),
            ));
        }
        if self.commanders.len() > MAX_COMMANDER_ENTRIES {
            return Err(SpellbookError::InvalidRequest(format!(
                "the command zone may contain at most {MAX_COMMANDER_ENTRIES} entries"
            )));
        }
        if self.main.len() > MAX_MAIN_ENTRIES {
            return Err(SpellbookError::InvalidRequest(format!(
                "the main deck may contain at most {MAX_MAIN_ENTRIES} entries"
            )));
        }

        for entry in self.commanders.iter().chain(&self.main) {
            let name = entry.card.trim();
            if name.is_empty() {
                return Err(SpellbookError::InvalidRequest(
                    "card names cannot be empty".into(),
                ));
            }
            if name.chars().count() > MAX_CARD_NAME_CHARS {
                return Err(SpellbookError::InvalidRequest(format!(
                    "card names may contain at most {MAX_CARD_NAME_CHARS} characters"
                )));
            }
            if entry.quantity == 0 {
                return Err(SpellbookError::InvalidRequest(format!(
                    "“{name}” has a zero quantity"
                )));
            }
        }
        Ok(())
    }
}

/// A reusable, guarded client for optional live enrichment.
///
/// Redirects are disabled so the HTTPS host allowlist is not bypassed. The
/// request has both connection and total timeouts, and the response is read in
/// chunks so the byte limit remains effective without trusting Content-Length.
pub struct SpellbookClient {
    client: reqwest::Client,
    endpoint: Url,
}

impl SpellbookClient {
    pub fn new() -> Result<Self, SpellbookError> {
        let endpoint = Url::parse(SPELLBOOK_ESTIMATE_ENDPOINT)?;
        validate_estimate_endpoint(&endpoint)?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()?;
        Ok(Self { client, endpoint })
    }

    pub async fn estimate_bracket(
        &self,
        request: &SpellbookDeckRequest,
    ) -> Result<EstimateBracketResponse, SpellbookError> {
        request.validate()?;
        validate_estimate_endpoint(&self.endpoint)?;

        let mut response = self
            .client
            .post(self.endpoint.clone())
            .header(USER_AGENT, USER_AGENT_VALUE)
            .header(ACCEPT, "application/json")
            .json(request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SpellbookError::ProviderStatus(response.status()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(SpellbookError::ResponseTooLarge);
        }
        let is_json = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                let media_type = value
                    .split(';')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase();
                media_type == "application/json" || media_type.ends_with("+json")
            });
        if !is_json {
            return Err(SpellbookError::UnexpectedContentType);
        }

        let mut bytes = Vec::with_capacity(
            response
                .content_length()
                .unwrap_or_default()
                .min(MAX_RESPONSE_BYTES as u64) as usize,
        );
        while let Some(chunk) = response.chunk().await? {
            let next_length = bytes
                .len()
                .checked_add(chunk.len())
                .ok_or(SpellbookError::ResponseTooLarge)?;
            if next_length > MAX_RESPONSE_BYTES {
                return Err(SpellbookError::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }

        Ok(serde_json::from_slice(&bytes)?)
    }
}

/// Allows only the one published endpoint used by this module.
///
/// This check is intentionally stricter than a suffix check: alternate hosts,
/// user-info tricks, nonstandard ports, redirects, query strings, fragments,
/// and adjacent paths are all rejected.
pub fn validate_estimate_endpoint(endpoint: &Url) -> Result<(), SpellbookError> {
    let valid = endpoint.scheme() == "https"
        && endpoint
            .host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case(SPELLBOOK_HOST))
        && endpoint.port_or_known_default() == Some(443)
        && endpoint.username().is_empty()
        && endpoint.password().is_none()
        && endpoint.path() == SPELLBOOK_ESTIMATE_PATH
        && endpoint.query().is_none()
        && endpoint.fragment().is_none();
    if valid {
        Ok(())
    } else {
        Err(SpellbookError::InvalidEndpoint)
    }
}

/// Returns an order- and case-insensitive cache key for one upstream revision.
///
/// `upstream_revision` should be the live API schema/release identifier or a
/// local snapshot's content hash/ETag. Including it prevents results from being
/// reused after the underlying combo catalog changes.
pub fn estimate_cache_key(request: &SpellbookDeckRequest, upstream_revision: &str) -> String {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, SPELLBOOK_CACHE_KEY_VERSION.as_bytes());
    hash_field(&mut hasher, upstream_revision.trim().as_bytes());
    hash_section(&mut hasher, b"commanders", &request.commanders);
    hash_section(&mut hasher, b"main", &request.main);
    let digest = hasher.finalize();
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{SPELLBOOK_CACHE_KEY_VERSION}:{hex}")
}

fn hash_section(hasher: &mut Sha256, section: &[u8], entries: &[SpellbookDeckEntry]) {
    hash_field(hasher, section);
    let mut grouped = BTreeMap::<String, u64>::new();
    for entry in entries {
        let normalized = entry.card.trim().to_lowercase();
        grouped
            .entry(normalized)
            .and_modify(|quantity| *quantity += u64::from(entry.quantity))
            .or_insert(u64::from(entry.quantity));
    }
    for (name, quantity) in grouped {
        hash_field(hasher, name.as_bytes());
        hasher.update(quantity.to_le_bytes());
    }
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

/// Spellbook's classification tag is intentionally retained verbatim. It is a
/// service-specific tag (for example `R` or `S`), not this app's 1-5 bracket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct SpellbookBracketTag(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateBracketResponse {
    pub bracket_tag: SpellbookBracketTag,
    pub cards: Vec<ClassifiedCard>,
    pub templates: Vec<ClassifiedTemplate>,
    pub combos: Vec<ClassifiedVariant>,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifiedCard {
    pub card: SpellbookCard,
    pub quantity: u32,
    pub banned: bool,
    pub game_changer: bool,
    pub mass_land_denial: bool,
    pub extra_turn: bool,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifiedTemplate {
    pub template: SpellbookTemplate,
    pub quantity: u32,
    pub mass_land_denial: bool,
    pub extra_turn: bool,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifiedVariant {
    pub combo: SpellbookVariant,
    pub relevant: bool,
    pub borderline_relevant: bool,
    pub arguably_two_card: bool,
    pub definitely_two_card: bool,
    pub speed: i32,
    pub mass_land_denial: bool,
    pub extra_turn: bool,
    pub lock: bool,
    pub skip_turns: bool,
    pub control_all_opponents: bool,
    pub control_some_opponents: bool,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellbookVariant {
    pub id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub uses: Vec<CardPiece>,
    #[serde(default)]
    pub requires: Vec<TemplatePiece>,
    #[serde(default)]
    pub produces: Vec<ProducedFeature>,
    #[serde(default)]
    pub identity: String,
    #[serde(default)]
    pub mana_needed: String,
    #[serde(default)]
    pub mana_value_needed: u32,
    #[serde(default)]
    pub easy_prerequisites: String,
    #[serde(default)]
    pub notable_prerequisites: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub popularity: Option<u64>,
    #[serde(default)]
    pub spoiler: bool,
    #[serde(default)]
    pub bracket_tag: Option<SpellbookBracketTag>,
    #[serde(default)]
    pub variant_count: u32,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

impl SpellbookVariant {
    /// Reports an unbounded/infinite result without asserting that it wins.
    ///
    /// Some unbounded results need another payoff, a legal target, a combat
    /// step, or other conversion condition. Consumers must inspect `produces`
    /// and all prerequisites rather than treating this as a direct-win flag.
    pub fn produces_unbounded_result(&self) -> bool {
        self.produces.iter().any(|result| {
            result.feature.uncountable
                || result
                    .feature
                    .name
                    .to_ascii_lowercase()
                    .contains("infinite")
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardPiece {
    pub card: SpellbookCard,
    #[serde(default)]
    pub zone_locations: Vec<String>,
    #[serde(default)]
    pub battlefield_card_state: String,
    #[serde(default)]
    pub exile_card_state: String,
    #[serde(default)]
    pub library_card_state: String,
    #[serde(default)]
    pub graveyard_card_state: String,
    #[serde(default)]
    pub must_be_commander: bool,
    pub quantity: u32,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePiece {
    pub template: SpellbookTemplate,
    #[serde(default)]
    pub zone_locations: Vec<String>,
    #[serde(default)]
    pub battlefield_card_state: String,
    #[serde(default)]
    pub exile_card_state: String,
    #[serde(default)]
    pub library_card_state: String,
    #[serde(default)]
    pub graveyard_card_state: String,
    #[serde(default)]
    pub must_be_commander: bool,
    pub quantity: u32,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProducedFeature {
    pub feature: SpellbookFeature,
    pub quantity: u32,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellbookFeature {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub uncountable: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellbookCard {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub oracle_id: Option<String>,
    #[serde(default)]
    pub spoiler: bool,
    #[serde(default)]
    pub type_line: String,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellbookTemplate {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub scryfall_query: Option<String>,
    #[serde(default, flatten)]
    pub additional: BTreeMap<String, serde_json::Value>,
}
