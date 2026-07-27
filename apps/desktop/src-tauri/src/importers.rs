use std::collections::HashMap;
use std::sync::LazyLock;

use chrono::Utc;
use regex::Regex;
use reqwest::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use serde::Deserialize;
use url::Url;

use crate::domain::ImportResult;

const USER_AGENT_VALUE: &str = concat!("CommanderDeckAnalyzer/", env!("CARGO_PKG_VERSION"));
const MAX_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const MAX_IMPORTED_ENTRIES: usize = 1_000;
const MAX_IMPORTED_TOTAL_CARDS: u32 = 1_000;
const MAX_IMPORTED_CARD_QUANTITY: u32 = 1_000;
const MAX_IMPORTED_COMMANDERS: u32 = 12;
const MAX_IMPORTED_CARD_NAME_CHARS: usize = 256;

static ARCHIDEKT_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^/decks/(?P<id>[1-9][0-9]*)(?:/[^/?#]+)?/?$").expect("valid Archidekt path")
});
static DECKSTATS_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^/decks/(?P<owner>[1-9][0-9]*)/(?P<deck>[1-9][0-9]*)(?:-[^/]*)?(?:/[A-Za-z]{2})?/?$",
    )
    .expect("valid Deckstats path")
});
static SCRYFALL_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^/@[^/]+/decks/(?P<id>[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12})/?$")
        .expect("valid Scryfall deck path")
});
static MOXFIELD_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^/decks/[A-Za-z0-9_-]{6,128}/?$").expect("valid Moxfield deck path")
});
static MTGGOLDFISH_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/deck/[1-9][0-9]*/?$").expect("valid MTGGoldfish deck path"));
static TAPPEDOUT_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^/mtg-decks/[A-Za-z0-9_%.-]{1,200}/?$").expect("valid TappedOut deck path")
});
static MANABOX_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^/decks/[A-Za-z0-9_-]{6,128}/?$").expect("valid ManaBox deck path")
});
static ARCHIDEKT_ENDPOINT_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^/api/decks/[1-9][0-9]*/$").expect("valid Archidekt endpoint path")
});
static SCRYFALL_ENDPOINT_PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^/decks/[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}/export/csv$")
        .expect("valid Scryfall endpoint path")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceProvider {
    Archidekt,
    Deckstats,
    Scryfall,
    MoxfieldManual,
    TappedOutManual,
    ManaBoxManual,
    MtgGoldfishManual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndpointPolicy {
    Archidekt,
    Deckstats,
    Scryfall,
}

impl EndpointPolicy {
    const fn accept(self) -> &'static str {
        match self {
            Self::Archidekt | Self::Deckstats => "application/json",
            Self::Scryfall => "text/csv",
        }
    }

    fn accepts_content_type(self, content_type: &str) -> bool {
        content_type.is_empty()
            || match self {
                Self::Archidekt | Self::Deckstats => {
                    content_type.contains("application/json") || content_type.contains("text/json")
                }
                Self::Scryfall => {
                    content_type.contains("text/csv")
                        || content_type.contains("application/csv")
                        || content_type.contains("application/octet-stream")
                }
            }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Enter a complete HTTPS deck URL.")]
    InvalidUrl,
    #[error(
        "This URL is not a supported public deck URL. Supported direct imports: Archidekt, Deckstats, and Scryfall Decks."
    )]
    UnsupportedUrl,
    #[error("{0}")]
    ManualOnly(String),
    #[error(
        "The deck provider returned {0}. The deck may be private, deleted, or temporarily unavailable."
    )]
    ProviderStatus(reqwest::StatusCode),
    #[error("The provider returned an unexpectedly large response.")]
    ResponseTooLarge,
    #[error(
        "The provider returned a challenge page instead of deck data. Export the deck as text in your browser and paste it here."
    )]
    ChallengePage,
    #[error("The deck provider changed its export format or returned invalid data: {0}")]
    InvalidResponse(String),
    #[error("Could not reach the deck provider: {0}")]
    Network(#[from] reqwest::Error),
}

pub async fn import_deck_url(input: &str) -> Result<ImportResult, ImportError> {
    let (url, provider) = parse_source_url(input)?;
    match provider {
        SourceProvider::Archidekt => import_archidekt(&url).await,
        SourceProvider::Deckstats => import_deckstats(&url).await,
        SourceProvider::Scryfall => import_scryfall_deck(&url).await,
        SourceProvider::MoxfieldManual => Err(ImportError::ManualOnly(
            "That is a Moxfield deck URL. Moxfield says its API is not public and its current Terms require approval for automated access, so the app will not use private endpoints or bypass its challenge page. Open the deck in Moxfield, use its Download/export action, then paste the list or open the downloaded file here.".into(),
        )),
        SourceProvider::TappedOutManual => Err(ImportError::ManualOnly(
            "That is a TappedOut deck URL, but TappedOut's current Terms of Use prohibit automated collection. Use its Download/Export control and paste the exported list here.".into(),
        )),
        SourceProvider::ManaBoxManual => Err(ImportError::ManualOnly(
            "That is a ManaBox deck URL, but ManaBox has no documented public deck API. Export as MTGO text in ManaBox and paste or open the file here.".into(),
        )),
        SourceProvider::MtgGoldfishManual => Err(ImportError::ManualOnly(
            "That is an MTGGoldfish deck URL. Its text-download route is not available to automated clients, so direct import would be unreliable. Use Download → Text File in your browser, then open that file here.".into(),
        )),
    }
}

fn parse_source_url(input: &str) -> Result<(Url, SourceProvider), ImportError> {
    let url = Url::parse(input.trim()).map_err(|_| ImportError::InvalidUrl)?;
    if url.scheme() != "https"
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ImportError::InvalidUrl);
    }
    let host = url
        .host_str()
        .map(|host| host.to_ascii_lowercase())
        .ok_or(ImportError::InvalidUrl)?;
    let path = url.path();

    let provider = match host.as_str() {
        "archidekt.com" | "www.archidekt.com" if ARCHIDEKT_PATH.is_match(path) => {
            SourceProvider::Archidekt
        }
        "deckstats.net" | "www.deckstats.net" if DECKSTATS_PATH.is_match(path) => {
            SourceProvider::Deckstats
        }
        "scryfall.com" | "www.scryfall.com" if SCRYFALL_PATH.is_match(path) => {
            SourceProvider::Scryfall
        }
        "moxfield.com" | "www.moxfield.com" if MOXFIELD_PATH.is_match(path) => {
            SourceProvider::MoxfieldManual
        }
        "tappedout.net" | "www.tappedout.net" if TAPPEDOUT_PATH.is_match(path) => {
            SourceProvider::TappedOutManual
        }
        "manabox.app" | "www.manabox.app" if MANABOX_PATH.is_match(path) => {
            SourceProvider::ManaBoxManual
        }
        "mtggoldfish.com" | "www.mtggoldfish.com" if MTGGOLDFISH_PATH.is_match(path) => {
            SourceProvider::MtgGoldfishManual
        }
        _ => return Err(ImportError::UnsupportedUrl),
    };
    Ok((url, provider))
}

async fn import_archidekt(source_url: &Url) -> Result<ImportResult, ImportError> {
    let captures = ARCHIDEKT_PATH
        .captures(source_url.path())
        .ok_or(ImportError::UnsupportedUrl)?;
    let deck_id = captures
        .name("id")
        .map(|capture| capture.as_str())
        .ok_or(ImportError::UnsupportedUrl)?;
    let endpoint = provider_endpoint(
        &format!("https://archidekt.com/api/decks/{deck_id}/?format=json"),
        EndpointPolicy::Archidekt,
    )?;
    let bytes = fetch_limited(endpoint, EndpointPolicy::Archidekt).await?;
    parse_archidekt_payload(&bytes, source_url)
}

fn parse_archidekt_payload(bytes: &[u8], source_url: &Url) -> Result<ImportResult, ImportError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| ImportError::InvalidResponse(error.to_string()))?;
    if let Some(message) = provider_error_message(&value) {
        return Err(ImportError::InvalidResponse(message));
    }
    let payload: ArchidektDeck = serde_json::from_value(value)
        .map_err(|error| ImportError::InvalidResponse(error.to_string()))?;

    let mut categories: HashMap<String, MergedCategory> = HashMap::new();
    for category in payload.categories {
        let key = category.name.to_ascii_lowercase();
        categories
            .entry(key)
            .and_modify(|merged| {
                merged.is_premier |= category.is_premier;
                merged.included_in_deck &= category.included_in_deck;
            })
            .or_insert(MergedCategory {
                is_premier: category.is_premier,
                included_in_deck: category.included_in_deck,
            });
    }

    let mut commanders = Vec::new();
    let mut mainboard = Vec::new();
    let mut warnings = Vec::new();
    for item in payload.cards {
        if item.quantity <= 0 || item.deleted_at.is_some() || item.companion {
            continue;
        }
        let name = item
            .card
            .and_then(|card| card.oracle_card)
            .map(|oracle| oracle.name)
            .filter(|name| !name.trim().is_empty());
        let Some(name) = name else {
            warnings.push("One Archidekt entry had no Oracle card name and was skipped.".into());
            continue;
        };
        let assigned = item
            .categories
            .iter()
            .filter_map(|category| categories.get(&category.to_ascii_lowercase()))
            .collect::<Vec<_>>();
        if assigned.iter().any(|category| !category.included_in_deck) {
            continue;
        }
        if assigned.iter().any(|category| category.is_premier) {
            commanders.push((item.quantity as u32, name));
        } else {
            mainboard.push((item.quantity as u32, name));
        }
    }
    if commanders.is_empty() && mainboard.is_empty() {
        return Err(ImportError::InvalidResponse(
            "No included deck cards were present in the Archidekt response.".into(),
        ));
    }

    build_import_result(
        "Archidekt",
        payload.name,
        commanders,
        mainboard,
        source_url,
        warnings,
    )
}

async fn import_deckstats(source_url: &Url) -> Result<ImportResult, ImportError> {
    let captures = DECKSTATS_PATH
        .captures(source_url.path())
        .ok_or(ImportError::UnsupportedUrl)?;
    let owner = captures
        .name("owner")
        .map(|capture| capture.as_str())
        .ok_or(ImportError::UnsupportedUrl)?;
    let deck = captures
        .name("deck")
        .map(|capture| capture.as_str())
        .ok_or(ImportError::UnsupportedUrl)?;
    let endpoint = provider_endpoint(
        &format!(
            "https://deckstats.net/api.php?action=get_deck&id_type=saved&owner_id={owner}&id={deck}&response_type=json"
        ),
        EndpointPolicy::Deckstats,
    )?;
    let bytes = fetch_limited(endpoint, EndpointPolicy::Deckstats).await?;
    parse_deckstats_payload(&bytes, source_url)
}

fn parse_deckstats_payload(bytes: &[u8], source_url: &Url) -> Result<ImportResult, ImportError> {
    let payload: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| ImportError::InvalidResponse(error.to_string()))?;
    if let Some(message) = provider_error_message(&payload) {
        return Err(ImportError::InvalidResponse(message));
    }
    if payload
        .get("is_public")
        .is_some_and(|value| !json_truthy(value))
    {
        return Err(ImportError::InvalidResponse(
            "Deckstats reports that this deck is private.".into(),
        ));
    }

    let mut commanders = Vec::new();
    let mut mainboard = Vec::new();
    let mut warnings = Vec::new();
    if let Some(sections) = payload
        .get("sections")
        .and_then(serde_json::Value::as_array)
    {
        for section in sections {
            let section_name = section
                .get("name")
                .or_else(|| section.get("title"))
                .or_else(|| section.get("type"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            match deckstats_section_kind(section_name) {
                DeckstatsSectionKind::Excluded => continue,
                DeckstatsSectionKind::Commander => append_deckstats_cards(
                    section.get("cards"),
                    true,
                    &mut commanders,
                    &mut mainboard,
                    &mut warnings,
                ),
                DeckstatsSectionKind::Mainboard => append_deckstats_cards(
                    section.get("cards"),
                    false,
                    &mut commanders,
                    &mut mainboard,
                    &mut warnings,
                ),
            }
        }
    }
    if commanders.is_empty() {
        warnings
            .push("Deckstats did not mark a commander. Select one before running analysis.".into());
    }
    if mainboard.is_empty() && commanders.is_empty() {
        return Err(ImportError::InvalidResponse(
            "No main-deck cards were present in the JSON response.".into(),
        ));
    }

    build_import_result(
        "Deckstats",
        payload
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        commanders,
        mainboard,
        source_url,
        warnings,
    )
}

fn append_deckstats_cards(
    value: Option<&serde_json::Value>,
    commander_section: bool,
    commanders: &mut Vec<(u32, String)>,
    mainboard: &mut Vec<(u32, String)>,
    warnings: &mut Vec<String>,
) {
    let Some(cards) = value.and_then(serde_json::Value::as_array) else {
        return;
    };
    for card in cards {
        let quantity = card
            .get("amount")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as u32;
        let name = card
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim();
        let deleted = card
            .get("deleted")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            || card
                .get("deletedAt")
                .or_else(|| card.get("deleted_at"))
                .is_some_and(|value| !value.is_null());
        if deleted {
            continue;
        }
        if quantity == 0 || name.is_empty() {
            warnings.push("A malformed Deckstats card entry was skipped.".into());
            continue;
        }
        if commander_section
            || card
                .get("isCommander")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        {
            commanders.push((quantity, name.into()));
        } else {
            mainboard.push((quantity, name.into()));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeckstatsSectionKind {
    Mainboard,
    Commander,
    Excluded,
}

fn deckstats_section_kind(name: &str) -> DeckstatsSectionKind {
    match name.trim().to_ascii_lowercase().as_str() {
        "commander" | "commanders" => DeckstatsSectionKind::Commander,
        "sideboard" | "side board" | "maybeboard" | "maybe board" | "tokens" | "token"
        | "companion" | "companions" => DeckstatsSectionKind::Excluded,
        _ => DeckstatsSectionKind::Mainboard,
    }
}

fn json_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_i64().unwrap_or_default() != 0,
        serde_json::Value::String(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no"
        ),
        serde_json::Value::Null => false,
        _ => true,
    }
}

fn provider_error_message(payload: &serde_json::Value) -> Option<String> {
    let explicitly_failed = payload
        .get("success")
        .is_some_and(|value| !json_truthy(value));
    let error = payload
        .get("error")
        .or_else(|| payload.get("detail"))
        .or_else(|| explicitly_failed.then(|| payload.get("message")).flatten());
    error.and_then(json_error_text).or_else(|| {
        explicitly_failed.then(|| "The provider reported that the deck could not be loaded.".into())
    })
}

fn json_error_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(message) if !message.trim().is_empty() => {
            Some(message.trim().to_string())
        }
        serde_json::Value::Array(messages) => {
            let joined = messages
                .iter()
                .filter_map(json_error_text)
                .collect::<Vec<_>>()
                .join("; ");
            (!joined.is_empty()).then_some(joined)
        }
        serde_json::Value::Object(_) => {
            let rendered = serde_json::to_string(value).ok()?;
            (!rendered.is_empty()).then_some(rendered)
        }
        _ => None,
    }
}

async fn import_scryfall_deck(source_url: &Url) -> Result<ImportResult, ImportError> {
    let captures = SCRYFALL_PATH
        .captures(source_url.path())
        .ok_or(ImportError::UnsupportedUrl)?;
    let deck_id = captures
        .name("id")
        .map(|capture| capture.as_str())
        .ok_or(ImportError::UnsupportedUrl)?;
    let endpoint = provider_endpoint(
        &format!("https://api.scryfall.com/decks/{deck_id}/export/csv"),
        EndpointPolicy::Scryfall,
    )?;
    let bytes = fetch_limited(endpoint, EndpointPolicy::Scryfall).await?;
    parse_scryfall_payload(&bytes, source_url)
}

fn parse_scryfall_payload(bytes: &[u8], source_url: &Url) -> Result<ImportResult, ImportError> {
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);
    let mut commanders = Vec::new();
    let mut mainboard = Vec::new();
    let mut warnings = Vec::new();
    for record in reader.deserialize::<ScryfallDeckRow>() {
        let row = record.map_err(|error| ImportError::InvalidResponse(error.to_string()))?;
        if row.count == 0 || row.name.trim().is_empty() {
            continue;
        }
        match row.section.to_ascii_lowercase().as_str() {
            "commanders" | "commander" => commanders.push((row.count, row.name)),
            "lands" | "nonlands" | "mainboard" | "main" | "deck" => {
                mainboard.push((row.count, row.name))
            }
            "sideboard" | "maybeboard" | "tokens" => {}
            other => warnings.push(format!(
                "Skipped Scryfall section “{other}” because it is outside the main deck."
            )),
        }
    }

    build_import_result(
        "Scryfall Decks",
        None,
        commanders,
        mainboard,
        source_url,
        warnings,
    )
}

fn provider_endpoint(input: &str, policy: EndpointPolicy) -> Result<Url, ImportError> {
    let endpoint = Url::parse(input).map_err(|error| {
        ImportError::InvalidResponse(format!(
            "Could not construct the provider endpoint: {error}"
        ))
    })?;
    if !endpoint_allowed(&endpoint, policy) {
        return Err(ImportError::InvalidResponse(
            "Refused a provider endpoint outside its exact allow-list.".into(),
        ));
    }
    Ok(endpoint)
}

fn endpoint_allowed(endpoint: &Url, policy: EndpointPolicy) -> bool {
    if endpoint.scheme() != "https"
        || endpoint.port_or_known_default() != Some(443)
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.fragment().is_some()
    {
        return false;
    }
    let host = endpoint
        .host_str()
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match policy {
        EndpointPolicy::Archidekt => {
            host == "archidekt.com"
                && ARCHIDEKT_ENDPOINT_PATH.is_match(endpoint.path())
                && exact_query(endpoint, &[("format", QueryValue::Exact("json"))])
        }
        EndpointPolicy::Deckstats => {
            host == "deckstats.net"
                && endpoint.path() == "/api.php"
                && exact_query(
                    endpoint,
                    &[
                        ("action", QueryValue::Exact("get_deck")),
                        ("id_type", QueryValue::Exact("saved")),
                        ("owner_id", QueryValue::PositiveInteger),
                        ("id", QueryValue::PositiveInteger),
                        ("response_type", QueryValue::Exact("json")),
                    ],
                )
        }
        EndpointPolicy::Scryfall => {
            host == "api.scryfall.com"
                && SCRYFALL_ENDPOINT_PATH.is_match(endpoint.path())
                && endpoint.query().is_none()
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum QueryValue {
    Exact(&'static str),
    PositiveInteger,
}

fn exact_query(endpoint: &Url, expected: &[(&str, QueryValue)]) -> bool {
    let actual = endpoint.query_pairs().collect::<Vec<_>>();
    if actual.len() != expected.len() {
        return false;
    }
    expected.iter().all(|(expected_key, expected_value)| {
        let mut matches = actual
            .iter()
            .filter(|(key, _)| key.as_ref() == *expected_key);
        let Some((_, value)) = matches.next() else {
            return false;
        };
        if matches.next().is_some() {
            return false;
        }
        match expected_value {
            QueryValue::Exact(expected) => value.as_ref() == *expected,
            QueryValue::PositiveInteger => {
                !value.is_empty()
                    && !value.starts_with('0')
                    && value.bytes().all(|byte| byte.is_ascii_digit())
            }
        }
    })
}

async fn fetch_limited(endpoint: Url, policy: EndpointPolicy) -> Result<Vec<u8>, ImportError> {
    if !endpoint_allowed(&endpoint, policy) {
        return Err(ImportError::InvalidResponse(
            "Refused a provider endpoint outside its exact allow-list.".into(),
        ));
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let mut response = client
        .get(endpoint)
        .header(USER_AGENT, USER_AGENT_VALUE)
        .header(ACCEPT, policy.accept())
        .send()
        .await?;
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
    {
        return Err(ImportError::ResponseTooLarge);
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let challenge_header = response
        .headers()
        .get("cf-mitigated")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("challenge"));
    if challenge_header {
        return Err(ImportError::ChallengePage);
    }
    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_RESPONSE_BYTES as u64) as usize,
    );
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(ImportError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(4_096)]).to_ascii_lowercase();
    if looks_like_challenge(&prefix) {
        return Err(ImportError::ChallengePage);
    }
    if !status.is_success() {
        return Err(ImportError::ProviderStatus(status));
    }
    if content_type.contains("text/html") {
        return Err(ImportError::ChallengePage);
    }
    if !policy.accepts_content_type(&content_type) {
        return Err(ImportError::InvalidResponse(format!(
            "Expected {}, but received {}.",
            policy.accept(),
            content_type
        )));
    }
    Ok(bytes)
}

fn looks_like_challenge(prefix: &str) -> bool {
    prefix.contains("just a moment")
        || prefix.contains("cdn-cgi/challenge-platform")
        || prefix.contains("cf-chl-")
        || prefix.contains("attention required! | cloudflare")
        || prefix.contains("verify you are human")
        || prefix.contains("g-recaptcha")
        || prefix.contains("hcaptcha")
}

fn build_import_result(
    provider: &str,
    deck_name: Option<String>,
    commanders: Vec<(u32, String)>,
    mainboard: Vec<(u32, String)>,
    source_url: &Url,
    warnings: Vec<String>,
) -> Result<ImportResult, ImportError> {
    validate_imported_sections(&commanders, &mainboard)?;
    let commander_names = commanders
        .iter()
        .flat_map(|(quantity, name)| std::iter::repeat_n(name.clone(), *quantity as usize))
        .collect::<Vec<_>>();
    let mut lines = Vec::new();
    if !commanders.is_empty() {
        lines.push("Commander".to_string());
        lines.extend(
            commanders
                .iter()
                .map(|(quantity, name)| format!("{quantity} {name}")),
        );
        lines.push(String::new());
    }
    lines.push("Deck".to_string());
    lines.extend(
        mainboard
            .iter()
            .map(|(quantity, name)| format!("{quantity} {name}")),
    );

    Ok(ImportResult {
        provider: provider.into(),
        deck_name,
        commanders: commander_names,
        deck_text: lines.join("\n"),
        source_url: source_url.as_str().into(),
        imported_at: Utc::now().to_rfc3339(),
        warnings,
    })
}

fn validate_imported_sections(
    commanders: &[(u32, String)],
    mainboard: &[(u32, String)],
) -> Result<(), ImportError> {
    if commanders.len().saturating_add(mainboard.len()) > MAX_IMPORTED_ENTRIES {
        return Err(ImportError::InvalidResponse(
            "The provider returned too many deck entries.".into(),
        ));
    }
    let mut total_cards = 0u32;
    let mut commander_cards = 0u32;
    for (quantity, name) in commanders.iter().chain(mainboard) {
        if *quantity == 0 || *quantity > MAX_IMPORTED_CARD_QUANTITY {
            return Err(ImportError::InvalidResponse(
                "The provider returned an invalid card quantity.".into(),
            ));
        }
        if name.trim().is_empty() || name.chars().count() > MAX_IMPORTED_CARD_NAME_CHARS {
            return Err(ImportError::InvalidResponse(
                "The provider returned an invalid card name.".into(),
            ));
        }
        total_cards = total_cards
            .checked_add(*quantity)
            .ok_or_else(|| ImportError::InvalidResponse("Deck quantity overflow.".into()))?;
    }
    for (quantity, _) in commanders {
        commander_cards = commander_cards
            .checked_add(*quantity)
            .ok_or_else(|| ImportError::InvalidResponse("Commander quantity overflow.".into()))?;
    }
    if total_cards > MAX_IMPORTED_TOTAL_CARDS {
        return Err(ImportError::InvalidResponse(format!(
            "The provider returned {total_cards} cards, above the {MAX_IMPORTED_TOTAL_CARDS}-card import safety limit."
        )));
    }
    if commander_cards > MAX_IMPORTED_COMMANDERS {
        return Err(ImportError::InvalidResponse(format!(
            "The provider returned {commander_cards} commanders, above the {MAX_IMPORTED_COMMANDERS}-commander import safety limit."
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchidektDeck {
    name: Option<String>,
    #[serde(default)]
    categories: Vec<ArchidektCategory>,
    #[serde(default)]
    cards: Vec<ArchidektCardEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchidektCategory {
    name: String,
    #[serde(default)]
    is_premier: bool,
    #[serde(default = "true_value")]
    included_in_deck: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchidektCardEntry {
    quantity: i32,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    companion: bool,
    deleted_at: Option<serde_json::Value>,
    card: Option<ArchidektPrinting>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchidektPrinting {
    oracle_card: Option<ArchidektOracleCard>,
}

#[derive(Debug, Deserialize)]
struct ArchidektOracleCard {
    name: String,
}

#[derive(Debug, Clone, Copy)]
struct MergedCategory {
    is_premier: bool,
    included_in_deck: bool,
}

#[derive(Debug, Deserialize)]
struct ScryfallDeckRow {
    section: String,
    count: u32,
    name: String,
}

const fn true_value() -> bool {
    true
}
