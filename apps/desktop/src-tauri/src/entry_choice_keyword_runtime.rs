//! Exact standalone rules for Unleash, Riot, Bloodthirst, and Ravenous.
//!
//! The compiler accepts only complete Oracle keyword clauses whose reminder
//! text matches the reviewed rules contract. The runtime models the entry
//! choices, counter replacement results, persistent Unleash restriction,
//! Riot haste grants, Bloodthirst damage history, and Ravenous X binding and
//! draw trigger. No production adapter is connected yet.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const ENTRY_CHOICE_KEYWORD_COMPILER_VERSION: &str = "entry-choice-keyword-compiler-0.1";
pub const ENTRY_CHOICE_KEYWORD_RUNTIME_VERSION: &str = "entry-choice-keyword-runtime-0.1";
pub const ENTRY_CHOICE_KEYWORD_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:107.3g,107.3i,107.3m,120,122,\
     603.4,603.7,604.1,614.1,614.12,616.1,702.54,702.98,702.136,702.156";

const UNLEASH_EXACT: &str = "Unleash (You may have this creature enter with a +1/+1 counter on it. It can't block as long as it has a +1/+1 counter on it.)";
const RIOT_EXACT: &str =
    "Riot (This creature enters with your choice of a +1/+1 counter or haste.)";
const RIOT_ADDITIONAL_EXACT: &str =
    "Riot (This creature enters with your choice of an additional +1/+1 counter or haste.)";
const BLOODTHIRST_X_EXACT: &str = "Bloodthirst X (This creature enters with X +1/+1 counters on it, where X is the damage dealt to your opponents this turn.)";
const RAVENOUS_EXACT: &str = "Ravenous (This creature enters with X +1/+1 counters on it. If X is 5 or more, draw a card when it enters.)";

pub type PlayerId = u8;
pub type BindingId = u64;
pub type EntryEventId = u64;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type TurnId = u64;

/// These programs are not live until the main engine supplies complete entry
/// replacement, damage history, X provenance, and continuous ability state.
pub const fn entry_choice_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntryChoiceKeywordKind {
    Unleash,
    Riot,
    BloodthirstFixed { counters: u32 },
    BloodthirstOpponentDamageTotal,
    Ravenous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryChoiceKeywordProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: EntryChoiceKeywordKind,
}

impl EntryChoiceKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> EntryChoiceKeywordKind {
        self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        entry_choice_keyword_production_adapter_connected()
    }
}

/// Compiles one complete Oracle line. The exact and normalized forms are both
/// content inputs. Location, card name, row identity, and snapshot metadata
/// are intentionally absent.
pub fn compile_entry_choice_keyword_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<EntryChoiceKeywordProgram> {
    if exact_source.is_empty()
        || normalized_source.is_empty()
        || exact_source.trim() != exact_source
        || normalized_source.trim() != normalized_source
        || exact_source.contains(['\r', '\n'])
        || normalized_source.contains(['\r', '\n'])
        || collapse_whitespace(exact_source) != exact_source
        || collapse_whitespace(normalized_source) != normalized_source
    {
        return None;
    }

    let expected_normalized = normalize_reviewed_clause(exact_source);
    if normalized_source != expected_normalized {
        return None;
    }

    let kind = if exact_source == UNLEASH_EXACT {
        EntryChoiceKeywordKind::Unleash
    } else if exact_source == RIOT_EXACT || exact_source == RIOT_ADDITIONAL_EXACT {
        EntryChoiceKeywordKind::Riot
    } else if exact_source == BLOODTHIRST_X_EXACT {
        EntryChoiceKeywordKind::BloodthirstOpponentDamageTotal
    } else if exact_source == RAVENOUS_EXACT {
        EntryChoiceKeywordKind::Ravenous
    } else {
        parse_fixed_bloodthirst(exact_source)?
    };

    let semantic_digest = entry_choice_keyword_semantic_digest_with_versions(
        exact_source,
        normalized_source,
        kind,
        ENTRY_CHOICE_KEYWORD_COMPILER_VERSION,
        ENTRY_CHOICE_KEYWORD_RUNTIME_VERSION,
        ENTRY_CHOICE_KEYWORD_RULES_CONTEXT_VERSION,
    );
    Some(EntryChoiceKeywordProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        kind,
    })
}

fn parse_fixed_bloodthirst(exact_source: &str) -> Option<EntryChoiceKeywordKind> {
    let rest = exact_source.strip_prefix("Bloodthirst ")?;
    let (amount, _) = rest.split_once(" (")?;
    if amount.is_empty()
        || !amount.bytes().all(|byte| byte.is_ascii_digit())
        || (amount.len() > 1 && amount.starts_with('0'))
    {
        return None;
    }
    let counters = amount.parse::<u32>().ok()?;
    if counters == 0 {
        return None;
    }
    let quantity = english_counter_quantity(counters)?;
    let counter_noun = if counters == 1 { "counter" } else { "counters" };
    let expected = format!(
        "Bloodthirst {counters} (If an opponent was dealt damage this turn, \
         this creature enters with {quantity} +1/+1 {counter_noun} on it.)"
    );
    (exact_source == expected).then_some(EntryChoiceKeywordKind::BloodthirstFixed { counters })
}

fn english_counter_quantity(amount: u32) -> Option<&'static str> {
    match amount {
        1 => Some("a"),
        2 => Some("two"),
        3 => Some("three"),
        4 => Some("four"),
        5 => Some("five"),
        6 => Some("six"),
        7 => Some("seven"),
        8 => Some("eight"),
        9 => Some("nine"),
        10 => Some("ten"),
        11 => Some("eleven"),
        12 => Some("twelve"),
        13 => Some("thirteen"),
        14 => Some("fourteen"),
        15 => Some("fifteen"),
        16 => Some("sixteen"),
        17 => Some("seventeen"),
        18 => Some("eighteen"),
        19 => Some("nineteen"),
        20 => Some("twenty"),
        _ => None,
    }
}

fn normalize_reviewed_clause(exact_source: &str) -> String {
    replace_ascii_case_insensitive(
        &collapse_whitespace(&exact_source.replace('\u{2019}', "'")),
        "this creature",
        "this object",
    )
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn replace_ascii_case_insensitive(source: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return source.to_owned();
    }
    let lower_source = source.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    let mut cursor = 0usize;
    let mut output = String::with_capacity(source.len());
    while let Some(relative) = lower_source[cursor..].find(&lower_needle) {
        let start = cursor + relative;
        let end = start + needle.len();
        output.push_str(&source[cursor..start]);
        output.push_str(replacement);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    output
}

fn entry_choice_keyword_semantic_digest_with_versions(
    exact_source: &str,
    normalized_source: &str,
    kind: EntryChoiceKeywordKind,
    compiler_version: &str,
    runtime_version: &str,
    rules_context_version: &str,
) -> String {
    let kind_contract = match kind {
        EntryChoiceKeywordKind::Unleash => {
            "unleash/v1;entry-choice=additional-plus-one-counter-or-decline;\
             blocking=prohibited-while-same-incarnation-has-plus-one-counter-and-ability"
                .to_owned()
        }
        EntryChoiceKeywordKind::Riot => {
            "riot/v1;entry-choice=additional-plus-one-counter-or-gain-haste;\
             multiple-instances=independent;haste-duration=same-incarnation"
                .to_owned()
        }
        EntryChoiceKeywordKind::BloodthirstFixed { counters } => format!(
            "bloodthirst-fixed/v1;condition=any-opponent-dealt-damage-this-turn;\
             entry-additional-plus-one-counters={counters};multiple-instances=independent"
        ),
        EntryChoiceKeywordKind::BloodthirstOpponentDamageTotal => {
            "bloodthirst-x/v1;entry-additional-plus-one-counters=total-damage-dealt-to-current-\
             opponents-this-turn;multiple-instances=independent"
                .to_owned()
        }
        EntryChoiceKeywordKind::Ravenous => {
            "ravenous/v1;x=chosen-value-from-cost-of-resolving-spell-that-became-permanent-or-zero;\
             entry-additional-plus-one-counters=x;trigger=enters-if-x-at-least-five;\
             resolution=controller-draws-one"
                .to_owned()
        }
    };
    let mut hasher = Sha256::new();
    for component in [
        "entry-choice-keyword-content/v1",
        compiler_version,
        runtime_version,
        rules_context_version,
        exact_source,
        normalized_source,
        &kind_contract,
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryChoiceKeywordBinding {
    pub binding_id: BindingId,
    pub source: ObjectRef,
    pub program: EntryChoiceKeywordProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpponentDamageHistoryEvidence {
    pub turn_id: TurnId,
    pub controller: PlayerId,
    pub opponents: BTreeSet<PlayerId>,
    /// Actual damage dealt this turn, after prevention and replacement.
    pub damage_dealt_by_player: BTreeMap<PlayerId, u32>,
    pub history_complete: bool,
    pub opponent_relationships_complete: bool,
}

impl OpponentDamageHistoryEvidence {
    fn validate_for(&self, controller: PlayerId) -> Result<(), EntryChoiceRuntimeError> {
        if !self.history_complete {
            return Err(EntryChoiceRuntimeError::IncompleteDamageHistory);
        }
        if !self.opponent_relationships_complete {
            return Err(EntryChoiceRuntimeError::IncompleteOpponentRelationships);
        }
        if self.controller != controller {
            return Err(EntryChoiceRuntimeError::DamageHistoryControllerMismatch {
                expected: controller,
                actual: self.controller,
            });
        }
        if self.opponents.contains(&controller) {
            return Err(EntryChoiceRuntimeError::ControllerListedAsOpponent(
                controller,
            ));
        }
        Ok(())
    }

    fn any_opponent_was_dealt_damage(&self) -> bool {
        self.opponents.iter().any(|opponent| {
            self.damage_dealt_by_player
                .get(opponent)
                .copied()
                .unwrap_or(0)
                > 0
        })
    }

    fn total_damage_dealt_to_opponents(&self) -> Result<u32, EntryChoiceRuntimeError> {
        self.opponents.iter().try_fold(0u32, |total, opponent| {
            total
                .checked_add(
                    self.damage_dealt_by_player
                        .get(opponent)
                        .copied()
                        .unwrap_or(0),
                )
                .ok_or(EntryChoiceRuntimeError::CounterQuantityOverflow)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryXEvidence {
    ResolvingSpell {
        spell: ObjectRef,
        chosen_x_for_a_cost: Option<u32>,
        all_cost_x_values_complete: bool,
    },
    NoResolvingSpell {
        entry_provenance_complete: bool,
    },
}

impl EntryXEvidence {
    fn ravenous_x(&self, source: ObjectRef) -> Result<u32, EntryChoiceRuntimeError> {
        match self {
            Self::ResolvingSpell {
                spell,
                chosen_x_for_a_cost,
                all_cost_x_values_complete,
            } => {
                if !all_cost_x_values_complete {
                    return Err(EntryChoiceRuntimeError::IncompleteSpellXEvidence);
                }
                if *spell != source {
                    return Err(EntryChoiceRuntimeError::ResolvingSpellMismatch {
                        expected: source,
                        actual: *spell,
                    });
                }
                Ok(chosen_x_for_a_cost.unwrap_or(0))
            }
            Self::NoResolvingSpell {
                entry_provenance_complete,
            } => {
                if !entry_provenance_complete {
                    return Err(EntryChoiceRuntimeError::IncompleteEntryProvenance);
                }
                Ok(0)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryAttemptEvidence {
    pub event_id: EntryEventId,
    /// The object immediately before it enters.
    pub source: ObjectRef,
    /// The incarnation assigned by the zone change that enters the battlefield.
    pub entering_incarnation_id: IncarnationId,
    pub controller: PlayerId,
    /// +1/+1 counters supplied by already reviewed non-keyword entry effects.
    pub prior_plus_one_counters: u32,
    pub applicable_keyword_instances_complete: bool,
    pub damage_history: Option<OpponentDamageHistoryEvidence>,
    pub x_evidence: Option<EntryXEvidence>,
}

impl EntryAttemptEvidence {
    pub fn entering_object(&self) -> ObjectRef {
        ObjectRef {
            object_id: self.source.object_id,
            incarnation_id: self.entering_incarnation_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnleashEntryChoice {
    AddCounter,
    DeclineCounter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiotEntryChoice {
    AddCounter,
    GainHaste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordEntryChoice {
    Unleash(UnleashEntryChoice),
    Riot(RiotEntryChoice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingEntryChoice {
    pub binding_id: BindingId,
    pub choice: KeywordEntryChoice,
}

/// The main replacement engine must provide the final result of each counter
/// placement after all applicable counter replacement effects have applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterPlacementEvidence {
    pub binding_id: BindingId,
    pub entering_object: ObjectRef,
    pub requested_counters: u32,
    pub counters_placed: u32,
    pub replacement_effects_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryChoiceResolutionInput {
    pub chooser: PlayerId,
    /// Each keyword replacement is applied once in the recorded order.
    pub application_order: Vec<BindingId>,
    pub choices: Vec<BindingEntryChoice>,
    pub counter_placements: Vec<CounterPlacementEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEntryChoiceKeywordTransaction {
    attempt: EntryAttemptEvidence,
    bindings: BTreeMap<BindingId, EntryChoiceKeywordBinding>,
}

pub fn begin_entry_choice_keyword_transaction(
    attempt: EntryAttemptEvidence,
    bindings: Vec<EntryChoiceKeywordBinding>,
) -> Result<PendingEntryChoiceKeywordTransaction, EntryChoiceRuntimeError> {
    if !attempt.applicable_keyword_instances_complete {
        return Err(EntryChoiceRuntimeError::IncompleteKeywordInstanceCensus);
    }
    if attempt.entering_incarnation_id == attempt.source.incarnation_id {
        return Err(EntryChoiceRuntimeError::ReusedIncarnation(
            attempt.entering_incarnation_id,
        ));
    }

    let mut indexed = BTreeMap::new();
    for binding in bindings {
        if binding.source != attempt.source {
            return Err(EntryChoiceRuntimeError::BindingSourceMismatch {
                binding_id: binding.binding_id,
                expected: attempt.source,
                actual: binding.source,
            });
        }
        if indexed.insert(binding.binding_id, binding).is_some() {
            return Err(EntryChoiceRuntimeError::DuplicateBinding);
        }
    }
    Ok(PendingEntryChoiceKeywordTransaction {
        attempt,
        bindings: indexed,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordEntryDecision {
    Unleash(UnleashEntryChoice),
    Riot(RiotEntryChoice),
    BloodthirstConditionMet(bool),
    BloodthirstDamageTotal(u32),
    RavenousX(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordEntryReceipt {
    pub binding_id: BindingId,
    pub semantic_digest: String,
    pub decision: KeywordEntryDecision,
    pub requested_counters: u32,
    pub counters_placed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnleashRestrictionDuration {
    WhileSameBattlefieldIncarnationHasPlusOneCounterAndUnleashAbility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnleashBlockingRestriction {
    pub binding_id: BindingId,
    pub object: ObjectRef,
    pub semantic_digest: String,
    pub duration: UnleashRestrictionDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiotHasteDuration {
    SameBattlefieldIncarnation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiotHasteGrant {
    pub binding_id: BindingId,
    pub object: ObjectRef,
    pub semantic_digest: String,
    pub duration: RiotHasteDuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveRiotHasteGrant {
    pub object: ObjectRef,
    /// Multiple Riot instances are independent, but haste itself is redundant.
    pub contributing_bindings: Vec<BindingId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RavenousTriggerId {
    pub entry_event_id: EntryEventId,
    pub binding_id: BindingId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRavenousDrawTrigger {
    pub trigger_id: RavenousTriggerId,
    pub controller: PlayerId,
    pub source_at_trigger: ObjectRef,
    pub bound_x: u32,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryChoiceKeywordResolution {
    pub event_id: EntryEventId,
    pub entering_object: ObjectRef,
    pub controller: PlayerId,
    pub plus_one_counters: u32,
    pub receipts: Vec<KeywordEntryReceipt>,
    pub unleash_restrictions: Vec<UnleashBlockingRestriction>,
    pub riot_haste_grants: Vec<RiotHasteGrant>,
    pub effective_riot_haste: Option<EffectiveRiotHasteGrant>,
    pub ravenous_draw_triggers: Vec<PendingRavenousDrawTrigger>,
}

impl PendingEntryChoiceKeywordTransaction {
    pub fn resolve(
        self,
        input: EntryChoiceResolutionInput,
    ) -> Result<EntryChoiceKeywordResolution, EntryChoiceRuntimeError> {
        if input.chooser != self.attempt.controller {
            return Err(EntryChoiceRuntimeError::WrongEntryChooser {
                expected: self.attempt.controller,
                actual: input.chooser,
            });
        }
        let order = exact_binding_order(&input.application_order, &self.bindings)?;
        let choices = index_choices(&input.choices, &self.bindings)?;
        let placements = index_placements(
            &input.counter_placements,
            self.attempt.entering_object(),
            &self.bindings,
        )?;

        let needs_damage_history = self.bindings.values().any(|binding| {
            matches!(
                binding.program.kind(),
                EntryChoiceKeywordKind::BloodthirstFixed { .. }
                    | EntryChoiceKeywordKind::BloodthirstOpponentDamageTotal
            )
        });
        let damage_history = match (&self.attempt.damage_history, needs_damage_history) {
            (Some(history), true) => {
                history.validate_for(self.attempt.controller)?;
                Some(history)
            }
            (None, true) => return Err(EntryChoiceRuntimeError::MissingDamageHistory),
            (Some(_), false) | (None, false) => None,
        };

        let needs_x = self
            .bindings
            .values()
            .any(|binding| binding.program.kind() == EntryChoiceKeywordKind::Ravenous);
        let ravenous_x = match (&self.attempt.x_evidence, needs_x) {
            (Some(evidence), true) => Some(evidence.ravenous_x(self.attempt.source)?),
            (None, true) => return Err(EntryChoiceRuntimeError::MissingSpellXEvidence),
            (Some(_), false) | (None, false) => None,
        };

        let entering_object = self.attempt.entering_object();
        let mut plus_one_counters = self.attempt.prior_plus_one_counters;
        let mut receipts = Vec::with_capacity(order.len());
        let mut unleash_restrictions = Vec::new();
        let mut riot_haste_grants = Vec::new();
        let mut ravenous_draw_triggers = Vec::new();
        let mut used_placements = BTreeSet::new();

        for binding_id in order {
            let binding = self
                .bindings
                .get(&binding_id)
                .expect("validated application order");
            let (decision, requested_counters) = match binding.program.kind() {
                EntryChoiceKeywordKind::Unleash => {
                    let choice = choices
                        .get(&binding_id)
                        .copied()
                        .ok_or(EntryChoiceRuntimeError::MissingEntryChoice(binding_id))?;
                    let KeywordEntryChoice::Unleash(choice) = choice else {
                        return Err(EntryChoiceRuntimeError::WrongEntryChoiceKind(binding_id));
                    };
                    unleash_restrictions.push(UnleashBlockingRestriction {
                        binding_id,
                        object: entering_object,
                        semantic_digest: binding.program.semantic_digest().to_owned(),
                        duration: UnleashRestrictionDuration::
                            WhileSameBattlefieldIncarnationHasPlusOneCounterAndUnleashAbility,
                    });
                    (
                        KeywordEntryDecision::Unleash(choice),
                        u32::from(choice == UnleashEntryChoice::AddCounter),
                    )
                }
                EntryChoiceKeywordKind::Riot => {
                    let choice = choices
                        .get(&binding_id)
                        .copied()
                        .ok_or(EntryChoiceRuntimeError::MissingEntryChoice(binding_id))?;
                    let KeywordEntryChoice::Riot(choice) = choice else {
                        return Err(EntryChoiceRuntimeError::WrongEntryChoiceKind(binding_id));
                    };
                    if choice == RiotEntryChoice::GainHaste {
                        riot_haste_grants.push(RiotHasteGrant {
                            binding_id,
                            object: entering_object,
                            semantic_digest: binding.program.semantic_digest().to_owned(),
                            duration: RiotHasteDuration::SameBattlefieldIncarnation,
                        });
                    }
                    (
                        KeywordEntryDecision::Riot(choice),
                        u32::from(choice == RiotEntryChoice::AddCounter),
                    )
                }
                EntryChoiceKeywordKind::BloodthirstFixed { counters } => {
                    let condition_met = damage_history
                        .expect("validated damage history")
                        .any_opponent_was_dealt_damage();
                    (
                        KeywordEntryDecision::BloodthirstConditionMet(condition_met),
                        if condition_met { counters } else { 0 },
                    )
                }
                EntryChoiceKeywordKind::BloodthirstOpponentDamageTotal => {
                    let total = damage_history
                        .expect("validated damage history")
                        .total_damage_dealt_to_opponents()?;
                    (KeywordEntryDecision::BloodthirstDamageTotal(total), total)
                }
                EntryChoiceKeywordKind::Ravenous => {
                    let x = ravenous_x.expect("validated Ravenous X evidence");
                    if x >= 5 {
                        ravenous_draw_triggers.push(PendingRavenousDrawTrigger {
                            trigger_id: RavenousTriggerId {
                                entry_event_id: self.attempt.event_id,
                                binding_id,
                            },
                            controller: self.attempt.controller,
                            source_at_trigger: entering_object,
                            bound_x: x,
                            semantic_digest: binding.program.semantic_digest().to_owned(),
                        });
                    }
                    (KeywordEntryDecision::RavenousX(x), x)
                }
            };

            let counters_placed = if requested_counters == 0 {
                if placements.contains_key(&binding_id) {
                    return Err(EntryChoiceRuntimeError::UnexpectedCounterPlacementEvidence(
                        binding_id,
                    ));
                }
                0
            } else {
                let placement = placements.get(&binding_id).ok_or(
                    EntryChoiceRuntimeError::MissingCounterPlacementEvidence(binding_id),
                )?;
                if !placement.replacement_effects_complete {
                    return Err(
                        EntryChoiceRuntimeError::IncompleteCounterReplacementEvidence(binding_id),
                    );
                }
                if placement.requested_counters != requested_counters {
                    return Err(EntryChoiceRuntimeError::CounterRequestMismatch {
                        binding_id,
                        expected: requested_counters,
                        actual: placement.requested_counters,
                    });
                }
                used_placements.insert(binding_id);
                placement.counters_placed
            };
            plus_one_counters = plus_one_counters
                .checked_add(counters_placed)
                .ok_or(EntryChoiceRuntimeError::CounterQuantityOverflow)?;
            receipts.push(KeywordEntryReceipt {
                binding_id,
                semantic_digest: binding.program.semantic_digest().to_owned(),
                decision,
                requested_counters,
                counters_placed,
            });
        }

        if let Some(unused) = placements
            .keys()
            .find(|binding_id| !used_placements.contains(binding_id))
        {
            return Err(EntryChoiceRuntimeError::UnexpectedCounterPlacementEvidence(
                *unused,
            ));
        }
        if let Some(unused) = choices.keys().find(|binding_id| {
            !matches!(
                self.bindings
                    .get(binding_id)
                    .map(|binding| binding.program.kind()),
                Some(EntryChoiceKeywordKind::Unleash | EntryChoiceKeywordKind::Riot)
            )
        }) {
            return Err(EntryChoiceRuntimeError::UnexpectedEntryChoice(*unused));
        }

        unleash_restrictions.sort_by_key(|restriction| restriction.binding_id);
        riot_haste_grants.sort_by_key(|grant| grant.binding_id);
        ravenous_draw_triggers.sort_by_key(|trigger| trigger.trigger_id);
        let effective_riot_haste =
            (!riot_haste_grants.is_empty()).then(|| EffectiveRiotHasteGrant {
                object: entering_object,
                contributing_bindings: riot_haste_grants
                    .iter()
                    .map(|grant| grant.binding_id)
                    .collect(),
            });
        Ok(EntryChoiceKeywordResolution {
            event_id: self.attempt.event_id,
            entering_object,
            controller: self.attempt.controller,
            plus_one_counters,
            receipts,
            unleash_restrictions,
            riot_haste_grants,
            effective_riot_haste,
            ravenous_draw_triggers,
        })
    }
}

fn exact_binding_order(
    order: &[BindingId],
    bindings: &BTreeMap<BindingId, EntryChoiceKeywordBinding>,
) -> Result<Vec<BindingId>, EntryChoiceRuntimeError> {
    if order.len() != bindings.len() {
        return Err(EntryChoiceRuntimeError::IncompleteApplicationOrder);
    }
    let mut seen = BTreeSet::new();
    for binding_id in order {
        if !bindings.contains_key(binding_id) {
            return Err(EntryChoiceRuntimeError::UnknownBinding(*binding_id));
        }
        if !seen.insert(*binding_id) {
            return Err(EntryChoiceRuntimeError::DuplicateApplication(*binding_id));
        }
    }
    Ok(order.to_vec())
}

fn index_choices(
    choices: &[BindingEntryChoice],
    bindings: &BTreeMap<BindingId, EntryChoiceKeywordBinding>,
) -> Result<BTreeMap<BindingId, KeywordEntryChoice>, EntryChoiceRuntimeError> {
    let mut indexed = BTreeMap::new();
    for choice in choices {
        if !bindings.contains_key(&choice.binding_id) {
            return Err(EntryChoiceRuntimeError::UnknownBinding(choice.binding_id));
        }
        if indexed.insert(choice.binding_id, choice.choice).is_some() {
            return Err(EntryChoiceRuntimeError::DuplicateEntryChoice(
                choice.binding_id,
            ));
        }
    }
    Ok(indexed)
}

fn index_placements(
    placements: &[CounterPlacementEvidence],
    entering_object: ObjectRef,
    bindings: &BTreeMap<BindingId, EntryChoiceKeywordBinding>,
) -> Result<BTreeMap<BindingId, CounterPlacementEvidence>, EntryChoiceRuntimeError> {
    let mut indexed = BTreeMap::new();
    for placement in placements {
        if !bindings.contains_key(&placement.binding_id) {
            return Err(EntryChoiceRuntimeError::UnknownBinding(
                placement.binding_id,
            ));
        }
        if placement.entering_object != entering_object {
            return Err(EntryChoiceRuntimeError::CounterPlacementObjectMismatch {
                binding_id: placement.binding_id,
                expected: entering_object,
                actual: placement.entering_object,
            });
        }
        if indexed.insert(placement.binding_id, *placement).is_some() {
            return Err(EntryChoiceRuntimeError::DuplicateCounterPlacementEvidence(
                placement.binding_id,
            ));
        }
    }
    Ok(indexed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnleashBlockingStateEvidence {
    pub object: ObjectRef,
    pub on_battlefield: bool,
    pub plus_one_counters: u32,
    pub active_unleash_bindings: BTreeSet<BindingId>,
    pub ability_state_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockingPermission {
    Allowed,
    ProhibitedByUnleash { bindings: Vec<BindingId> },
}

pub fn evaluate_unleash_blocking_permission(
    restrictions: &[UnleashBlockingRestriction],
    state: &UnleashBlockingStateEvidence,
) -> Result<BlockingPermission, EntryChoiceRuntimeError> {
    if !state.ability_state_complete {
        return Err(EntryChoiceRuntimeError::IncompleteUnleashAbilityState);
    }
    if !state.on_battlefield || state.plus_one_counters == 0 {
        return Ok(BlockingPermission::Allowed);
    }
    let mut bindings = restrictions
        .iter()
        .filter(|restriction| {
            restriction.object == state.object
                && state
                    .active_unleash_bindings
                    .contains(&restriction.binding_id)
        })
        .map(|restriction| restriction.binding_id)
        .collect::<Vec<_>>();
    bindings.sort_unstable();
    bindings.dedup();
    if bindings.is_empty() {
        Ok(BlockingPermission::Allowed)
    } else {
        Ok(BlockingPermission::ProhibitedByUnleash { bindings })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RavenousTriggerDisposition {
    Resolve,
    Countered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RavenousTriggerResolution {
    Draw {
        trigger_id: RavenousTriggerId,
        player: PlayerId,
        cards: u32,
    },
    Countered {
        trigger_id: RavenousTriggerId,
    },
}

pub fn resolve_ravenous_draw_trigger(
    trigger: PendingRavenousDrawTrigger,
    disposition: RavenousTriggerDisposition,
) -> Result<RavenousTriggerResolution, EntryChoiceRuntimeError> {
    if trigger.bound_x < 5 {
        return Err(EntryChoiceRuntimeError::InvalidRavenousTriggerX(
            trigger.bound_x,
        ));
    }
    Ok(match disposition {
        RavenousTriggerDisposition::Resolve => RavenousTriggerResolution::Draw {
            trigger_id: trigger.trigger_id,
            player: trigger.controller,
            cards: 1,
        },
        RavenousTriggerDisposition::Countered => RavenousTriggerResolution::Countered {
            trigger_id: trigger.trigger_id,
        },
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryChoiceRuntimeError {
    IncompleteKeywordInstanceCensus,
    ReusedIncarnation(IncarnationId),
    DuplicateBinding,
    BindingSourceMismatch {
        binding_id: BindingId,
        expected: ObjectRef,
        actual: ObjectRef,
    },
    WrongEntryChooser {
        expected: PlayerId,
        actual: PlayerId,
    },
    IncompleteApplicationOrder,
    UnknownBinding(BindingId),
    DuplicateApplication(BindingId),
    DuplicateEntryChoice(BindingId),
    MissingEntryChoice(BindingId),
    UnexpectedEntryChoice(BindingId),
    WrongEntryChoiceKind(BindingId),
    MissingDamageHistory,
    IncompleteDamageHistory,
    IncompleteOpponentRelationships,
    DamageHistoryControllerMismatch {
        expected: PlayerId,
        actual: PlayerId,
    },
    ControllerListedAsOpponent(PlayerId),
    MissingSpellXEvidence,
    IncompleteSpellXEvidence,
    IncompleteEntryProvenance,
    ResolvingSpellMismatch {
        expected: ObjectRef,
        actual: ObjectRef,
    },
    DuplicateCounterPlacementEvidence(BindingId),
    MissingCounterPlacementEvidence(BindingId),
    UnexpectedCounterPlacementEvidence(BindingId),
    IncompleteCounterReplacementEvidence(BindingId),
    CounterPlacementObjectMismatch {
        binding_id: BindingId,
        expected: ObjectRef,
        actual: ObjectRef,
    },
    CounterRequestMismatch {
        binding_id: BindingId,
        expected: u32,
        actual: u32,
    },
    CounterQuantityOverflow,
    IncompleteUnleashAbilityState,
    InvalidRavenousTriggerX(u32),
}

impl fmt::Display for EntryChoiceRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EntryChoiceRuntimeError {}
