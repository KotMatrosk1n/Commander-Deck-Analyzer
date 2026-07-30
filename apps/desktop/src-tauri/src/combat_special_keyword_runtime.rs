//! Exact standalone rules programs for Ninjutsu, Encore, and Saddle.
//!
//! Only complete, reviewed Oracle keyword clauses are accepted. Granted
//! abilities, cost modifiers, variants, compound clauses, and reminderless
//! keyword actions stay rejected. The runtime preserves the hidden-zone,
//! combat, multiplayer, cost, stack, copy, token, delayed-trigger, and object
//! incarnation boundaries required by the three keyword abilities. Nothing in
//! this module is connected to the production simulator yet.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const COMBAT_SPECIAL_KEYWORD_COMPILER_VERSION: &str = "combat-special-keyword-compiler-0.1";
pub const COMBAT_SPECIAL_KEYWORD_RUNTIME_VERSION: &str = "combat-special-keyword-runtime-0.1";
pub const COMBAT_SPECIAL_KEYWORD_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:117,118,400.7,506.3,508.1,508.4,509.1h,\
     602,603.7,707,702.49,702.141,702.171";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type IncarnationId = u64;
pub type ActivationId = u64;
pub type StackObjectId = u64;
pub type EventId = u64;
pub type CombatId = u64;
pub type TurnId = u64;
pub type EndStepId = u64;
pub type BindingId = u64;
pub type ManaUnitId = u64;
pub type TriggerId = u64;

/// Recognition is not execution coverage. The production engine does not yet
/// supply the complete evidence required by this runtime.
pub const fn combat_special_keyword_production_adapter_connected() -> bool {
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CombatSpecialKeywordFamily {
    Ninjutsu,
    Encore,
    Saddle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl ManaColor {
    fn code(self) -> &'static str {
        match self {
            Self::White => "W",
            Self::Blue => "U",
            Self::Black => "B",
            Self::Red => "R",
            Self::Green => "G",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaSymbol {
    Generic(u32),
    Colored(ManaColor),
    Colorless,
    Snow,
    Hybrid(ManaColor, ManaColor),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManaCost {
    pub symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    pub fn oracle_text(&self) -> String {
        let mut output = String::new();
        for symbol in &self.symbols {
            match symbol {
                ManaSymbol::Generic(amount) => output.push_str(&format!("{{{amount}}}")),
                ManaSymbol::Colored(color) => {
                    output.push('{');
                    output.push_str(color.code());
                    output.push('}');
                }
                ManaSymbol::Colorless => output.push_str("{C}"),
                ManaSymbol::Snow => output.push_str("{S}"),
                ManaSymbol::Hybrid(first, second) => {
                    output.push('{');
                    output.push_str(first.code());
                    output.push('/');
                    output.push_str(second.code());
                    output.push('}');
                }
            }
        }
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatSpecialKeywordKind {
    Ninjutsu { cost: ManaCost },
    Encore { cost: ManaCost },
    Saddle { threshold: u32 },
}

impl CombatSpecialKeywordKind {
    pub const fn family(&self) -> CombatSpecialKeywordFamily {
        match self {
            Self::Ninjutsu { .. } => CombatSpecialKeywordFamily::Ninjutsu,
            Self::Encore { .. } => CombatSpecialKeywordFamily::Encore,
            Self::Saddle { .. } => CombatSpecialKeywordFamily::Saddle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatSpecialKeywordProgram {
    exact_source: String,
    normalized_source: String,
    semantic_digest: String,
    kind: CombatSpecialKeywordKind,
}

impl CombatSpecialKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &CombatSpecialKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        combat_special_keyword_production_adapter_connected()
    }
}

/// Reserved for an exact clause that a compiler earlier in the backend order
/// already owns. The installed snapshot currently has no such occurrence in
/// these three families, but the classification keeps that boundary explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlierCombatSpecialClauseOwner {
    OfficialKeywordRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatSpecialClauseClassification {
    Program(CombatSpecialKeywordProgram),
    EarlierOwner {
        family: CombatSpecialKeywordFamily,
        owner: EarlierCombatSpecialClauseOwner,
    },
    Rejected,
}

pub fn compile_combat_special_keyword_program(
    exact_source: &str,
    normalized_source: &str,
) -> Option<CombatSpecialKeywordProgram> {
    match classify_combat_special_keyword_clause(exact_source, normalized_source) {
        CombatSpecialClauseClassification::Program(program) => Some(program),
        CombatSpecialClauseClassification::EarlierOwner { .. }
        | CombatSpecialClauseClassification::Rejected => None,
    }
}

pub fn classify_combat_special_keyword_clause(
    exact_source: &str,
    normalized_source: &str,
) -> CombatSpecialClauseClassification {
    if !is_complete_single_line(exact_source)
        || !is_complete_single_line(normalized_source)
        || !normalized_source_is_content_derived(exact_source, normalized_source)
    {
        return CombatSpecialClauseClassification::Rejected;
    }

    let kind = if let Some(cost_text) = exact_source
        .strip_prefix("Ninjutsu ")
        .and_then(|remainder| remainder.split_once(" ("))
        .and_then(|(cost, _)| {
            let expected = format!(
                "Ninjutsu {cost} ({cost}, Return an unblocked attacker you control to hand: \
                 Put this card onto the battlefield from your hand tapped and attacking.)"
            );
            (exact_source == expected).then_some(cost)
        }) {
        parse_mana_cost(cost_text).map(|cost| CombatSpecialKeywordKind::Ninjutsu { cost })
    } else if let Some(cost_text) = exact_source
        .strip_prefix("Encore ")
        .and_then(|remainder| remainder.split_once(" ("))
        .and_then(|(cost, _)| {
            let expected = format!(
                "Encore {cost} ({cost}, Exile this card from your graveyard: For each opponent, \
                 create a token copy that attacks that opponent this turn if able. They gain \
                 haste. Sacrifice them at the beginning of the next end step. Activate only as \
                 a sorcery.)"
            );
            (exact_source == expected).then_some(cost)
        })
    {
        parse_mana_cost(cost_text).map(|cost| CombatSpecialKeywordKind::Encore { cost })
    } else {
        exact_source
            .strip_prefix("Saddle ")
            .and_then(|remainder| remainder.split_once(" ("))
            .and_then(|(number, _)| parse_positive_u32(number))
            .filter(|threshold| {
                exact_source
                    == format!(
                        "Saddle {threshold} (Tap any number of other creatures you control with \
                     total power {threshold} or more: This Mount becomes saddled until end of \
                     turn. Saddle only as a sorcery.)"
                    )
            })
            .map(|threshold| CombatSpecialKeywordKind::Saddle { threshold })
    };

    let Some(kind) = kind else {
        return CombatSpecialClauseClassification::Rejected;
    };
    let semantic_digest = combat_special_semantic_digest(exact_source, &kind);
    CombatSpecialClauseClassification::Program(CombatSpecialKeywordProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        semantic_digest,
        kind,
    })
}

fn is_complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source.contains(['\r', '\n'])
        && collapse_whitespace(source) == source
}

fn normalized_source_is_content_derived(exact_source: &str, normalized_source: &str) -> bool {
    normalized_source == exact_source
        || normalized_source == reviewed_normalized_source(exact_source)
}

pub fn reviewed_normalized_source(exact_source: &str) -> String {
    let source = collapse_whitespace(&exact_source.replace('\u{2019}', "'"));
    replace_ascii_case_insensitive(&source, "this card", "this object")
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn replace_ascii_case_insensitive(source: &str, needle: &str, replacement: &str) -> String {
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

fn parse_positive_u32(value: &str) -> Option<u32> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    let value = value.parse::<u32>().ok()?;
    (value > 0).then_some(value)
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() {
        return None;
    }
    let mut symbols = Vec::new();
    let mut remainder = source;
    while let Some(after_open) = remainder.strip_prefix('{') {
        let close = after_open.find('}')?;
        let token = &after_open[..close];
        let symbol = if let Some(amount) = parse_nonnegative_u32(token) {
            ManaSymbol::Generic(amount)
        } else if let Some(color) = parse_color(token) {
            ManaSymbol::Colored(color)
        } else if token == "C" {
            ManaSymbol::Colorless
        } else if token == "S" {
            ManaSymbol::Snow
        } else if let Some((first, second)) = token
            .split_once('/')
            .and_then(|(first, second)| Some((parse_color(first)?, parse_color(second)?)))
        {
            (first != second).then_some(ManaSymbol::Hybrid(first, second))?
        } else {
            return None;
        };
        symbols.push(symbol);
        remainder = &after_open[close + 1..];
    }
    if !remainder.is_empty() || symbols.is_empty() {
        return None;
    }
    Some(ManaCost { symbols })
}

fn parse_nonnegative_u32(value: &str) -> Option<u32> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse::<u32>().ok()
}

fn parse_color(value: &str) -> Option<ManaColor> {
    match value {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        _ => None,
    }
}

fn combat_special_semantic_digest(exact_source: &str, kind: &CombatSpecialKeywordKind) -> String {
    let contract = match kind {
        CombatSpecialKeywordKind::Ninjutsu { cost } => format!(
            "ninjutsu/v1;base-cost={};zone=hand;reveal-until-stack-exit;\
             additional-cost=return-owned-hand-unblocked-attacker-controlled;\
             resolution=enter-tapped-attacking-captured-defender",
            cost.oracle_text()
        ),
        CombatSpecialKeywordKind::Encore { cost } => format!(
            "encore/v1;base-cost={};zone=graveyard;sorcery-speed;additional-cost=exile-source;\
             resolution=one-copy-token-per-current-opponent;haste;assigned-opponent-attack-\
             requirement;delayed-next-end-step-sacrifice",
            cost.oracle_text()
        ),
        CombatSpecialKeywordKind::Saddle { threshold } => format!(
            "saddle/v1;threshold={threshold};sorcery-speed;cost=tap-any-positive-number-of-other-\
             untapped-controlled-creatures-with-current-total-power-at-least-threshold;\
             resolution=saddled-until-end-of-turn;designation-noncopiable"
        ),
    };
    let mut hasher = Sha256::new();
    for component in [
        "combat-special-keyword-content/v1",
        COMBAT_SPECIAL_KEYWORD_COMPILER_VERSION,
        COMBAT_SPECIAL_KEYWORD_RUNTIME_VERSION,
        COMBAT_SPECIAL_KEYWORD_RULES_CONTEXT_VERSION,
        exact_source,
        &contract,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Command,
    Stack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttackTarget {
    Player(PlayerId),
    Planeswalker(ObjectRef),
    Battle(ObjectRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackablePermanentKind {
    Planeswalker,
    Battle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackablePermanentEvidence {
    pub object: ObjectRef,
    pub zone: Zone,
    pub kind: AttackablePermanentKind,
    pub controller: PlayerId,
    pub battle_protector: Option<PlayerId>,
    pub characteristics_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatTopologyEvidence {
    pub attacker_controller: PlayerId,
    pub players_in_game: BTreeSet<PlayerId>,
    pub opponents: BTreeSet<PlayerId>,
    pub defending_players: BTreeSet<PlayerId>,
    pub attackable_permanents: BTreeMap<ObjectRef, AttackablePermanentEvidence>,
    pub relationships_complete: bool,
    pub range_of_influence_complete: bool,
}

impl CombatTopologyEvidence {
    fn validate(&self) -> Result<(), CombatSpecialRuntimeError> {
        if !self.relationships_complete || !self.range_of_influence_complete {
            return Err(CombatSpecialRuntimeError::IncompleteCombatTopology);
        }
        if self.opponents.contains(&self.attacker_controller)
            || self.defending_players.contains(&self.attacker_controller)
            || !self.players_in_game.contains(&self.attacker_controller)
            || !self.defending_players.is_subset(&self.opponents)
            || !self.opponents.is_subset(&self.players_in_game)
        {
            return Err(CombatSpecialRuntimeError::InvalidCombatTopology);
        }
        Ok(())
    }

    pub fn target_is_currently_attackable(
        &self,
        target: AttackTarget,
    ) -> Result<bool, CombatSpecialRuntimeError> {
        self.validate()?;
        Ok(match target {
            AttackTarget::Player(player) => {
                self.players_in_game.contains(&player) && self.defending_players.contains(&player)
            }
            AttackTarget::Planeswalker(object) => self
                .attackable_permanents
                .get(&object)
                .is_some_and(|permanent| {
                    permanent.characteristics_complete
                        && permanent.zone == Zone::Battlefield
                        && permanent.kind == AttackablePermanentKind::Planeswalker
                        && permanent.object == object
                        && self.defending_players.contains(&permanent.controller)
                }),
            AttackTarget::Battle(object) => {
                self.attackable_permanents
                    .get(&object)
                    .is_some_and(|permanent| {
                        permanent.characteristics_complete
                            && permanent.zone == Zone::Battlefield
                            && permanent.kind == AttackablePermanentKind::Battle
                            && permanent.object == object
                            && permanent.battle_protector.is_some_and(|protector| {
                                self.defending_players.contains(&protector)
                            })
                    })
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManaKind {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentUnit {
    pub unit_id: ManaUnitId,
    pub kind: ManaKind,
    pub from_snow_source: bool,
    pub legal_for_this_activation: bool,
    pub spending_restriction_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentEvidence {
    pub payer: PlayerId,
    pub printed_base_cost: ManaCost,
    /// The result after all increases, reductions, and alternative payment
    /// rules have been applied by the main cost engine.
    pub final_cost: ManaCost,
    pub modifiers_complete: bool,
    pub final_cost_legally_determined: bool,
    pub mana_ability_window_complete: bool,
    pub paid_units: Vec<ManaPaymentUnit>,
}

fn validate_mana_payment(
    payment: &ManaPaymentEvidence,
    payer: PlayerId,
    printed_cost: &ManaCost,
) -> Result<(), CombatSpecialRuntimeError> {
    if payment.payer != payer {
        return Err(CombatSpecialRuntimeError::WrongPayer {
            expected: payer,
            actual: payment.payer,
        });
    }
    if &payment.printed_base_cost != printed_cost {
        return Err(CombatSpecialRuntimeError::PrintedManaCostMismatch);
    }
    if !payment.modifiers_complete
        || !payment.final_cost_legally_determined
        || !payment.mana_ability_window_complete
    {
        return Err(CombatSpecialRuntimeError::IncompleteManaCostEvidence);
    }
    let mut unit_ids = BTreeSet::new();
    if payment.paid_units.iter().any(|unit| {
        !unit_ids.insert(unit.unit_id)
            || !unit.legal_for_this_activation
            || !unit.spending_restriction_evidence_complete
    }) {
        return Err(CombatSpecialRuntimeError::InvalidManaPayment);
    }
    let requirements = expanded_mana_requirements(&payment.final_cost)?;
    if requirements.len() != payment.paid_units.len()
        || !mana_assignment_exists(&requirements, &payment.paid_units, 0, &mut BTreeSet::new())
    {
        return Err(CombatSpecialRuntimeError::InvalidManaPayment);
    }
    Ok(())
}

fn expanded_mana_requirements(
    cost: &ManaCost,
) -> Result<Vec<ManaSymbol>, CombatSpecialRuntimeError> {
    let mut requirements = Vec::new();
    for symbol in &cost.symbols {
        match symbol {
            ManaSymbol::Generic(amount) => {
                let amount = usize::try_from(*amount)
                    .map_err(|_| CombatSpecialRuntimeError::ManaQuantityOverflow)?;
                requirements.extend(std::iter::repeat_n(ManaSymbol::Generic(1), amount));
            }
            other => requirements.push(*other),
        }
    }
    Ok(requirements)
}

fn mana_assignment_exists(
    requirements: &[ManaSymbol],
    units: &[ManaPaymentUnit],
    index: usize,
    used: &mut BTreeSet<usize>,
) -> bool {
    if index == requirements.len() {
        return true;
    }
    for (unit_index, unit) in units.iter().enumerate() {
        if !used.contains(&unit_index) && mana_unit_satisfies(unit, requirements[index]) {
            used.insert(unit_index);
            if mana_assignment_exists(requirements, units, index + 1, used) {
                return true;
            }
            used.remove(&unit_index);
        }
    }
    false
}

fn mana_unit_satisfies(unit: &ManaPaymentUnit, requirement: ManaSymbol) -> bool {
    match requirement {
        ManaSymbol::Generic(_) => true,
        ManaSymbol::Colored(color) => mana_kind_matches_color(unit.kind, color),
        ManaSymbol::Colorless => unit.kind == ManaKind::Colorless,
        ManaSymbol::Snow => unit.from_snow_source,
        ManaSymbol::Hybrid(first, second) => {
            mana_kind_matches_color(unit.kind, first) || mana_kind_matches_color(unit.kind, second)
        }
    }
}

fn mana_kind_matches_color(kind: ManaKind, color: ManaColor) -> bool {
    matches!(
        (kind, color),
        (ManaKind::White, ManaColor::White)
            | (ManaKind::Blue, ManaColor::Blue)
            | (ManaKind::Black, ManaColor::Black)
            | (ManaKind::Red, ManaColor::Red)
            | (ManaKind::Green, ManaColor::Green)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SorceryTimingEvidence {
    pub actor: PlayerId,
    pub active_player: PlayerId,
    pub phase_is_precombat_or_postcombat_main: bool,
    pub stack_is_empty: bool,
    pub actor_has_priority: bool,
    pub turn_based_actions_complete: bool,
}

fn validate_sorcery_timing(
    evidence: &SorceryTimingEvidence,
    actor: PlayerId,
) -> Result<(), CombatSpecialRuntimeError> {
    if evidence.actor != actor
        || evidence.active_player != actor
        || !evidence.phase_is_precombat_or_postcombat_main
        || !evidence.stack_is_empty
        || !evidence.actor_has_priority
        || !evidence.turn_based_actions_complete
    {
        return Err(CombatSpecialRuntimeError::SorceryTimingUnavailable);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneMoveEvidence {
    pub object_before: ObjectRef,
    pub object_after: Option<ObjectRef>,
    pub owner: PlayerId,
    pub controller_before: Option<PlayerId>,
    pub from: Zone,
    pub to: Zone,
    pub move_succeeded: bool,
    pub replacement_effects_complete: bool,
}

fn validate_paid_zone_move(
    evidence: &ZoneMoveEvidence,
    expected_object: ObjectRef,
    from: Zone,
    to: Zone,
) -> Result<ObjectRef, CombatSpecialRuntimeError> {
    if evidence.object_before != expected_object
        || evidence.from != from
        || evidence.to != to
        || !evidence.move_succeeded
        || !evidence.replacement_effects_complete
    {
        return Err(CombatSpecialRuntimeError::InvalidCostZoneMove);
    }
    let after = evidence
        .object_after
        .ok_or(CombatSpecialRuntimeError::InvalidCostZoneMove)?;
    if after.object_id != expected_object.object_id
        || after.incarnation_id == expected_object.incarnation_id
    {
        return Err(CombatSpecialRuntimeError::InvalidCostZoneMove);
    }
    Ok(after)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneObjectEvidence {
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: Option<PlayerId>,
    pub zone: Zone,
    pub zone_player: Option<PlayerId>,
    pub characteristics_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatAttackerEvidence {
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub is_creature: bool,
    pub is_attacking: bool,
    pub is_unblocked: bool,
    pub combat_id: CombatId,
    pub attack_target: AttackTarget,
    pub block_status_complete: bool,
    pub attack_target_history_complete: bool,
    pub combat_is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NinjutsuActivationInput {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub actor: PlayerId,
    pub actor_has_priority: bool,
    pub source: ZoneObjectEvidence,
    pub source_is_revealed_before_activation: bool,
    pub attacker: CombatAttackerEvidence,
    pub topology: CombatTopologyEvidence,
    pub all_activation_choices_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingNinjutsuActivation {
    program: CombatSpecialKeywordProgram,
    input: NinjutsuActivationInput,
}

pub fn begin_ninjutsu_activation(
    program: &CombatSpecialKeywordProgram,
    input: NinjutsuActivationInput,
) -> Result<PendingNinjutsuActivation, CombatSpecialRuntimeError> {
    if !matches!(program.kind(), CombatSpecialKeywordKind::Ninjutsu { .. }) {
        return Err(CombatSpecialRuntimeError::WrongProgramKind);
    }
    if !input.actor_has_priority || !input.all_activation_choices_complete {
        return Err(CombatSpecialRuntimeError::IncompleteActivationEvidence);
    }
    if input.source.zone != Zone::Hand
        || input.source.zone_player != Some(input.actor)
        || input.source.owner != input.actor
        || !input.source.characteristics_complete
    {
        return Err(CombatSpecialRuntimeError::InvalidNinjutsuSource);
    }
    let attacker = &input.attacker;
    if attacker.zone != Zone::Battlefield
        || attacker.controller != input.actor
        || !attacker.is_creature
        || !attacker.is_attacking
        || !attacker.is_unblocked
        || !attacker.block_status_complete
        || !attacker.attack_target_history_complete
        || !attacker.combat_is_active
    {
        return Err(CombatSpecialRuntimeError::NoEligibleUnblockedAttacker);
    }
    if input.topology.attacker_controller != input.actor {
        return Err(CombatSpecialRuntimeError::InvalidCombatTopology);
    }
    input.topology.validate()?;
    Ok(PendingNinjutsuActivation {
        program: program.clone(),
        input,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NinjutsuStackAbility {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub controller: PlayerId,
    pub source_in_hand: ObjectRef,
    pub source_owner: PlayerId,
    pub returned_attacker_before: ObjectRef,
    pub returned_attacker_in_owner_hand: ObjectRef,
    pub captured_attack_target: AttackTarget,
    pub combat_id: CombatId,
    pub source_remains_revealed: bool,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NinjutsuActivationReceipt {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub source_revealed: ObjectRef,
    pub source_was_already_revealed: bool,
    pub reveal_public_to_all_players: bool,
    pub reveal_duration_until_stack_exit: bool,
    pub returned_attacker_before: ObjectRef,
    pub returned_attacker_after: ObjectRef,
    pub returned_to_owner: PlayerId,
    pub captured_attack_target: AttackTarget,
    pub semantic_digest: String,
}

impl PendingNinjutsuActivation {
    pub fn commit(
        self,
        payment: ManaPaymentEvidence,
        returned_attacker: ZoneMoveEvidence,
    ) -> Result<(NinjutsuStackAbility, NinjutsuActivationReceipt), CombatSpecialRuntimeError> {
        let CombatSpecialKeywordKind::Ninjutsu { cost } = self.program.kind() else {
            return Err(CombatSpecialRuntimeError::WrongProgramKind);
        };
        validate_mana_payment(&payment, self.input.actor, cost)?;
        if returned_attacker.owner != self.input.attacker.owner
            || returned_attacker.controller_before != Some(self.input.actor)
        {
            return Err(CombatSpecialRuntimeError::InvalidReturnedAttackerOwner);
        }
        let attacker_after = validate_paid_zone_move(
            &returned_attacker,
            self.input.attacker.object,
            Zone::Battlefield,
            Zone::Hand,
        )?;
        let ability = NinjutsuStackAbility {
            activation_id: self.input.activation_id,
            stack_id: self.input.stack_id,
            controller: self.input.actor,
            source_in_hand: self.input.source.object,
            source_owner: self.input.source.owner,
            returned_attacker_before: self.input.attacker.object,
            returned_attacker_in_owner_hand: attacker_after,
            captured_attack_target: self.input.attacker.attack_target,
            combat_id: self.input.attacker.combat_id,
            source_remains_revealed: true,
            semantic_digest: self.program.semantic_digest.clone(),
        };
        let receipt = NinjutsuActivationReceipt {
            activation_id: self.input.activation_id,
            stack_id: self.input.stack_id,
            source_revealed: self.input.source.object,
            source_was_already_revealed: self.input.source_is_revealed_before_activation,
            reveal_public_to_all_players: true,
            reveal_duration_until_stack_exit: true,
            returned_attacker_before: self.input.attacker.object,
            returned_attacker_after: attacker_after,
            returned_to_owner: self.input.attacker.owner,
            captured_attack_target: self.input.attacker.attack_target,
            semantic_digest: self.program.semantic_digest,
        };
        Ok((ability, receipt))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NinjutsuEntryEvidence {
    pub source_before: ObjectRef,
    pub object_after: Option<ObjectRef>,
    pub destination: Zone,
    pub controller_after: Option<PlayerId>,
    pub owner: PlayerId,
    pub entered_tapped: bool,
    pub is_creature_after_entry: bool,
    pub is_battle_after_entry: bool,
    pub characteristics_complete: bool,
    pub replacement_effects_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NinjutsuResolutionInput {
    pub stack_object_is_resolving: bool,
    pub source_current: Option<ZoneObjectEvidence>,
    pub entry: Option<NinjutsuEntryEvidence>,
    pub topology: CombatTopologyEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NinjutsuResolutionReceipt {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub source_before: ObjectRef,
    pub battlefield_object: Option<ObjectRef>,
    pub entered_battlefield: bool,
    pub entered_tapped: bool,
    pub entered_attacking: bool,
    pub entered_unblocked: bool,
    pub attack_target: Option<AttackTarget>,
    pub attack_declaration_triggers_fire: bool,
    pub source_reveal_ended: bool,
    pub semantic_digest: String,
}

pub fn resolve_ninjutsu(
    ability: NinjutsuStackAbility,
    input: NinjutsuResolutionInput,
) -> Result<NinjutsuResolutionReceipt, CombatSpecialRuntimeError> {
    if !input.stack_object_is_resolving || !ability.source_remains_revealed {
        return Err(CombatSpecialRuntimeError::InvalidStackState);
    }
    input.topology.validate()?;
    if input.topology.attacker_controller != ability.controller {
        return Err(CombatSpecialRuntimeError::InvalidCombatTopology);
    }

    let source_available = input.source_current.as_ref().is_some_and(|source| {
        source.object == ability.source_in_hand
            && source.owner == ability.source_owner
            && source.zone == Zone::Hand
            && source.zone_player == Some(ability.controller)
            && source.characteristics_complete
    });
    if !source_available {
        if input.entry.is_some() {
            return Err(CombatSpecialRuntimeError::UnexpectedEntryEvidence);
        }
        return Ok(NinjutsuResolutionReceipt {
            activation_id: ability.activation_id,
            stack_id: ability.stack_id,
            source_before: ability.source_in_hand,
            battlefield_object: None,
            entered_battlefield: false,
            entered_tapped: false,
            entered_attacking: false,
            entered_unblocked: false,
            attack_target: None,
            attack_declaration_triggers_fire: false,
            source_reveal_ended: true,
            semantic_digest: ability.semantic_digest,
        });
    }

    let entry = input
        .entry
        .ok_or(CombatSpecialRuntimeError::MissingEntryEvidence)?;
    if entry.source_before != ability.source_in_hand
        || entry.owner != ability.source_owner
        || !entry.characteristics_complete
        || !entry.replacement_effects_complete
    {
        return Err(CombatSpecialRuntimeError::InvalidNinjutsuEntry);
    }
    let entered = entry.destination == Zone::Battlefield;
    let battlefield_object = if entered {
        let object = entry
            .object_after
            .ok_or(CombatSpecialRuntimeError::InvalidNinjutsuEntry)?;
        if object.object_id != ability.source_in_hand.object_id
            || object.incarnation_id == ability.source_in_hand.incarnation_id
            || entry.controller_after != Some(ability.controller)
            || !entry.entered_tapped
        {
            return Err(CombatSpecialRuntimeError::InvalidNinjutsuEntry);
        }
        Some(object)
    } else {
        let object = entry
            .object_after
            .ok_or(CombatSpecialRuntimeError::InvalidNinjutsuEntry)?;
        if object.object_id != ability.source_in_hand.object_id
            || object.incarnation_id == ability.source_in_hand.incarnation_id
            || entry.entered_tapped
            || entry.controller_after.is_some()
        {
            return Err(CombatSpecialRuntimeError::InvalidNinjutsuEntry);
        }
        None
    };
    let target_valid = input
        .topology
        .target_is_currently_attackable(ability.captured_attack_target)?;
    let enters_attacking =
        entered && entry.is_creature_after_entry && !entry.is_battle_after_entry && target_valid;
    Ok(NinjutsuResolutionReceipt {
        activation_id: ability.activation_id,
        stack_id: ability.stack_id,
        source_before: ability.source_in_hand,
        battlefield_object,
        entered_battlefield: entered,
        entered_tapped: entered && entry.entered_tapped,
        entered_attacking: enters_attacking,
        entered_unblocked: enters_attacking,
        attack_target: enters_attacking.then_some(ability.captured_attack_target),
        attack_declaration_triggers_fire: false,
        source_reveal_ended: true,
        semantic_digest: ability.semantic_digest,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NinjutsuStackExitReceipt {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub source: ObjectRef,
    pub resolved: bool,
    pub source_reveal_ended: bool,
    pub semantic_digest: String,
}

pub fn remove_ninjutsu_from_stack_without_resolution(
    ability: NinjutsuStackAbility,
) -> NinjutsuStackExitReceipt {
    NinjutsuStackExitReceipt {
        activation_id: ability.activation_id,
        stack_id: ability.stack_id,
        source: ability.source_in_hand,
        resolved: false,
        source_reveal_ended: true,
        semantic_digest: ability.semantic_digest,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiableCharacteristics {
    pub name: String,
    pub mana_cost: Option<ManaCost>,
    pub colors: BTreeSet<ManaColor>,
    pub supertypes: BTreeSet<String>,
    pub card_types: BTreeSet<String>,
    pub subtypes: BTreeSet<String>,
    pub oracle_text: String,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub loyalty: Option<i32>,
    pub defense: Option<i32>,
    pub copy_layers_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreActivationInput {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub actor: PlayerId,
    pub source: ZoneObjectEvidence,
    pub source_copiable_characteristics: CopiableCharacteristics,
    pub timing: SorceryTimingEvidence,
    pub all_activation_choices_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEncoreActivation {
    program: CombatSpecialKeywordProgram,
    input: EncoreActivationInput,
}

pub fn begin_encore_activation(
    program: &CombatSpecialKeywordProgram,
    input: EncoreActivationInput,
) -> Result<PendingEncoreActivation, CombatSpecialRuntimeError> {
    if !matches!(program.kind(), CombatSpecialKeywordKind::Encore { .. }) {
        return Err(CombatSpecialRuntimeError::WrongProgramKind);
    }
    validate_sorcery_timing(&input.timing, input.actor)?;
    if !input.all_activation_choices_complete
        || input.source.zone != Zone::Graveyard
        || input.source.zone_player != Some(input.actor)
        || input.source.owner != input.actor
        || !input.source.characteristics_complete
        || !input.source_copiable_characteristics.copy_layers_complete
    {
        return Err(CombatSpecialRuntimeError::InvalidEncoreSource);
    }
    Ok(PendingEncoreActivation {
        program: program.clone(),
        input,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreStackAbility {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub controller: PlayerId,
    pub source_before_exile: ObjectRef,
    pub source_in_exile: ObjectRef,
    pub source_owner: PlayerId,
    /// Public-zone copiable values at activation. Resolution supplies either
    /// current exile information or complete last-known exile information.
    pub activation_copiable_characteristics: CopiableCharacteristics,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreActivationReceipt {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub source_before_exile: ObjectRef,
    pub source_in_exile: ObjectRef,
    pub semantic_digest: String,
}

impl PendingEncoreActivation {
    pub fn commit(
        self,
        payment: ManaPaymentEvidence,
        exile_source: ZoneMoveEvidence,
    ) -> Result<(EncoreStackAbility, EncoreActivationReceipt), CombatSpecialRuntimeError> {
        let CombatSpecialKeywordKind::Encore { cost } = self.program.kind() else {
            return Err(CombatSpecialRuntimeError::WrongProgramKind);
        };
        validate_mana_payment(&payment, self.input.actor, cost)?;
        if exile_source.owner != self.input.source.owner {
            return Err(CombatSpecialRuntimeError::InvalidEncoreSource);
        }
        let source_in_exile = validate_paid_zone_move(
            &exile_source,
            self.input.source.object,
            Zone::Graveyard,
            Zone::Exile,
        )?;
        let ability = EncoreStackAbility {
            activation_id: self.input.activation_id,
            stack_id: self.input.stack_id,
            controller: self.input.actor,
            source_before_exile: self.input.source.object,
            source_in_exile,
            source_owner: self.input.source.owner,
            activation_copiable_characteristics: self.input.source_copiable_characteristics,
            semantic_digest: self.program.semantic_digest.clone(),
        };
        let receipt = EncoreActivationReceipt {
            activation_id: ability.activation_id,
            stack_id: ability.stack_id,
            source_before_exile: ability.source_before_exile,
            source_in_exile,
            semantic_digest: self.program.semantic_digest,
        };
        Ok((ability, receipt))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoreCopyInformationBasis {
    CurrentTrackedExileObject,
    LastKnownTrackedExileObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreCopySourceEvidence {
    pub tracked_exile_object: ObjectRef,
    pub current_object: Option<ObjectRef>,
    pub current_zone: Option<Zone>,
    pub basis: EncoreCopyInformationBasis,
    pub characteristics: CopiableCharacteristics,
    pub tracking_and_information_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreCreatedTokenEvidence {
    pub token: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub resulting_zone: Zone,
    pub resulting_copiable_characteristics: CopiableCharacteristics,
    pub controller_assignment_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreTokenCreationEvidence {
    pub designated_opponent: PlayerId,
    pub requested_copy_count: u32,
    pub requested_copy_characteristics: CopiableCharacteristics,
    /// Replacement effects may prevent or multiply the requested token.
    pub created_tokens: Vec<EncoreCreatedTokenEvidence>,
    pub creator_after_replacements: PlayerId,
    pub replacement_effects_complete: bool,
    pub token_creation_event_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreResolutionInput {
    pub resolution_event_id: EventId,
    pub turn_id: TurnId,
    pub stack_object_is_resolving: bool,
    pub topology: CombatTopologyEvidence,
    pub current_opponents: BTreeSet<PlayerId>,
    pub opponent_census_complete: bool,
    pub copy_source: EncoreCopySourceEvidence,
    pub token_creations: Vec<EncoreTokenCreationEvidence>,
    pub next_end_step_id: EndStepId,
    pub delayed_trigger_schedule_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreAttackRequirement {
    pub token: ObjectRef,
    pub designated_opponent: PlayerId,
    pub encore_controller: PlayerId,
    pub turn_id: TurnId,
    pub if_able: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreToken {
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub copied_characteristics: CopiableCharacteristics,
    pub has_haste: bool,
    pub attack_requirement: EncoreAttackRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEncoreEndStepSacrifice {
    pub trigger_id: TriggerId,
    pub controller: PlayerId,
    pub created_by_resolution_event_id: EventId,
    pub next_end_step_id: EndStepId,
    pub tracked_tokens: BTreeSet<ObjectRef>,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreResolutionReceipt {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub resolution_event_id: EventId,
    pub opponents_at_resolution: BTreeSet<PlayerId>,
    pub tokens: Vec<EncoreToken>,
    pub delayed_sacrifice: PendingEncoreEndStepSacrifice,
    pub semantic_digest: String,
}

pub fn resolve_encore(
    ability: EncoreStackAbility,
    input: EncoreResolutionInput,
) -> Result<EncoreResolutionReceipt, CombatSpecialRuntimeError> {
    if !input.stack_object_is_resolving
        || !input.opponent_census_complete
        || !input.delayed_trigger_schedule_complete
    {
        return Err(CombatSpecialRuntimeError::IncompleteEncoreResolutionEvidence);
    }
    input.topology.validate()?;
    if input.topology.attacker_controller != ability.controller
        || input.current_opponents != input.topology.opponents
        || input.current_opponents.contains(&ability.controller)
    {
        return Err(CombatSpecialRuntimeError::InvalidCombatTopology);
    }
    let copy_source = input.copy_source;
    if !copy_source.tracking_and_information_complete
        || copy_source.tracked_exile_object != ability.source_in_exile
        || !copy_source.characteristics.copy_layers_complete
    {
        return Err(CombatSpecialRuntimeError::IncompleteEncoreCopyInformation);
    }
    match copy_source.basis {
        EncoreCopyInformationBasis::CurrentTrackedExileObject => {
            if copy_source.current_object != Some(ability.source_in_exile)
                || copy_source.current_zone != Some(Zone::Exile)
            {
                return Err(CombatSpecialRuntimeError::IncompleteEncoreCopyInformation);
            }
        }
        EncoreCopyInformationBasis::LastKnownTrackedExileObject => {
            if copy_source.current_object == Some(ability.source_in_exile)
                && copy_source.current_zone == Some(Zone::Exile)
            {
                return Err(CombatSpecialRuntimeError::IncompleteEncoreCopyInformation);
            }
        }
    }
    let mut creations = BTreeMap::new();
    for creation in input.token_creations {
        let opponent = creation.designated_opponent;
        if creations.insert(opponent, creation).is_some() {
            return Err(CombatSpecialRuntimeError::DuplicateOpponentTokenEvidence);
        }
    }
    if creations.keys().copied().collect::<BTreeSet<_>>() != input.current_opponents {
        return Err(CombatSpecialRuntimeError::IncompleteOpponentTokenEvidence);
    }

    let mut tokens = Vec::new();
    let mut token_ids = BTreeSet::new();
    for opponent in &input.current_opponents {
        let creation = creations
            .remove(opponent)
            .ok_or(CombatSpecialRuntimeError::IncompleteOpponentTokenEvidence)?;
        if !creation.replacement_effects_complete
            || !creation.token_creation_event_complete
            || creation.requested_copy_count != 1
            || creation.requested_copy_characteristics != copy_source.characteristics
            || !input
                .topology
                .players_in_game
                .contains(&creation.creator_after_replacements)
        {
            return Err(CombatSpecialRuntimeError::InvalidEncoreTokenEvidence);
        }
        for created in creation.created_tokens {
            if !token_ids.insert(created.token) {
                return Err(CombatSpecialRuntimeError::DuplicateTokenObject);
            }
            if created.resulting_zone != Zone::Battlefield
                || created.owner != creation.creator_after_replacements
                || !created.controller_assignment_complete
                || !created
                    .resulting_copiable_characteristics
                    .copy_layers_complete
                || !input.topology.players_in_game.contains(&created.controller)
            {
                return Err(CombatSpecialRuntimeError::InvalidEncoreTokenEvidence);
            }
            tokens.push(EncoreToken {
                object: created.token,
                owner: created.owner,
                controller: created.controller,
                copied_characteristics: created.resulting_copiable_characteristics,
                has_haste: true,
                attack_requirement: EncoreAttackRequirement {
                    token: created.token,
                    designated_opponent: *opponent,
                    encore_controller: ability.controller,
                    turn_id: input.turn_id,
                    if_able: true,
                },
            });
        }
    }
    tokens.sort_by_key(|token| token.attack_requirement.designated_opponent);
    let tracked_tokens = tokens.iter().map(|token| token.object).collect();
    let delayed_sacrifice = PendingEncoreEndStepSacrifice {
        trigger_id: input
            .resolution_event_id
            .wrapping_mul(1_000_003)
            .wrapping_add(141),
        controller: ability.controller,
        created_by_resolution_event_id: input.resolution_event_id,
        next_end_step_id: input.next_end_step_id,
        tracked_tokens,
        semantic_digest: ability.semantic_digest.clone(),
    };
    Ok(EncoreResolutionReceipt {
        activation_id: ability.activation_id,
        stack_id: ability.stack_id,
        resolution_event_id: input.resolution_event_id,
        opponents_at_resolution: input.current_opponents,
        tokens,
        delayed_sacrifice,
        semantic_digest: ability.semantic_digest,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreTokenAttackDecision {
    pub token: ObjectRef,
    pub chosen_target: Option<AttackTarget>,
    pub legal_targets_without_optional_attack_cost: BTreeSet<AttackTarget>,
    pub restrictions_complete: bool,
    pub attack_costs_complete: bool,
    pub all_attack_requirements_complete: bool,
    pub chosen_declaration_maximizes_satisfied_requirements: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreAttackDeclarationInput {
    pub turn_id: TurnId,
    pub combat_id: CombatId,
    pub active_player: PlayerId,
    pub topology: CombatTopologyEvidence,
    pub decisions: Vec<EncoreTokenAttackDecision>,
    pub global_declaration_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreAttackDeclarationReceipt {
    pub combat_id: CombatId,
    pub declared_targets: BTreeMap<ObjectRef, Option<AttackTarget>>,
    pub requirements_satisfied: BTreeMap<ObjectRef, bool>,
}

pub fn validate_encore_attack_declaration(
    tokens: &[EncoreToken],
    input: EncoreAttackDeclarationInput,
) -> Result<EncoreAttackDeclarationReceipt, CombatSpecialRuntimeError> {
    if !input.global_declaration_complete {
        return Err(CombatSpecialRuntimeError::IncompleteAttackDeclaration);
    }
    input.topology.validate()?;
    if input.topology.attacker_controller != input.active_player {
        return Err(CombatSpecialRuntimeError::InvalidCombatTopology);
    }
    let mut decisions = BTreeMap::new();
    for decision in input.decisions {
        if decisions.insert(decision.token, decision).is_some() {
            return Err(CombatSpecialRuntimeError::DuplicateAttackDecision);
        }
    }
    let expected = tokens
        .iter()
        .map(|token| token.object)
        .collect::<BTreeSet<_>>();
    if decisions.keys().copied().collect::<BTreeSet<_>>() != expected {
        return Err(CombatSpecialRuntimeError::IncompleteAttackDeclaration);
    }

    let mut declared_targets = BTreeMap::new();
    let mut requirements_satisfied = BTreeMap::new();
    for token in tokens {
        if token.attack_requirement.encore_controller != input.active_player
            || token.attack_requirement.turn_id != input.turn_id
        {
            return Err(CombatSpecialRuntimeError::WrongEncoreAttackTurn);
        }
        let decision = decisions
            .remove(&token.object)
            .ok_or(CombatSpecialRuntimeError::IncompleteAttackDeclaration)?;
        if !decision.restrictions_complete
            || !decision.attack_costs_complete
            || !decision.all_attack_requirements_complete
            || !decision.chosen_declaration_maximizes_satisfied_requirements
        {
            return Err(CombatSpecialRuntimeError::IncompleteAttackDeclaration);
        }
        if token.controller != input.active_player
            && (decision.chosen_target.is_some()
                || !decision
                    .legal_targets_without_optional_attack_cost
                    .is_empty())
        {
            return Err(CombatSpecialRuntimeError::IllegalAttackTarget);
        }
        for target in &decision.legal_targets_without_optional_attack_cost {
            if !input.topology.target_is_currently_attackable(*target)? {
                return Err(CombatSpecialRuntimeError::IllegalAttackTarget);
            }
        }
        if decision.chosen_target.is_some_and(|target| {
            !decision
                .legal_targets_without_optional_attack_cost
                .contains(&target)
        }) {
            return Err(CombatSpecialRuntimeError::IllegalAttackTarget);
        }
        let satisfied = decision.chosen_target
            == Some(AttackTarget::Player(
                token.attack_requirement.designated_opponent,
            ));
        declared_targets.insert(token.object, decision.chosen_target);
        requirements_satisfied.insert(token.object, satisfied);
    }
    Ok(EncoreAttackDeclarationReceipt {
        combat_id: input.combat_id,
        declared_targets,
        requirements_satisfied,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreTrackedTokenState {
    pub tracked_object: ObjectRef,
    pub current_object: Option<ObjectRef>,
    pub zone: Option<Zone>,
    pub current_controller: Option<PlayerId>,
    pub state_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreEndStepSacrificeInput {
    pub end_step_id: EndStepId,
    pub is_beginning_of_end_step: bool,
    pub trigger_is_resolving: bool,
    pub tracked_states: Vec<EncoreTrackedTokenState>,
    pub sacrifice_moves: Vec<ZoneMoveEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoreEndStepSacrificeReceipt {
    pub trigger_id: TriggerId,
    pub end_step_id: EndStepId,
    pub sacrificed_tokens: BTreeSet<ObjectRef>,
    pub no_longer_same_battlefield_object: BTreeSet<ObjectRef>,
    pub not_controlled_by_trigger_controller: BTreeSet<ObjectRef>,
    pub tokens_cease_to_exist_after_zone_change: bool,
}

pub fn resolve_encore_end_step_sacrifice(
    trigger: PendingEncoreEndStepSacrifice,
    input: EncoreEndStepSacrificeInput,
) -> Result<EncoreEndStepSacrificeReceipt, CombatSpecialRuntimeError> {
    if input.end_step_id != trigger.next_end_step_id
        || !input.is_beginning_of_end_step
        || !input.trigger_is_resolving
    {
        return Err(CombatSpecialRuntimeError::WrongEndStep);
    }
    let mut states = BTreeMap::new();
    for state in input.tracked_states {
        if !state.state_evidence_complete || states.insert(state.tracked_object, state).is_some() {
            return Err(CombatSpecialRuntimeError::IncompleteTrackedTokenState);
        }
    }
    if states.keys().copied().collect::<BTreeSet<_>>() != trigger.tracked_tokens {
        return Err(CombatSpecialRuntimeError::IncompleteTrackedTokenState);
    }
    let mut moves = BTreeMap::new();
    for movement in input.sacrifice_moves {
        if moves.insert(movement.object_before, movement).is_some() {
            return Err(CombatSpecialRuntimeError::DuplicateSacrificeMove);
        }
    }

    let mut sacrificed_tokens = BTreeSet::new();
    let mut no_longer_same_battlefield_object = BTreeSet::new();
    let mut not_controlled_by_trigger_controller = BTreeSet::new();
    for tracked in &trigger.tracked_tokens {
        let state = states
            .remove(tracked)
            .ok_or(CombatSpecialRuntimeError::IncompleteTrackedTokenState)?;
        if state.current_object == Some(*tracked)
            && state.zone == Some(Zone::Battlefield)
            && state.current_controller == Some(trigger.controller)
        {
            let movement = moves
                .remove(tracked)
                .ok_or(CombatSpecialRuntimeError::MissingSacrificeMove)?;
            if movement.object_before != *tracked
                || movement.from != Zone::Battlefield
                || movement.to == Zone::Battlefield
                || !movement.move_succeeded
                || !movement.replacement_effects_complete
                || movement.object_after.is_none()
            {
                return Err(CombatSpecialRuntimeError::InvalidSacrificeMove);
            }
            sacrificed_tokens.insert(*tracked);
        } else if state.current_object == Some(*tracked)
            && state.zone == Some(Zone::Battlefield)
            && state.current_controller.is_some()
        {
            if moves.contains_key(tracked) {
                return Err(CombatSpecialRuntimeError::UnexpectedSacrificeMove);
            }
            not_controlled_by_trigger_controller.insert(*tracked);
        } else {
            if moves.contains_key(tracked) {
                return Err(CombatSpecialRuntimeError::UnexpectedSacrificeMove);
            }
            no_longer_same_battlefield_object.insert(*tracked);
        }
    }
    if !moves.is_empty() {
        return Err(CombatSpecialRuntimeError::UnexpectedSacrificeMove);
    }
    Ok(EncoreEndStepSacrificeReceipt {
        trigger_id: trigger.trigger_id,
        end_step_id: input.end_step_id,
        sacrificed_tokens,
        no_longer_same_battlefield_object,
        not_controlled_by_trigger_controller,
        tokens_cease_to_exist_after_zone_change: true,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddleSourceEvidence {
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub is_permanent: bool,
    pub characteristics_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddleCostCreatureEvidence {
    pub object: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub is_creature: bool,
    pub untapped_before_payment: bool,
    pub current_power: i32,
    pub continuous_effects_complete: bool,
    pub power_evidence_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddleActivationInput {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub binding_id: BindingId,
    pub actor: PlayerId,
    pub turn_id: TurnId,
    pub source: SaddleSourceEvidence,
    pub timing: SorceryTimingEvidence,
    pub selected_creatures: Vec<SaddleCostCreatureEvidence>,
    pub activation_authority_complete: bool,
    pub actor_is_authorized_to_activate_source: bool,
    pub applicable_saddle_instances_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddleStackAbility {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub binding_id: BindingId,
    pub controller: PlayerId,
    pub turn_id: TurnId,
    pub source: ObjectRef,
    pub tapped_to_saddle: BTreeSet<ObjectRef>,
    pub paid_total_power: i64,
    pub threshold: u32,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddleActivationReceipt {
    pub activation_id: ActivationId,
    pub stack_id: StackObjectId,
    pub source: ObjectRef,
    pub binding_id: BindingId,
    pub creatures_tapped_simultaneously: BTreeSet<ObjectRef>,
    pub paid_total_power: i64,
    pub summoning_sickness_was_not_a_cost_restriction: bool,
    pub semantic_digest: String,
}

pub fn activate_saddle(
    program: &CombatSpecialKeywordProgram,
    input: SaddleActivationInput,
) -> Result<(SaddleStackAbility, SaddleActivationReceipt), CombatSpecialRuntimeError> {
    let CombatSpecialKeywordKind::Saddle { threshold } = program.kind() else {
        return Err(CombatSpecialRuntimeError::WrongProgramKind);
    };
    validate_sorcery_timing(&input.timing, input.actor)?;
    if !input.applicable_saddle_instances_complete
        || !input.activation_authority_complete
        || !input.actor_is_authorized_to_activate_source
        || input.source.zone != Zone::Battlefield
        || !input.source.is_permanent
        || !input.source.characteristics_complete
    {
        return Err(CombatSpecialRuntimeError::InvalidSaddleSource);
    }
    let mut tapped = BTreeSet::new();
    let mut total_power = 0i64;
    for creature in &input.selected_creatures {
        if creature.object == input.source.object
            || creature.controller != input.actor
            || creature.zone != Zone::Battlefield
            || !creature.is_creature
            || !creature.untapped_before_payment
            || !creature.continuous_effects_complete
            || !creature.power_evidence_complete
            || !tapped.insert(creature.object)
        {
            return Err(CombatSpecialRuntimeError::InvalidSaddleCostCreature);
        }
        total_power = total_power
            .checked_add(i64::from(creature.current_power))
            .ok_or(CombatSpecialRuntimeError::PowerTotalOverflow)?;
    }
    if tapped.is_empty() || total_power < i64::from(*threshold) {
        return Err(CombatSpecialRuntimeError::InsufficientSaddlePower {
            required: *threshold,
            actual: total_power,
        });
    }
    let ability = SaddleStackAbility {
        activation_id: input.activation_id,
        stack_id: input.stack_id,
        binding_id: input.binding_id,
        controller: input.actor,
        turn_id: input.turn_id,
        source: input.source.object,
        tapped_to_saddle: tapped.clone(),
        paid_total_power: total_power,
        threshold: *threshold,
        semantic_digest: program.semantic_digest.clone(),
    };
    let receipt = SaddleActivationReceipt {
        activation_id: input.activation_id,
        stack_id: input.stack_id,
        source: input.source.object,
        binding_id: input.binding_id,
        creatures_tapped_simultaneously: tapped,
        paid_total_power: total_power,
        summoning_sickness_was_not_a_cost_restriction: true,
        semantic_digest: program.semantic_digest.clone(),
    };
    Ok((ability, receipt))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddledDesignation {
    pub source: ObjectRef,
    pub turn_id: TurnId,
    pub contributing_resolutions: BTreeSet<ActivationId>,
    pub is_copiable_value: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddleResolutionReceipt {
    pub activation_id: ActivationId,
    pub source: ObjectRef,
    pub became_saddled: bool,
    pub designation: Option<SaddledDesignation>,
    pub semantic_digest: String,
}

pub fn resolve_saddle(
    ability: SaddleStackAbility,
    source_current: Option<SaddleSourceEvidence>,
    existing: Option<SaddledDesignation>,
    resolution_turn_id: TurnId,
) -> Result<SaddleResolutionReceipt, CombatSpecialRuntimeError> {
    if resolution_turn_id != ability.turn_id {
        return Err(CombatSpecialRuntimeError::WrongSaddleTurn);
    }
    let source_exists = source_current.as_ref().is_some_and(|source| {
        source.object == ability.source
            && source.zone == Zone::Battlefield
            && source.is_permanent
            && source.characteristics_complete
    });
    if !source_exists {
        if existing.is_some() {
            return Err(CombatSpecialRuntimeError::StaleSaddledDesignation);
        }
        return Ok(SaddleResolutionReceipt {
            activation_id: ability.activation_id,
            source: ability.source,
            became_saddled: false,
            designation: None,
            semantic_digest: ability.semantic_digest,
        });
    }
    let mut designation = match existing {
        Some(existing)
            if existing.source == ability.source
                && existing.turn_id == ability.turn_id
                && !existing.is_copiable_value =>
        {
            existing
        }
        Some(_) => return Err(CombatSpecialRuntimeError::StaleSaddledDesignation),
        None => SaddledDesignation {
            source: ability.source,
            turn_id: ability.turn_id,
            contributing_resolutions: BTreeSet::new(),
            is_copiable_value: false,
        },
    };
    designation
        .contributing_resolutions
        .insert(ability.activation_id);
    Ok(SaddleResolutionReceipt {
        activation_id: ability.activation_id,
        source: ability.source,
        became_saddled: true,
        designation: Some(designation),
        semantic_digest: ability.semantic_digest,
    })
}

pub fn saddled_designation_is_active(
    designation: &SaddledDesignation,
    current_object: Option<ObjectRef>,
    current_zone: Option<Zone>,
    current_turn_id: TurnId,
) -> bool {
    current_turn_id == designation.turn_id
        && current_object == Some(designation.source)
        && current_zone == Some(Zone::Battlefield)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaddledTriggerLimit {
    OncePerCombat,
    EveryQualifyingAttack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddledAttackTriggerBinding {
    pub binding_id: BindingId,
    pub source: ObjectRef,
    pub effect_semantic_digest: String,
    pub limit: SaddledTriggerLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SaddledTriggerHistory {
    fired_once_per_combat: BTreeSet<(BindingId, CombatId)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaddledAttackEvent {
    pub event_id: EventId,
    pub combat_id: CombatId,
    pub turn_id: TurnId,
    pub attacker: ObjectRef,
    pub was_declared_as_attacker: bool,
    pub attack_event_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSaddledAttackTrigger {
    pub trigger_id: TriggerId,
    pub binding_id: BindingId,
    pub source_at_trigger: ObjectRef,
    pub combat_id: CombatId,
    pub effect_semantic_digest: String,
}

pub fn observe_saddled_attack_event(
    designation: &SaddledDesignation,
    event: SaddledAttackEvent,
    bindings: &[SaddledAttackTriggerBinding],
    applicable_bindings_complete: bool,
    history: &mut SaddledTriggerHistory,
) -> Result<Vec<PendingSaddledAttackTrigger>, CombatSpecialRuntimeError> {
    if !applicable_bindings_complete
        || !event.attack_event_complete
        || !event.was_declared_as_attacker
        || event.attacker != designation.source
        || event.turn_id != designation.turn_id
        || designation.is_copiable_value
    {
        return Err(CombatSpecialRuntimeError::InvalidSaddledAttackEvent);
    }
    let mut seen = BTreeSet::new();
    let mut pending = Vec::new();
    for binding in bindings {
        if binding.source != designation.source || !seen.insert(binding.binding_id) {
            return Err(CombatSpecialRuntimeError::InvalidSaddledTriggerBinding);
        }
        let may_fire = match binding.limit {
            SaddledTriggerLimit::OncePerCombat => history
                .fired_once_per_combat
                .insert((binding.binding_id, event.combat_id)),
            SaddledTriggerLimit::EveryQualifyingAttack => true,
        };
        if may_fire {
            pending.push(PendingSaddledAttackTrigger {
                trigger_id: event
                    .event_id
                    .wrapping_mul(1_000_003)
                    .wrapping_add(binding.binding_id),
                binding_id: binding.binding_id,
                source_at_trigger: binding.source,
                combat_id: event.combat_id,
                effect_semantic_digest: binding.effect_semantic_digest.clone(),
            });
        }
    }
    pending.sort_by_key(|trigger| trigger.binding_id);
    Ok(pending)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatSpecialRuntimeError {
    WrongProgramKind,
    IncompleteCombatTopology,
    InvalidCombatTopology,
    WrongPayer {
        expected: PlayerId,
        actual: PlayerId,
    },
    PrintedManaCostMismatch,
    IncompleteManaCostEvidence,
    InvalidManaPayment,
    ManaQuantityOverflow,
    SorceryTimingUnavailable,
    InvalidCostZoneMove,
    IncompleteActivationEvidence,
    InvalidNinjutsuSource,
    NoEligibleUnblockedAttacker,
    InvalidReturnedAttackerOwner,
    InvalidStackState,
    UnexpectedEntryEvidence,
    MissingEntryEvidence,
    InvalidNinjutsuEntry,
    InvalidEncoreSource,
    IncompleteEncoreResolutionEvidence,
    IncompleteEncoreCopyInformation,
    DuplicateOpponentTokenEvidence,
    IncompleteOpponentTokenEvidence,
    InvalidEncoreTokenEvidence,
    DuplicateTokenObject,
    IncompleteAttackDeclaration,
    DuplicateAttackDecision,
    WrongEncoreAttackTurn,
    IllegalAttackTarget,
    WrongEndStep,
    IncompleteTrackedTokenState,
    DuplicateSacrificeMove,
    MissingSacrificeMove,
    InvalidSacrificeMove,
    UnexpectedSacrificeMove,
    InvalidSaddleSource,
    InvalidSaddleCostCreature,
    InsufficientSaddlePower {
        required: u32,
        actual: i64,
    },
    PowerTotalOverflow,
    WrongSaddleTurn,
    StaleSaddledDesignation,
    InvalidSaddledAttackEvent,
    InvalidSaddledTriggerBinding,
}

impl fmt::Display for CombatSpecialRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CombatSpecialRuntimeError {}
