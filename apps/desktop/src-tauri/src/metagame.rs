//! Report-only metagame context from already validated local snapshots.
//!
//! Popularity and tournament observations are deliberately kept separate from
//! functional semantics, simulation, and bracket scoring. This module performs
//! exact commander matching, derives bounded descriptive facts, and carries
//! provider attribution/export policy with every result. Missing provider facts
//! remain unknown; they are never converted into negative evidence.

#![allow(dead_code)] // Source foundation; analysis/report integration is a separate shared-contract change.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::edhrec_data::{
    EDHREC_AGGREGATE_SCHEMA_VERSION, EDHREC_DERIVATION_VERSION, EdhrecAccessAuthorization,
    EdhrecAggregateSnapshot, EdhrecAuthorizationBasis, EdhrecCommanderScope, EdhrecTimeWindow,
};
use crate::parser::normalize_card_name;
use crate::topdeck_data::{
    TOPDECK_ATTRIBUTION_TEXT, TOPDECK_ATTRIBUTION_URL, TOPDECK_EDH_DATASET_NAME,
    TOPDECK_EDH_INGESTOR_VERSION, TOPDECK_EDH_SCHEMA_VERSION, TopDeckEdhQueryScope,
    TopDeckEdhSnapshot, TopDeckTournamentObservation,
};

pub(crate) const METAGAME_CONTEXT_MODEL_VERSION: &str = "metagame-context-0.1";
const EDHREC_PROVIDER_URL: &str = "https://edhrec.com/";
const MAX_DECK_ORACLE_CARDS: usize = 100;
const MAX_TOPDECK_TOURNAMENTS: usize = 100_000;
const MAX_TOPDECK_COMMANDER_AGGREGATES: usize = 2_000_000;
const MAX_EDHREC_SCOPES: usize = 50_000;
const MAX_EDHREC_CARD_FACTS_PER_SCOPE: usize = 10_000;
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Error)]
pub(crate) enum MetagameContextError {
    #[error("Metagame context input was invalid: {0}")]
    InvalidInput(String),
    #[error("Metagame context aggregation overflowed while summing {0}.")]
    Overflow(&'static str),
    #[error("Metagame context source was internally inconsistent: {0}")]
    InconsistentSource(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MetagameDeckInput {
    pub commanders: Vec<MetagameCommanderInput>,
    /// Unique Oracle identities present in the submitted 100-card list.
    /// Commander identities may be present; the EDHREC library-card projection
    /// excludes them before matching inclusion facts.
    pub cards: Vec<MetagameDeckCardInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MetagameCommanderInput {
    pub name: String,
    pub oracle_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MetagameDeckCardInput {
    pub oracle_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MetagameContextReport {
    pub model_version: String,
    pub use_policy: MetagameUsePolicy,
    pub topdeck: TopDeckMetagameContext,
    pub edhrec: EdhrecMetagameContext,
    pub interpretation_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct MetagameUsePolicy {
    pub disposition: MetagameDisposition,
    pub affects_bracket_rating: bool,
    pub affects_simulation: bool,
    pub affects_functional_synergy: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MetagameDisposition {
    ReportOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderAttribution {
    pub text: String,
    pub url: String,
    pub required_in_derived_displays: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ProviderExportPolicy {
    pub local_derived_display_allowed: bool,
    pub shareable_derived_export_allowed: bool,
    pub raw_source_export_allowed: bool,
    pub attribution_required: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TopDeckAvailability {
    NotInstalled,
    ExactCommanderNotObserved,
    ExactCommanderObserved,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TopDeckFormatContext {
    Edh,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CompetitiveClassification {
    NotInferred,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TopDeckMetagameContext {
    pub availability: TopDeckAvailability,
    pub format: TopDeckFormatContext,
    pub competitive_classification: CompetitiveClassification,
    pub matched_commanders: Vec<String>,
    pub source: Option<TopDeckSourceProvenance>,
    pub observation: TopDeckCommanderObservation,
    pub attribution: ProviderAttribution,
    pub export_policy: ProviderExportPolicy,
    pub unknown_is_negative_evidence: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TopDeckSourceProvenance {
    pub schema_version: String,
    pub ingestor_version: String,
    pub dataset_name: String,
    pub installed_at: String,
    pub endpoint: String,
    pub query_scope: TopDeckQueryScopeReport,
    pub request_fingerprint: String,
    pub source_response_sha256: String,
    pub cache_fingerprint: String,
    pub snapshot_integrity_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TopDeckQueryScopeReport {
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub last: Option<u32>,
    pub participant_min: Option<u32>,
    pub participant_max: Option<u32>,
    pub include_leagues: bool,
}

impl From<&TopDeckEdhQueryScope> for TopDeckQueryScopeReport {
    fn from(value: &TopDeckEdhQueryScope) -> Self {
        Self {
            start: value.start,
            end: value.end,
            last: value.last,
            participant_min: value.participant_min,
            participant_max: value.participant_max,
            include_leagues: value.include_leagues,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TopDeckCommanderObservation {
    pub source_tournament_count: u64,
    pub matched_tournament_count: u64,
    pub matched_league_count: u64,
    pub matched_entries: u64,
    pub wins: u64,
    pub draws: u64,
    pub losses: u64,
    pub reported_result_count: u64,
    /// `(wins + 0.5 * draws) / (wins + draws + losses)`.
    /// This is descriptive reported-record context, not multiplayer win rate.
    pub reported_record_rate: Option<f64>,
    pub source_identified_entries: u64,
    pub source_unidentified_entries: u64,
    pub identified_entry_share: Option<f64>,
    pub earliest_matched_start: Option<i64>,
    pub latest_matched_start: Option<i64>,
    pub rate_definition: String,
}

impl Default for TopDeckCommanderObservation {
    fn default() -> Self {
        Self {
            source_tournament_count: 0,
            matched_tournament_count: 0,
            matched_league_count: 0,
            matched_entries: 0,
            wins: 0,
            draws: 0,
            losses: 0,
            reported_result_count: 0,
            reported_record_rate: None,
            source_identified_entries: 0,
            source_unidentified_entries: 0,
            identified_entry_share: None,
            earliest_matched_start: None,
            latest_matched_start: None,
            rate_definition: "(wins + 0.5 × draws) / reported W-D-L results".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EdhrecAvailability {
    NotInstalled,
    ExactUnthemedCommanderScopeNotObserved,
    ExactUnthemedCommanderScopeObserved,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EdhrecScopeKind {
    UnthemedCommander,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AuthorizationBasisReport {
    WrittenProviderAgreement,
    ProviderPublishedDataLicense,
    ProviderSuppliedAuthorizedExport,
}

impl From<EdhrecAuthorizationBasis> for AuthorizationBasisReport {
    fn from(value: EdhrecAuthorizationBasis) -> Self {
        match value {
            EdhrecAuthorizationBasis::WrittenProviderAgreement => Self::WrittenProviderAgreement,
            EdhrecAuthorizationBasis::ProviderPublishedDataLicense => {
                Self::ProviderPublishedDataLicense
            }
            EdhrecAuthorizationBasis::ProviderSuppliedAuthorizedExport => {
                Self::ProviderSuppliedAuthorizedExport
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecMetagameContext {
    pub availability: EdhrecAvailability,
    pub scope_kind: Option<EdhrecScopeKind>,
    pub matched_commander_oracle_ids: Vec<String>,
    pub source: Option<EdhrecSourceProvenance>,
    pub scope_deck_count: Option<u64>,
    pub commander_rank: Option<u64>,
    pub deck_library_oracle_card_count: u64,
    pub matched_card_count: u64,
    pub unknown_card_count: u64,
    pub facts: Vec<EdhrecCardPopularityFact>,
    pub unknown_cards: Vec<UnknownMetagameCard>,
    pub ignored_matching_theme_scope_count: u64,
    pub attribution: Option<ProviderAttribution>,
    pub export_policy: ProviderExportPolicy,
    pub unknown_is_negative_evidence: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecSourceProvenance {
    pub schema_version: String,
    pub derivation_version: String,
    pub generated_at: String,
    pub installed_at: String,
    pub snapshot_sha256: String,
    pub time_window: EdhrecTimeWindowReport,
    pub provider_name: String,
    pub authorization_basis: AuthorizationBasisReport,
    pub license_or_agreement: String,
    pub authorization_expires_at: Option<String>,
    pub terms_url: Option<String>,
    pub authorization_reference_present: bool,
    pub source_mix_count: u64,
    pub deduplication_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecTimeWindowReport {
    pub start_date: String,
    pub end_date: String,
    pub label: Option<String>,
}

impl From<&EdhrecTimeWindow> for EdhrecTimeWindowReport {
    fn from(value: &EdhrecTimeWindow) -> Self {
        Self {
            start_date: value.start_date.clone(),
            end_date: value.end_date.clone(),
            label: value.label.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct EdhrecCardPopularityFact {
    pub card_oracle_id: String,
    pub display_name: String,
    pub inclusion_deck_count: u64,
    pub eligible_deck_count: u64,
    pub inclusion_rate: f64,
    pub color_identity_inclusion_deck_count: u64,
    pub color_identity_eligible_deck_count: u64,
    pub color_identity_baseline_rate: f64,
    /// Commander/theme inclusion minus color-identity inclusion.
    /// This is differential popularity, not functional synergy or power.
    pub differential_popularity: f64,
    pub differential_popularity_percentage_points: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UnknownMetagameReason {
    SnapshotMissing,
    UnthemedScopeAbsent,
    CardNotReported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UnknownMetagameCard {
    pub card_oracle_id: String,
    pub display_name: String,
    pub reason: UnknownMetagameReason,
    pub interpretation: String,
}

#[derive(Debug, Clone)]
struct CanonicalDeckInput {
    commander_names: Vec<String>,
    commander_oracle_ids: Vec<String>,
    library_cards: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
struct TopDeckProjectionSource {
    schema_version: String,
    ingestor_version: String,
    dataset_name: String,
    installed_at: String,
    endpoint: String,
    query_scope: TopDeckQueryScopeReport,
    request_fingerprint: String,
    source_response_sha256: String,
    cache_fingerprint: String,
    snapshot_integrity_sha256: String,
    attribution_text: String,
    attribution_url: String,
    observations: Vec<TopDeckTournamentObservation>,
}

impl From<&TopDeckEdhSnapshot> for TopDeckProjectionSource {
    fn from(snapshot: &TopDeckEdhSnapshot) -> Self {
        Self {
            schema_version: snapshot.schema_version.clone(),
            ingestor_version: snapshot.ingestor_version.clone(),
            dataset_name: snapshot.dataset_name.clone(),
            installed_at: snapshot.installed_at.clone(),
            endpoint: snapshot.endpoint.clone(),
            query_scope: (&snapshot.query_scope).into(),
            request_fingerprint: snapshot.request_fingerprint.clone(),
            source_response_sha256: snapshot.source_response_sha256.clone(),
            cache_fingerprint: snapshot.cache_fingerprint.clone(),
            snapshot_integrity_sha256: snapshot.snapshot_integrity_sha256.clone(),
            attribution_text: snapshot.attribution_text.clone(),
            attribution_url: snapshot.attribution_url.clone(),
            observations: snapshot.observations(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EdhrecProjectionSource<'a> {
    schema_version: &'a str,
    derivation_version: &'a str,
    generated_at: &'a str,
    installed_at: &'a str,
    snapshot_sha256: &'a str,
    time_window: &'a EdhrecTimeWindow,
    access: &'a EdhrecAccessAuthorization,
    source_mix_count: usize,
    deduplication_notes: &'a str,
    scopes: &'a [EdhrecCommanderScope],
}

impl<'a> From<&'a EdhrecAggregateSnapshot> for EdhrecProjectionSource<'a> {
    fn from(snapshot: &'a EdhrecAggregateSnapshot) -> Self {
        Self {
            schema_version: &snapshot.schema_version,
            derivation_version: &snapshot.derivation_version,
            generated_at: &snapshot.data.generated_at,
            installed_at: &snapshot.installed_at,
            snapshot_sha256: &snapshot.snapshot_sha256,
            time_window: &snapshot.data.time_window,
            access: &snapshot.data.access,
            source_mix_count: snapshot.data.source_mix.len(),
            deduplication_notes: &snapshot.data.deduplication_notes,
            scopes: &snapshot.data.scopes,
        }
    }
}

/// Build descriptive metagame context from already validated, locally active
/// snapshots. This function performs no network access and exposes no scoring
/// adjustment.
pub(crate) fn build_metagame_context(
    deck: &MetagameDeckInput,
    topdeck: Option<&TopDeckEdhSnapshot>,
    edhrec: Option<&EdhrecAggregateSnapshot>,
) -> Result<MetagameContextReport, MetagameContextError> {
    let topdeck_source = topdeck.map(TopDeckProjectionSource::from);
    let edhrec_source = edhrec.map(EdhrecProjectionSource::from);
    build_from_projections(
        deck,
        topdeck_source.as_ref(),
        edhrec_source.as_ref().copied(),
    )
}

fn build_from_projections(
    deck: &MetagameDeckInput,
    topdeck: Option<&TopDeckProjectionSource>,
    edhrec: Option<EdhrecProjectionSource<'_>>,
) -> Result<MetagameContextReport, MetagameContextError> {
    let canonical = canonicalize_deck_input(deck)?;
    Ok(MetagameContextReport {
        model_version: METAGAME_CONTEXT_MODEL_VERSION.into(),
        use_policy: MetagameUsePolicy {
            disposition: MetagameDisposition::ReportOnly,
            affects_bracket_rating: false,
            affects_simulation: false,
            affects_functional_synergy: false,
        },
        topdeck: build_topdeck_context(&canonical, topdeck)?,
        edhrec: build_edhrec_context(&canonical, edhrec)?,
        interpretation_notes: vec![
            "Tournament records, deck-submission popularity, functional synergy, intrinsic power, and bracket policy are separate evidence classes.".into(),
            "This metagame revision is report-only. Optional provider data cannot change simulation, cohesion, or bracket rating.".into(),
            "An absent commander, scope, or card fact means unknown in the selected snapshot; it is never negative evidence.".into(),
        ],
    })
}

fn canonicalize_deck_input(
    deck: &MetagameDeckInput,
) -> Result<CanonicalDeckInput, MetagameContextError> {
    if !(1..=2).contains(&deck.commanders.len()) {
        return Err(MetagameContextError::InvalidInput(
            "commanders must contain one or two exact selections".into(),
        ));
    }
    if deck.cards.is_empty() || deck.cards.len() > MAX_DECK_ORACLE_CARDS {
        return Err(MetagameContextError::InvalidInput(format!(
            "cards must contain 1..={MAX_DECK_ORACLE_CARDS} unique Oracle identities"
        )));
    }

    let mut commander_names = Vec::with_capacity(deck.commanders.len());
    let mut commander_oracle_ids = Vec::with_capacity(deck.commanders.len());
    for commander in &deck.commanders {
        validate_display_name(&commander.name)?;
        validate_oracle_id(&commander.oracle_id)?;
        commander_names.push(normalize_card_name(&commander.name));
        commander_oracle_ids.push(commander.oracle_id.clone());
    }
    commander_names.sort_unstable();
    commander_names.dedup();
    commander_oracle_ids.sort_unstable();
    commander_oracle_ids.dedup();
    if commander_names.len() != deck.commanders.len()
        || commander_oracle_ids.len() != deck.commanders.len()
    {
        return Err(MetagameContextError::InvalidInput(
            "selected commander names and Oracle identities must be unique".into(),
        ));
    }

    let commander_ids = commander_oracle_ids.iter().collect::<BTreeSet<_>>();
    let mut library_cards = BTreeMap::new();
    for card in &deck.cards {
        validate_oracle_id(&card.oracle_id)?;
        validate_display_name(&card.display_name)?;
        if library_cards
            .insert(card.oracle_id.clone(), card.display_name.clone())
            .is_some()
        {
            return Err(MetagameContextError::InvalidInput(format!(
                "card Oracle identity `{}` occurs more than once",
                card.oracle_id
            )));
        }
    }
    library_cards.retain(|oracle_id, _| !commander_ids.contains(oracle_id));
    Ok(CanonicalDeckInput {
        commander_names,
        commander_oracle_ids,
        library_cards,
    })
}

fn build_topdeck_context(
    deck: &CanonicalDeckInput,
    source: Option<&TopDeckProjectionSource>,
) -> Result<TopDeckMetagameContext, MetagameContextError> {
    let attribution = ProviderAttribution {
        text: source
            .map_or(TOPDECK_ATTRIBUTION_TEXT, |source| &source.attribution_text)
            .into(),
        url: source
            .map_or(TOPDECK_ATTRIBUTION_URL, |source| &source.attribution_url)
            .into(),
        required_in_derived_displays: true,
    };
    let Some(source) = source else {
        return Ok(TopDeckMetagameContext {
            availability: TopDeckAvailability::NotInstalled,
            format: TopDeckFormatContext::Edh,
            competitive_classification: CompetitiveClassification::NotInferred,
            matched_commanders: deck.commander_names.clone(),
            source: None,
            observation: TopDeckCommanderObservation::default(),
            attribution,
            export_policy: ProviderExportPolicy {
                local_derived_display_allowed: false,
                shareable_derived_export_allowed: false,
                raw_source_export_allowed: false,
                attribution_required: true,
                reason: "No local TopDeck.gg aggregate snapshot is installed.".into(),
            },
            unknown_is_negative_evidence: false,
            limitations: topdeck_limitations(),
        });
    };

    if source.schema_version != TOPDECK_EDH_SCHEMA_VERSION
        || source.ingestor_version != TOPDECK_EDH_INGESTOR_VERSION
        || source.dataset_name != TOPDECK_EDH_DATASET_NAME
        || source.attribution_text != TOPDECK_ATTRIBUTION_TEXT
        || source.attribution_url != TOPDECK_ATTRIBUTION_URL
    {
        return Err(MetagameContextError::InconsistentSource(
            "TopDeck schema, ingestor, dataset, or attribution did not match the reviewed contract."
                .into(),
        ));
    }
    if source.observations.len() > MAX_TOPDECK_TOURNAMENTS {
        return Err(MetagameContextError::InconsistentSource(format!(
            "TopDeck observations exceeded the {MAX_TOPDECK_TOURNAMENTS} context bound"
        )));
    }

    let mut aggregate_count = 0usize;
    let mut observation = TopDeckCommanderObservation {
        source_tournament_count: source.observations.len() as u64,
        ..Default::default()
    };
    for tournament in &source.observations {
        observation.source_identified_entries = checked_add(
            observation.source_identified_entries,
            tournament.identified_commander_entries,
            "TopDeck identified entries",
        )?;
        observation.source_unidentified_entries = checked_add(
            observation.source_unidentified_entries,
            tournament.unidentified_entries,
            "TopDeck unidentified entries",
        )?;
        let mut tournament_matched = false;
        for commander in &tournament.commanders {
            aggregate_count =
                aggregate_count
                    .checked_add(1)
                    .ok_or(MetagameContextError::Overflow(
                        "TopDeck commander aggregate count",
                    ))?;
            if aggregate_count > MAX_TOPDECK_COMMANDER_AGGREGATES {
                return Err(MetagameContextError::InconsistentSource(format!(
                    "TopDeck commander aggregates exceeded the {MAX_TOPDECK_COMMANDER_AGGREGATES} context bound"
                )));
            }
            let mut candidate = commander
                .commanders
                .iter()
                .map(|name| normalize_card_name(name))
                .collect::<Vec<_>>();
            candidate.sort_unstable();
            candidate.dedup();
            if candidate != deck.commander_names {
                continue;
            }
            tournament_matched = true;
            observation.matched_entries = checked_add(
                observation.matched_entries,
                commander.entries,
                "TopDeck matched entries",
            )?;
            observation.wins = checked_add(observation.wins, commander.wins, "TopDeck wins")?;
            observation.draws = checked_add(observation.draws, commander.draws, "TopDeck draws")?;
            observation.losses =
                checked_add(observation.losses, commander.losses, "TopDeck losses")?;
        }
        if tournament_matched {
            observation.matched_tournament_count = checked_add(
                observation.matched_tournament_count,
                1,
                "TopDeck matched tournaments",
            )?;
            if tournament.is_league {
                observation.matched_league_count = checked_add(
                    observation.matched_league_count,
                    1,
                    "TopDeck matched leagues",
                )?;
            }
            observation.earliest_matched_start = Some(
                observation
                    .earliest_matched_start
                    .map_or(tournament.start_date, |value| {
                        value.min(tournament.start_date)
                    }),
            );
            observation.latest_matched_start = Some(
                observation
                    .latest_matched_start
                    .map_or(tournament.start_date, |value| {
                        value.max(tournament.start_date)
                    }),
            );
        }
    }
    observation.reported_result_count = checked_add(
        checked_add(
            observation.wins,
            observation.draws,
            "TopDeck reported results",
        )?,
        observation.losses,
        "TopDeck reported results",
    )?;
    observation.reported_record_rate = (observation.reported_result_count > 0).then(|| {
        (observation.wins as f64 + observation.draws as f64 * 0.5)
            / observation.reported_result_count as f64
    });
    observation.identified_entry_share = (observation.source_identified_entries > 0)
        .then(|| observation.matched_entries as f64 / observation.source_identified_entries as f64);

    let availability = if observation.matched_entries > 0 {
        TopDeckAvailability::ExactCommanderObserved
    } else {
        TopDeckAvailability::ExactCommanderNotObserved
    };
    Ok(TopDeckMetagameContext {
        availability,
        format: TopDeckFormatContext::Edh,
        competitive_classification: CompetitiveClassification::NotInferred,
        matched_commanders: deck.commander_names.clone(),
        source: Some(TopDeckSourceProvenance {
            schema_version: source.schema_version.clone(),
            ingestor_version: source.ingestor_version.clone(),
            dataset_name: source.dataset_name.clone(),
            installed_at: source.installed_at.clone(),
            endpoint: source.endpoint.clone(),
            query_scope: source.query_scope.clone(),
            request_fingerprint: source.request_fingerprint.clone(),
            source_response_sha256: source.source_response_sha256.clone(),
            cache_fingerprint: source.cache_fingerprint.clone(),
            snapshot_integrity_sha256: source.snapshot_integrity_sha256.clone(),
        }),
        observation,
        attribution,
        export_policy: ProviderExportPolicy {
            local_derived_display_allowed: true,
            shareable_derived_export_allowed: true,
            raw_source_export_allowed: false,
            attribution_required: true,
            reason: "Only bounded commander-level derived observations may be displayed or exported, with the exact TopDeck.gg attribution; raw responses and decklists are not retained or exportable.".into(),
        },
        unknown_is_negative_evidence: false,
        limitations: topdeck_limitations(),
    })
}

fn topdeck_limitations() -> Vec<String> {
    vec![
        "TopDeck's documented API format is EDH. This context does not infer that an event or deck is cEDH.".into(),
        "The match is commander-level only; it does not establish that an observed 99-card list resembles the analyzed list.".into(),
        "Reported W-D-L records are descriptive sample context, not pod win probability, causal commander strength, or bracket evidence.".into(),
        "No exact commander observation means unknown within the selected query scope, not weak or unpopular.".into(),
    ]
}

fn build_edhrec_context(
    deck: &CanonicalDeckInput,
    source: Option<EdhrecProjectionSource<'_>>,
) -> Result<EdhrecMetagameContext, MetagameContextError> {
    let Some(source) = source else {
        return Ok(empty_edhrec_context(
            deck,
            EdhrecAvailability::NotInstalled,
            UnknownMetagameReason::SnapshotMissing,
            None,
            0,
        ));
    };
    if source.schema_version != EDHREC_AGGREGATE_SCHEMA_VERSION
        || source.derivation_version != EDHREC_DERIVATION_VERSION
        || source.scopes.len() > MAX_EDHREC_SCOPES
    {
        return Err(MetagameContextError::InconsistentSource(
            "EDHREC schema, derivation, or scope bounds did not match the reviewed authorized-aggregate contract.".into(),
        ));
    }

    let matching_scopes = source
        .scopes
        .iter()
        .filter(|scope| canonical_scope_commanders(scope) == deck.commander_oracle_ids)
        .collect::<Vec<_>>();
    let ignored_matching_theme_scope_count = matching_scopes
        .iter()
        .filter(|scope| scope.theme.is_some())
        .count() as u64;
    let unthemed = matching_scopes
        .iter()
        .filter(|scope| scope.theme.is_none())
        .copied()
        .collect::<Vec<_>>();
    if unthemed.len() > 1 {
        return Err(MetagameContextError::InconsistentSource(
            "more than one exact unthemed EDHREC commander scope was present".into(),
        ));
    }

    let provenance = EdhrecSourceProvenance {
        schema_version: source.schema_version.into(),
        derivation_version: source.derivation_version.into(),
        generated_at: source.generated_at.into(),
        installed_at: source.installed_at.into(),
        snapshot_sha256: source.snapshot_sha256.into(),
        time_window: source.time_window.into(),
        provider_name: source.access.provider_name.clone(),
        authorization_basis: source.access.basis.into(),
        license_or_agreement: source.access.license_or_agreement.clone(),
        authorization_expires_at: source.access.expires_at.clone(),
        terms_url: source.access.terms_url.clone(),
        authorization_reference_present: !source.access.authorization_reference.is_empty(),
        source_mix_count: source.source_mix_count as u64,
        deduplication_notes: source.deduplication_notes.into(),
    };
    let attribution = ProviderAttribution {
        text: source.access.attribution.clone(),
        url: EDHREC_PROVIDER_URL.into(),
        required_in_derived_displays: true,
    };
    let export_policy = ProviderExportPolicy {
        local_derived_display_allowed: source.access.derived_analysis_allowed,
        shareable_derived_export_allowed: source.access.redistribution_allowed,
        raw_source_export_allowed: false,
        attribution_required: true,
        reason: if source.access.redistribution_allowed {
            "The recorded provider authorization permits redistribution; derived displays and exports must retain the supplied attribution. Raw provider files remain outside report export.".into()
        } else {
            "The recorded provider authorization does not permit redistribution. Derived EDHREC values are local-display-only and must be omitted from shareable report exports.".into()
        },
    };

    let Some(scope) = unthemed.first().copied() else {
        let mut context = empty_edhrec_context(
            deck,
            EdhrecAvailability::ExactUnthemedCommanderScopeNotObserved,
            UnknownMetagameReason::UnthemedScopeAbsent,
            Some((provenance, attribution, export_policy)),
            ignored_matching_theme_scope_count,
        );
        if ignored_matching_theme_scope_count > 0 {
            context.limitations.push(
                "Matching themed scopes were present but ignored. This builder never guesses an EDHREC theme from card roles, strategy labels, or popularity."
                    .into(),
            );
        }
        return Ok(context);
    };
    if scope.cards.len() > MAX_EDHREC_CARD_FACTS_PER_SCOPE {
        return Err(MetagameContextError::InconsistentSource(format!(
            "EDHREC scope exceeded the {MAX_EDHREC_CARD_FACTS_PER_SCOPE} card-fact context bound"
        )));
    }

    let facts_by_oracle_id = scope
        .cards
        .iter()
        .map(|fact| (fact.card_oracle_id.as_str(), fact))
        .collect::<BTreeMap<_, _>>();
    let mut facts = Vec::new();
    let mut unknown_cards = Vec::new();
    for (oracle_id, display_name) in &deck.library_cards {
        let Some(fact) = facts_by_oracle_id.get(oracle_id.as_str()) else {
            unknown_cards.push(unknown_card(
                oracle_id,
                display_name,
                UnknownMetagameReason::CardNotReported,
            ));
            continue;
        };
        let derived = fact.derived_metrics().ok_or_else(|| {
            MetagameContextError::InconsistentSource(format!(
                "EDHREC card fact `{oracle_id}` had a zero denominator"
            ))
        })?;
        facts.push(EdhrecCardPopularityFact {
            card_oracle_id: oracle_id.clone(),
            display_name: display_name.clone(),
            inclusion_deck_count: fact.inclusion_deck_count,
            eligible_deck_count: fact.eligible_deck_count,
            inclusion_rate: derived.inclusion_rate,
            color_identity_inclusion_deck_count: fact.color_identity_inclusion_deck_count,
            color_identity_eligible_deck_count: fact.color_identity_eligible_deck_count,
            color_identity_baseline_rate: derived.color_identity_baseline_rate,
            differential_popularity: derived.synergy_score,
            differential_popularity_percentage_points: derived.synergy_percentage_points,
        });
    }
    let matched_card_count = facts.len() as u64;
    let unknown_card_count = unknown_cards.len() as u64;
    Ok(EdhrecMetagameContext {
        availability: EdhrecAvailability::ExactUnthemedCommanderScopeObserved,
        scope_kind: Some(EdhrecScopeKind::UnthemedCommander),
        matched_commander_oracle_ids: deck.commander_oracle_ids.clone(),
        source: Some(provenance),
        scope_deck_count: Some(scope.scope_deck_count()),
        commander_rank: scope.commander_popularity.rank,
        deck_library_oracle_card_count: deck.library_cards.len() as u64,
        matched_card_count,
        unknown_card_count,
        facts,
        unknown_cards,
        ignored_matching_theme_scope_count,
        attribution: Some(attribution),
        export_policy,
        unknown_is_negative_evidence: false,
        limitations: edhrec_limitations(),
    })
}

fn empty_edhrec_context(
    deck: &CanonicalDeckInput,
    availability: EdhrecAvailability,
    reason: UnknownMetagameReason,
    source: Option<(
        EdhrecSourceProvenance,
        ProviderAttribution,
        ProviderExportPolicy,
    )>,
    ignored_matching_theme_scope_count: u64,
) -> EdhrecMetagameContext {
    let unknown_cards = deck
        .library_cards
        .iter()
        .map(|(oracle_id, display_name)| unknown_card(oracle_id, display_name, reason))
        .collect::<Vec<_>>();
    let unknown_card_count = unknown_cards.len() as u64;
    let (provenance, attribution, export_policy) = source.map_or_else(
        || {
            (
                None,
                None,
                ProviderExportPolicy {
                    local_derived_display_allowed: false,
                    shareable_derived_export_allowed: false,
                    raw_source_export_allowed: false,
                    attribution_required: true,
                    reason: "No provider-authorized EDHREC aggregate snapshot is installed.".into(),
                },
            )
        },
        |(provenance, attribution, export_policy)| {
            (Some(provenance), Some(attribution), export_policy)
        },
    );
    EdhrecMetagameContext {
        availability,
        scope_kind: None,
        matched_commander_oracle_ids: deck.commander_oracle_ids.clone(),
        source: provenance,
        scope_deck_count: None,
        commander_rank: None,
        deck_library_oracle_card_count: deck.library_cards.len() as u64,
        matched_card_count: 0,
        unknown_card_count,
        facts: Vec::new(),
        unknown_cards,
        ignored_matching_theme_scope_count,
        attribution,
        export_policy,
        unknown_is_negative_evidence: false,
        limitations: edhrec_limitations(),
    }
}

fn edhrec_limitations() -> Vec<String> {
    vec![
        "EDHREC inclusion and differential popularity describe submitted-deck prevalence, not rules interaction, causal benefit, game outcome, or bracket power.".into(),
        "Only an exact unthemed commander or commander-pair Oracle-ID scope is used. Theme identity is never inferred.".into(),
        "A card absent from an authorized aggregate scope is unknown, not anti-synergistic or weak.".into(),
        "Every rate retains its numerator and denominator because time window, eligibility, source mix, and sample size constrain interpretation.".into(),
    ]
}

fn canonical_scope_commanders(scope: &EdhrecCommanderScope) -> Vec<String> {
    let mut commanders = vec![scope.commander_oracle_id.clone()];
    if let Some(partner) = scope.partner_oracle_id.as_ref() {
        commanders.push(partner.clone());
    }
    commanders.sort_unstable();
    commanders.dedup();
    commanders
}

fn unknown_card(
    oracle_id: &str,
    display_name: &str,
    reason: UnknownMetagameReason,
) -> UnknownMetagameCard {
    UnknownMetagameCard {
        card_oracle_id: oracle_id.into(),
        display_name: display_name.into(),
        reason,
        interpretation:
            "Unknown in this exact authorized snapshot; no negative evidence was inferred.".into(),
    }
}

fn checked_add(left: u64, right: u64, label: &'static str) -> Result<u64, MetagameContextError> {
    left.checked_add(right)
        .ok_or(MetagameContextError::Overflow(label))
}

fn validate_display_name(value: &str) -> Result<(), MetagameContextError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(MetagameContextError::InvalidInput(
            "card display names must be non-empty trimmed text without control characters".into(),
        ));
    }
    Ok(())
}

fn validate_oracle_id(value: &str) -> Result<(), MetagameContextError> {
    let bytes = value.as_bytes();
    if bytes.len() != 36
        || bytes.iter().enumerate().any(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte != b'-',
            _ => !(byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
        })
    {
        return Err(MetagameContextError::InvalidInput(format!(
            "Oracle identity `{value}` must be a lowercase UUID"
        )));
    }
    Ok(())
}
