use std::collections::{HashMap, HashSet};

use crate::ability_ir::{AbilityProfile, GraphCard, build_synergy_graph, compile_ability_profile};
use crate::ability_program::{
    AbilityCost, AbilityEffect, AbilityTiming, ActivationWindow, CardType as ProgramCardType,
    ControllerRelation, DelayedEvent, DelayedObjectReference, DiscardedObjectReference,
    ExecutableAbilityProgramV1, FaceCastCharacteristicsInput, LibraryPosition,
    ManaCost as ProgramManaCost, ManaKind as ProgramManaKind, NecropotenceDiscardEvent,
    OracleCardFaceInput, OracleCardInput, StepProcedure, TargetSelector, TokenKind,
    TriggerEventKind, TurnStep, VariableCreatureOverrunEffect, VariableCreatureTutorEffect, Zone,
    compile_executable_ability_program, compile_face_bound_ability_program_with_characteristics,
};
use crate::comprehensive_rules::ComprehensiveRulesSnapshot;
use crate::domain::{
    CardDefinition, DeckEntry, KnownLine, KnownLineOutcome, LineRequirement, RoleCount,
    StrategyPlan, SynergyGraph, SynergyReport,
};
use crate::effects::{EffectDescriptor, compile_effect_descriptor};
use crate::parser::normalize_card_name;
use crate::rules_capabilities::apply_rules_capabilities;
use crate::semantic_store::{OverrideMatch, SemanticOverridePackage, SemanticRole};

pub(crate) const ANNOTATION_MODEL_VERSION: &str = "oracle-annotations-0.9";
pub(crate) const COMBO_CATALOG_VERSION: &str = "known-lines-0.7";

pub mod role {
    pub const LAND: u32 = 1 << 0;
    pub const MANA_SOURCE: u32 = 1 << 1;
    pub const RAMP: u32 = 1 << 2;
    pub const FAST_MANA: u32 = 1 << 3;
    pub const DRAW: u32 = 1 << 4;
    pub const TUTOR: u32 = 1 << 5;
    pub const REMOVAL: u32 = 1 << 6;
    pub const COUNTERSPELL: u32 = 1 << 7;
    pub const BOARD_WIPE: u32 = 1 << 8;
    pub const PROTECTION: u32 = 1 << 9;
    pub const ENGINE: u32 = 1 << 10;
    pub const ENABLER: u32 = 1 << 11;
    pub const PAYOFF: u32 = 1 << 12;
    pub const WIN_CONDITION: u32 = 1 << 13;
    pub const COMBO_PIECE: u32 = 1 << 14;
    pub const GRAVEYARD: u32 = 1 << 15;
    pub const TOKEN: u32 = 1 << 16;
    pub const SACRIFICE: u32 = 1 << 17;
    pub const STAX: u32 = 1 << 18;
    pub const CREATURE: u32 = 1 << 19;
    pub const ARTIFACT: u32 = 1 << 20;
    pub const ENCHANTMENT: u32 = 1 << 21;
    pub const INSTANT_SORCERY: u32 = 1 << 22;
    pub const RECURSION: u32 = 1 << 23;
    /// The card explicitly rewards, copies, discounts, or otherwise refers to
    /// casting instants/sorceries (or the wider noncreature-spell family).
    pub const SPELL_MATTERS: u32 = 1 << 24;
    /// The card explicitly treats artifacts as a strategic resource. Merely
    /// being an artifact is intentionally insufficient.
    pub const ARTIFACT_MATTERS: u32 = 1 << 25;
    /// The card explicitly treats enchantments as a strategic resource.
    pub const ENCHANTMENT_MATTERS: u32 = 1 << 26;
    /// The card rewards, replaces, counts, or otherwise refers to tokens as a
    /// strategic resource, beyond simply creating one token.
    pub const TOKEN_MATTERS: u32 = 1 << 27;
    /// The card explicitly reacts to creatures/permanents dying.
    pub const DEATH_MATTERS: u32 = 1 << 28;
    /// The card explicitly rewards a wide creature board or creature spells.
    pub const CREATURE_MATTERS: u32 = 1 << 29;
}

#[derive(Debug, Clone)]
pub struct CompiledCard {
    pub name: String,
    pub normalized_name: String,
    pub type_line: String,
    /// Exact Scryfall color characteristic for the resolved face/root.
    ///
    /// This is deliberately distinct from color identity and from mana-cost
    /// pips. Dynamic mana abilities such as “among legendary creatures and
    /// planeswalkers you control” need the controlled object's actual colors,
    /// including color indicators and effects represented by the card-data
    /// layer, rather than an approximation from its casting cost.
    pub colors: Vec<String>,
    pub quantity: u16,
    pub mana_value: f32,
    /// Exact integer printed power of the primary battlefield face when the
    /// local card record supplies one. Dynamic `*` expressions remain absent.
    pub printed_power: Option<i16>,
    /// Exact integer printed toughness of the primary battlefield face when
    /// the local card record supplies one. Dynamic `*` expressions remain
    /// absent instead of being guessed.
    pub printed_toughness: Option<i16>,
    pub roles: u32,
    pub is_commander: bool,
    pub semantic_confidence: f32,
    pub effects: EffectDescriptor,
    // This integration is intentionally staged ahead of simulator execution.
    #[allow(dead_code)]
    pub(crate) ability_program: ExecutableAbilityProgramV1,
}

impl CompiledCard {
    pub fn has(&self, role: u32) -> bool {
        self.roles & role != 0
    }

    /// Exact physical-card characteristics in hand, including front-face
    /// binding for double-faced cards.
    pub fn hand_zone_characteristics(&self) -> &crate::effects::HandZoneCharacteristics {
        &self.effects.hand_zone
    }
}

#[derive(Debug, Clone)]
pub struct CompiledDeck {
    pub cards: Vec<CompiledCard>,
    pub library: Vec<usize>,
    pub commanders: Vec<usize>,
    pub known_lines: Vec<KnownLine>,
    pub synergy: SynergyReport,
    pub semantic_coverage: f32,
    pub approximated_cards: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SemanticOverrideApplicationSummary {
    pub applied_cards: Vec<String>,
    pub unguarded_applied_cards: Vec<String>,
    pub oracle_text_guard_mismatches: Vec<String>,
    pub rules_backed_mechanics: Vec<String>,
    pub rules_report_only_cards: Vec<String>,
}
#[allow(dead_code)]
pub fn compile_deck_with_semantic_overrides(
    entries: &[DeckEntry],
    definitions: &HashMap<String, CardDefinition>,
    selected_commanders: &[String],
    additional_known_lines: &[KnownLine],
    semantic_overrides: &SemanticOverridePackage,
) -> (CompiledDeck, SemanticOverrideApplicationSummary) {
    compile_deck_internal(
        entries,
        definitions,
        selected_commanders,
        additional_known_lines,
        Some(semantic_overrides),
        None,
    )
}

pub fn compile_deck_with_rules_and_semantic_overrides(
    entries: &[DeckEntry],
    definitions: &HashMap<String, CardDefinition>,
    selected_commanders: &[String],
    additional_known_lines: &[KnownLine],
    semantic_overrides: &SemanticOverridePackage,
    comprehensive_rules: Option<&ComprehensiveRulesSnapshot>,
) -> (CompiledDeck, SemanticOverrideApplicationSummary) {
    compile_deck_internal(
        entries,
        definitions,
        selected_commanders,
        additional_known_lines,
        Some(semantic_overrides),
        comprehensive_rules,
    )
}

fn canonical_compilation_entries<'a>(
    entries: &'a [DeckEntry],
    selected_commanders: &HashSet<String>,
) -> Vec<&'a DeckEntry> {
    let mut commanders = Vec::new();
    let mut noncommanders = Vec::new();

    for entry in entries {
        let normalized_name = normalize_card_name(&entry.name);
        if entry.is_commander || selected_commanders.contains(&normalized_name) {
            // Commander ordering is an explicit user selection/deck-section
            // contract. Preserve the relative order of those objects while
            // removing unrelated library-line order from their card indices.
            commanders.push(entry);
        } else {
            noncommanders.push((normalized_name, entry));
        }
    }

    // Decklist text order is presentation metadata, not game state. Compile
    // library identities in a canonical order so the same canonical deck and
    // seed cannot produce different shuffled identities or planner action
    // tie-breaks merely because noncommander lines were rearranged. Quantity
    // and spelling are deterministic secondary keys for split duplicate
    // entries; line numbers intentionally remain parser/report metadata.
    noncommanders.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.quantity.cmp(&right.1.quantity))
            .then_with(|| left.1.name.cmp(&right.1.name))
    });

    commanders.extend(noncommanders.into_iter().map(|(_, entry)| entry));
    commanders
}

fn compile_deck_internal(
    entries: &[DeckEntry],
    definitions: &HashMap<String, CardDefinition>,
    selected_commanders: &[String],
    additional_known_lines: &[KnownLine],
    semantic_overrides: Option<&SemanticOverridePackage>,
    comprehensive_rules: Option<&ComprehensiveRulesSnapshot>,
) -> (CompiledDeck, SemanticOverrideApplicationSummary) {
    let selected = selected_commanders
        .iter()
        .map(|name| normalize_card_name(name))
        .collect::<HashSet<_>>();
    let compilation_entries = canonical_compilation_entries(entries, &selected);
    let mut cards = Vec::new();
    let mut ability_profiles = Vec::new();
    let mut weighted_confidence = 0.0f32;
    let mut total_cards = 0u32;
    let mut approximated_cards = Vec::new();
    let mut override_summary = SemanticOverrideApplicationSummary::default();

    for entry in compilation_entries {
        let normalized = normalize_card_name(&entry.name);
        let definition = definitions.get(&normalized);
        let type_line = definition
            .map(|definition| definition.type_line.clone())
            .unwrap_or_default();
        let mut colors = definition
            .map(|definition| definition.colors.clone())
            .unwrap_or_default();
        let (mut mana_value, roles, semantic_confidence, effects, ability_profile, ability_program) =
            if let Some(definition) = definition {
                let mut effects = compile_effect_descriptor(definition);
                let (mana_value, mut roles, role_confidence) = classify_card(definition);
                let mut semantic_confidence =
                    (role_confidence * 0.55 + effects.confidence * 0.45).clamp(0.0, 1.0);
                let mut rules_confidence_ceiling = None::<f32>;
                if let Some(rules) = comprehensive_rules {
                    let rules_application = apply_rules_capabilities(
                        &definition.oracle_text,
                        &definition.keywords,
                        rules,
                    );
                    roles |= rules_application.added_role_bits;
                    let has_rules_gaps = !rules_application.unsupported_clauses.is_empty();
                    for mechanic in &rules_application.recognized_mechanics {
                        override_summary
                            .rules_backed_mechanics
                            .push(format!("{} (CR {})", mechanic.name, mechanic.rule_id));
                    }
                    if has_rules_gaps {
                        override_summary
                            .rules_report_only_cards
                            .push(entry.name.clone());
                    }
                    effects
                        .unsupported_clauses
                        .extend(rules_application.unsupported_clauses);
                    effects.unsupported_clauses.sort();
                    effects.unsupported_clauses.dedup();
                    if has_rules_gaps {
                        // The rulebook establishes what each keyword means, but
                        // this registry intentionally contributes only strategic
                        // roles. Until a typed executor exists, do not let the
                        // newly recognized mechanic imply high simulation
                        // coverage.
                        rules_confidence_ceiling = Some(0.72);
                    }
                }
                if let Some(overrides) = semantic_overrides {
                    match overrides.match_card(definition) {
                        OverrideMatch::None => {}
                        OverrideMatch::OracleTextGuardMismatch => {
                            override_summary
                                .oracle_text_guard_mismatches
                                .push(entry.name.clone());
                        }
                        OverrideMatch::Applied(annotation) => {
                            for removed in &annotation.remove_roles {
                                roles &= !semantic_role_mask(*removed);
                            }
                            for added in &annotation.add_roles {
                                roles |= semantic_role_mask(*added);
                            }
                            if let Some(confidence) = annotation.semantic_confidence {
                                semantic_confidence = confidence;
                            }
                            if let Some(metadata) = &annotation.effect_support {
                                for clause in &metadata.unsupported_clauses {
                                    if !effects
                                        .unsupported_clauses
                                        .iter()
                                        .any(|existing| existing.eq_ignore_ascii_case(clause))
                                    {
                                        effects.unsupported_clauses.push(clause.clone());
                                    }
                                }
                            }
                            if !effects.unsupported_clauses.is_empty() {
                                let unsupported_ceiling = (0.80
                                    - (effects.unsupported_clauses.len() - 1) as f32 * 0.04)
                                    .max(0.50);
                                semantic_confidence = semantic_confidence.min(unsupported_ceiling);
                            }
                            if annotation.oracle_text_sha256.is_none() {
                                override_summary
                                    .unguarded_applied_cards
                                    .push(entry.name.clone());
                            }
                            override_summary.applied_cards.push(entry.name.clone());
                        }
                    }
                }
                if let Some(ceiling) = rules_confidence_ceiling {
                    // A reviewed semantic override may refine a card, but it
                    // cannot silently turn a rules capability that is explicitly
                    // report-only into executable simulation coverage.
                    semantic_confidence = semantic_confidence.min(ceiling);
                }
                let oracle_input = OracleCardInput {
                    name: &definition.name,
                    layout: &definition.layout,
                    type_line: &definition.type_line,
                    oracle_text: &definition.oracle_text,
                    has_face_records: !definition.faces.is_empty(),
                };
                let ability_program = if definition.faces.is_empty() {
                    compile_executable_ability_program(oracle_input)
                } else {
                    let face_inputs = definition
                        .faces
                        .iter()
                        .map(|face| OracleCardFaceInput {
                            name: &face.name,
                            type_line: &face.type_line,
                            oracle_text: &face.oracle_text,
                        })
                        .collect::<Vec<_>>();
                    let face_characteristics = definition
                        .faces
                        .iter()
                        .map(|face| FaceCastCharacteristicsInput {
                            name: &face.name,
                            type_line: &face.type_line,
                            mana_cost: face.mana_cost.as_deref(),
                            colors: &face.colors,
                            color_indicator: &face.color_indicator,
                            keywords: &face.keywords,
                            power: face.power.as_deref(),
                            toughness: face.toughness.as_deref(),
                        })
                        .collect::<Vec<_>>();
                    compile_face_bound_ability_program_with_characteristics(
                        oracle_input,
                        &face_inputs,
                        &face_characteristics,
                    )
                };
                effects.retain_exact_mana_network_program(&definition.type_line, &ability_program);
                let complete_necropotence_lifecycle =
                    ability_program_has_complete_necropotence_lifecycle(&ability_program);
                if ability_program_has_necropotence_lifecycle_candidate(&ability_program)
                    && !complete_necropotence_lifecycle
                {
                    // The scalar descriptor has no representation for the
                    // linked draw-step restriction, exact discarded-object
                    // trigger, and delayed face-down top-card transaction.
                    // Keep one explicit gap whenever the typed compiler owns
                    // that family but cannot prove the entire physical-card
                    // root executable.
                    const GAP: &str =
                        "Linked draw-step, discard-exile, and delayed-access lifecycle";
                    if !effects
                        .unsupported_clauses
                        .iter()
                        .any(|clause| clause.eq_ignore_ascii_case(GAP))
                    {
                        effects.unsupported_clauses.push(GAP.into());
                        effects.unsupported_clauses.sort();
                    }
                    let unsupported_ceiling =
                        (0.80 - (effects.unsupported_clauses.len() - 1) as f32 * 0.04).max(0.50);
                    semantic_confidence = semantic_confidence.min(unsupported_ceiling);
                }
                if effects.unsupported_clauses.is_empty()
                    && ability_program_has_complete_burst_card_access(&ability_program)
                {
                    // The legacy scalar descriptor intentionally cannot
                    // express Wheel/Ad-Nauseam-style ordered transactions.
                    // Their exact, name-independent typed program is stronger
                    // evidence than that descriptor's low scalar confidence,
                    // so do not keep calling the complete card function an
                    // approximation. Any unsupported sibling or mutated root
                    // fails the predicate below and retains the old ceiling.
                    semantic_confidence = semantic_confidence
                        .max((role_confidence * 0.55 + 0.96 * 0.45).clamp(0.0, 1.0));
                }
                if effects.unsupported_clauses.is_empty() && complete_necropotence_lifecycle {
                    // The scalar descriptor deliberately has no lossy
                    // stand-in for this linked permanent lifecycle. The
                    // exclusive, structurally revalidated typed root may
                    // supersede only that low scalar confidence. Any
                    // unsupported clause contributed by rules or a reviewed
                    // override remains attached and prevents this bridge.
                    semantic_confidence = semantic_confidence
                        .max((role_confidence * 0.55 + 0.96 * 0.45).clamp(0.0, 1.0));
                }
                if let Some(ceiling) = rules_confidence_ceiling {
                    semantic_confidence = semantic_confidence.min(ceiling);
                }
                let ability_profile = compile_ability_profile(definition, &effects);
                (
                    mana_value,
                    roles,
                    semantic_confidence,
                    effects,
                    ability_profile,
                    ability_program,
                )
            } else {
                (
                    0.0,
                    infer_roles_from_name(&entry.name),
                    0.0,
                    EffectDescriptor::default(),
                    AbilityProfile::unresolved(),
                    ExecutableAbilityProgramV1::unresolved(),
                )
            };
        let structural = effects.structural_characteristics.battlefield_profile(None);
        if let Some(exact) = structural.mana_value {
            mana_value = exact.as_number() as f32;
        }
        let printed_power = structural.fixed_power().and_then(|value| {
            (value.denominator == 1)
                .then(|| i16::try_from(value.numerator).ok())
                .flatten()
        });
        let printed_toughness = structural.fixed_toughness().and_then(|value| {
            (value.denominator == 1)
                .then(|| i16::try_from(value.numerator).ok())
                .flatten()
        });
        if colors.is_empty()
            && effects.hand_zone.exact
            && !type_line.contains(" // ")
            && !entry.name.contains(" // ")
        {
            // A single-face object's color is the same characteristic in hand
            // and on the battlefield unless a typed effect changes it. The
            // hand compiler can recover that exact characteristic from the
            // printed mana cost/color indicator when an older local Scryfall
            // row retained an empty root `colors` array. Reuse it here so
            // color-dependent permanent mana (for example, Mox Amber) does
            // not remain broken after Chrome Mox has been repaired.
            colors.clone_from(&effects.hand_zone.colors);
        }
        let is_commander = entry.is_commander || selected.contains(&normalized);
        weighted_confidence += semantic_confidence * entry.quantity as f32;
        total_cards += entry.quantity as u32;
        if (semantic_confidence < 0.62 || !effects.unsupported_clauses.is_empty())
            && !approximated_cards.contains(&entry.name)
        {
            approximated_cards.push(entry.name.clone());
        }
        ability_profiles.push(ability_profile);
        cards.push(CompiledCard {
            name: entry.name.clone(),
            normalized_name: normalized,
            type_line,
            colors,
            quantity: entry.quantity,
            mana_value,
            printed_power,
            printed_toughness,
            roles,
            is_commander,
            semantic_confidence,
            effects,
            ability_program,
        });
    }

    let mut library = Vec::new();
    let mut commanders = Vec::new();
    for (index, card) in cards.iter().enumerate() {
        if card.is_commander {
            commanders.push(index);
            for _ in 1..card.quantity {
                library.push(index);
            }
        } else {
            for _ in 0..card.quantity {
                library.push(index);
            }
        }
    }

    let mut known_lines = detect_known_lines(&cards);
    let mut line_keys = known_lines
        .iter()
        .map(known_line_identity_key)
        .collect::<HashSet<_>>();
    for line in additional_known_lines.iter().take(64) {
        if line_keys.insert(known_line_identity_key(line)) {
            known_lines.push(line.clone());
        }
    }
    promote_executable_artifact_tap_treasure_lines(&cards, &mut known_lines);
    promote_executable_maskwood_artifact_dwarf_treasure_lines(&cards, &mut known_lines);
    add_executable_infinite_mana_creature_overrun_attempts(&cards, &mut known_lines);
    let combo_names = known_lines
        .iter()
        .flat_map(|line| line.cards.iter())
        .map(|name| normalize_card_name(name))
        .collect::<HashSet<_>>();
    for card in &mut cards {
        if combo_names.contains(&card.normalized_name) {
            card.roles |= role::COMBO_PIECE;
        }
    }

    let graph_cards = cards
        .iter()
        .zip(ability_profiles.iter())
        .map(|(card, profile)| GraphCard {
            name: &card.name,
            normalized_name: &card.normalized_name,
            quantity: card.quantity,
            is_commander: card.is_commander,
            profile,
        })
        .collect::<Vec<_>>();
    let synergy_graph = build_synergy_graph(&graph_cards, &known_lines);
    let synergy = build_synergy_report(&cards, known_lines.clone(), synergy_graph);
    approximated_cards.sort();
    approximated_cards.truncate(16);
    let semantic_coverage = if total_cards == 0 {
        0.0
    } else {
        weighted_confidence / total_cards as f32
    };

    override_summary.applied_cards.sort();
    override_summary.applied_cards.dedup();
    override_summary.unguarded_applied_cards.sort();
    override_summary.unguarded_applied_cards.dedup();
    override_summary.oracle_text_guard_mismatches.sort();
    override_summary.oracle_text_guard_mismatches.dedup();
    override_summary.rules_backed_mechanics.sort();
    override_summary.rules_backed_mechanics.dedup();
    override_summary.rules_report_only_cards.sort();
    override_summary.rules_report_only_cards.dedup();
    let mut compiled = CompiledDeck {
        cards,
        library,
        commanders,
        known_lines,
        synergy,
        semantic_coverage,
        approximated_cards,
    };
    compiled.synergy.strategic_profile = Some(
        crate::strategic_profile::classify_strategic_profile(&compiled),
    );
    (compiled, override_summary)
}

fn ability_program_has_complete_burst_card_access(program: &ExecutableAbilityProgramV1) -> bool {
    if program.abilities.len() != 1 || program.unsupported_abilities().next().is_some() {
        return false;
    }
    let mut executable = program.executable_abilities();
    let Some(ability) = executable.next() else {
        return false;
    };
    executable.next().is_none()
        && ability.timing == AbilityTiming::SpellResolution
        && ability.costs.is_empty()
        && ability.effects.len() == 1
        && matches!(
            ability.effects[0],
            AbilityEffect::WholeHandDiscardThenDraw(_) | AbilityEffect::RepeatableTopCardReveal(_)
        )
}

fn ability_program_has_necropotence_lifecycle_candidate(
    program: &ExecutableAbilityProgramV1,
) -> bool {
    program.necropotence_lifecycle.is_some()
        || program
            .face_programs
            .iter()
            .any(|face| face.necropotence_lifecycle.is_some())
}

fn ability_program_has_complete_necropotence_lifecycle(
    program: &ExecutableAbilityProgramV1,
) -> bool {
    // Semantic confidence is for the complete physical card, not merely one
    // attractive primary-face action. Until every retained face participates
    // in the same coverage accounting, a multiface card must remain a gap.
    if !program.abilities.is_empty()
        || !program.face_programs.is_empty()
        || program.self_transfer_tutor_permanent.is_some()
        || program.entry_linked_permanent.is_some()
        || program.atomic_transaction.is_some()
        || program.unsupported_necropotence_lifecycle().is_some()
    {
        return false;
    }

    let Some(lifecycle) = program.executable_necropotence_lifecycle() else {
        return false;
    };
    lifecycle.draw_step.player == ControllerRelation::You
        && lifecycle.draw_step.step == TurnStep::Draw
        && lifecycle.draw_step.procedure == StepProcedure::Skip
        && lifecycle.discarded_card.player == ControllerRelation::You
        && lifecycle.discarded_card.event == NecropotenceDiscardEvent::WheneverYouDiscardOneCard
        && lifecycle.discarded_card.tracked_object
            == DiscardedObjectReference::CardDiscardedByThisTrigger
        && lifecycle.discarded_card.from == Zone::Graveyard
        && lifecycle.discarded_card.destination == Zone::Exile
        && lifecycle.activation.source_zone == Zone::Battlefield
        && lifecycle.activation.window == ActivationWindow::NormalPriority
        && lifecycle.activation.costs == [AbilityCost::PayLife(1)]
        && lifecycle.activation.access.player == ControllerRelation::You
        && lifecycle.activation.access.count == 1
        && lifecycle.activation.access.from == Zone::Library
        && lifecycle.activation.access.source_position == LibraryPosition::Top
        && lifecycle.activation.access.intermediate == Zone::Exile
        && lifecycle.activation.access.face_down
        && lifecycle.activation.access.tracked_object
            == DelayedObjectReference::CardMovedByThisEffect
        && lifecycle.activation.access.delayed_event == DelayedEvent::BeginningOfYourNextEndStep
        && lifecycle.activation.access.destination == Zone::Hand
}

fn semantic_role_mask(role: SemanticRole) -> u32 {
    match role {
        SemanticRole::ManaSource => role::MANA_SOURCE,
        SemanticRole::Ramp => role::RAMP,
        SemanticRole::FastMana => role::FAST_MANA,
        SemanticRole::Draw => role::DRAW,
        SemanticRole::Tutor => role::TUTOR,
        SemanticRole::Removal => role::REMOVAL,
        SemanticRole::Counterspell => role::COUNTERSPELL,
        SemanticRole::BoardWipe => role::BOARD_WIPE,
        SemanticRole::Protection => role::PROTECTION,
        SemanticRole::Engine => role::ENGINE,
        SemanticRole::Enabler => role::ENABLER,
        SemanticRole::Payoff => role::PAYOFF,
        SemanticRole::WinCondition => role::WIN_CONDITION,
        SemanticRole::ComboPiece => role::COMBO_PIECE,
        SemanticRole::Graveyard => role::GRAVEYARD,
        SemanticRole::Token => role::TOKEN,
        SemanticRole::Sacrifice => role::SACRIFICE,
        SemanticRole::Stax => role::STAX,
        SemanticRole::Recursion => role::RECURSION,
        SemanticRole::SpellMatters => role::SPELL_MATTERS,
        SemanticRole::ArtifactMatters => role::ARTIFACT_MATTERS,
        SemanticRole::EnchantmentMatters => role::ENCHANTMENT_MATTERS,
        SemanticRole::TokenMatters => role::TOKEN_MATTERS,
        SemanticRole::DeathMatters => role::DEATH_MATTERS,
        SemanticRole::CreatureMatters => role::CREATURE_MATTERS,
    }
}

fn known_line_identity_key(line: &KnownLine) -> String {
    let mut cards = line
        .cards
        .iter()
        .map(|name| normalize_card_name(name))
        .collect::<Vec<_>>();
    cards.sort_unstable();
    cards.join("|")
}

fn promote_executable_artifact_tap_treasure_lines(cards: &[CompiledCard], lines: &mut [KnownLine]) {
    for line in lines {
        if line
            .simulation_requirements
            .contains(&LineRequirement::Unmodeled)
            && line_has_typed_artifact_tap_treasure_cycle(line, cards)
        {
            line.simulation_requirements = vec![LineRequirement::ExecutableArtifactTapTreasureLoop];
        }
    }
}

fn line_has_typed_artifact_tap_treasure_cycle(line: &KnownLine, cards: &[CompiledCard]) -> bool {
    let members = line
        .cards
        .iter()
        .filter_map(|name| {
            let normalized = normalize_card_name(name);
            let mut matches = cards
                .iter()
                .filter(|card| card.normalized_name == normalized);
            let card = matches.next()?;
            matches.next().is_none().then_some(card)
        })
        .collect::<Vec<_>>();
    if members.len() != line.cards.len() {
        return false;
    }

    let has_dwarf_tap_treasure_trigger = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(
                &ability.timing,
                AbilityTiming::Triggered { event }
                    if event.kind == TriggerEventKind::PermanentBecomesTapped
                        && event.object_filter.subtype.as_deref() == Some("Dwarf")
            ) && ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    AbilityEffect::CreateToken(token)
                        if token.kind == TokenKind::Treasure && token.count >= 1
                )
            })
        })
    });
    let has_artifact_tap_untap_engine = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(ability.timing, AbilityTiming::Activated { .. })
                && ability.costs.iter().any(|cost| {
                    matches!(
                        cost,
                        AbilityCost::TapPermanents {
                            filter,
                            count,
                            exclude_source: false,
                        } if *count == 2
                            && filter.card_type == Some(ProgramCardType::Artifact)
                    )
                })
                && ability.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        AbilityEffect::Untap(TargetSelector::Target(filter))
                            if filter.card_type == Some(ProgramCardType::Artifact)
                    )
                })
        })
    });
    let has_artifact_dwarf = members.iter().any(|card| {
        card.effects.card_types.is_artifact && compiled_card_has_subtype(card, "Dwarf")
    });

    has_dwarf_tap_treasure_trigger && has_artifact_tap_untap_engine && has_artifact_dwarf
}

fn compiled_card_has_subtype(card: &CompiledCard, subtype: &str) -> bool {
    card.type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(subtype))
        || card
            .ability_program
            .executable_abilities()
            .any(|ability| ability.normalized_oracle.eq_ignore_ascii_case("Changeling"))
        || card
            .ability_program
            .unsupported_abilities()
            .any(|ability| ability.normalized_oracle.eq_ignore_ascii_case("Changeling"))
}

fn promote_executable_maskwood_artifact_dwarf_treasure_lines(
    cards: &[CompiledCard],
    lines: &mut [KnownLine],
) {
    for line in lines {
        if line
            .simulation_requirements
            .contains(&LineRequirement::Unmodeled)
            && line_has_typed_maskwood_artifact_dwarf_treasure_cycle(line, cards)
        {
            let empty_catalog_total = line
                .mana_needed
                .as_deref()
                .is_some_and(|cost| cost.trim().is_empty());
            line.simulation_requirements.retain(|requirement| {
                requirement != &LineRequirement::Unmodeled
                    && !(empty_catalog_total && requirement == &LineRequirement::TotalExecutionMana)
            });
            line.simulation_requirements
                .push(LineRequirement::ExecutableMaskwoodArtifactDwarfTreasureLoop);
            // The witness establishes unbounded Treasure production only. It
            // does not prove a typed outlet or a table-ending conversion.
            line.is_infinite = true;
            line.table_lethal_if_resolved = false;
            line.outcome = KnownLineOutcome::InfiniteEngine;
        }
    }
}

fn line_has_typed_maskwood_artifact_dwarf_treasure_cycle(
    line: &KnownLine,
    cards: &[CompiledCard],
) -> bool {
    let members = line
        .cards
        .iter()
        .filter_map(|name| {
            let normalized = normalize_card_name(name);
            let mut matches = cards
                .iter()
                .filter(|card| card.normalized_name == normalized);
            let card = matches.next()?;
            matches.next().is_none().then_some(card)
        })
        .collect::<Vec<_>>();
    if members.len() != line.cards.len() {
        return false;
    }

    let has_dwarf_tap_treasure_trigger = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(
                &ability.timing,
                AbilityTiming::Triggered { event }
                    if event.kind == TriggerEventKind::PermanentBecomesTapped
                        && event.actor == crate::ability_program::ControllerRelation::You
                        && event.object_filter.subtype.as_deref() == Some("Dwarf")
                        && event.object_filter.controller
                            == Some(crate::ability_program::ControllerRelation::You)
            ) && ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    AbilityEffect::CreateToken(token)
                        if token.kind == TokenKind::Treasure && token.count >= 1
                )
            })
        })
    });
    let has_artifact_entry_optional_self_untap = members.iter().any(|card| {
        card.effects.card_types.is_artifact
            && card.effects.card_types.is_creature
            && card.ability_program.executable_abilities().any(|ability| {
                matches!(
                    &ability.timing,
                    AbilityTiming::Triggered { event }
                        if event.kind == TriggerEventKind::PermanentEntersBattlefield
                            && event.actor
                                == crate::ability_program::ControllerRelation::Any
                            && event.object_filter.card_type
                                == Some(ProgramCardType::Artifact)
                            && event.object_filter.controller.is_none()
                ) && ability.effects
                    == [AbilityEffect::OptionalUntap(TargetSelector::SelfPermanent)]
            })
    });
    let has_tap_untapped_dwarf_activation = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(ability.timing, AbilityTiming::Activated { .. })
                && ability.costs.iter().any(|cost| {
                    matches!(
                        cost,
                        AbilityCost::TapPermanents {
                            filter,
                            count,
                            exclude_source: false,
                        } if *count == 1
                            && filter.subtype.as_deref() == Some("Dwarf")
                            && filter.controller
                                == Some(crate::ability_program::ControllerRelation::You)
                    )
                })
                && ability.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        AbilityEffect::ModifyPowerToughnessUntilEndOfTurn(modifier)
                            if modifier.power_delta == 2
                                && modifier.toughness_delta == 0
                                && matches!(
                                    &modifier.target,
                                    TargetSelector::Target(filter)
                                        if filter.card_type
                                            == Some(ProgramCardType::Creature)
                                )
                    )
                })
        })
    });
    let has_exact_all_creature_types_static = members.iter().any(|card| {
        card.effects.card_types.is_artifact
            && card.ability_program.executable_abilities().any(|ability| {
                ability.timing == AbilityTiming::StaticModifier
                    && ability.effects.iter().any(|effect| {
                        matches!(
                            effect,
                            AbilityEffect::GrantAllCreatureTypes(scope)
                                if scope.creatures_you_control
                                    && scope.creature_spells_you_control
                                    && scope.nonbattlefield_creature_cards_you_own
                        )
                    })
            })
    });

    has_dwarf_tap_treasure_trigger
        && has_artifact_entry_optional_self_untap
        && has_tap_untapped_dwarf_activation
        && has_exact_all_creature_types_static
}

fn add_executable_infinite_mana_creature_overrun_attempts(
    cards: &[CompiledCard],
    lines: &mut Vec<KnownLine>,
) {
    let mut identities = lines
        .iter()
        .map(known_line_identity_key)
        .collect::<HashSet<_>>();
    let base_lines = lines.clone();
    let mut additions = Vec::new();

    for base in base_lines.iter().filter(|line| {
        line.simulation_requirements
            .contains(&LineRequirement::ReviewedInfiniteManaLoop)
            && line_has_typed_infinite_mana_cycle(line, cards)
    }) {
        let base_names = base
            .cards
            .iter()
            .map(|name| normalize_card_name(name))
            .collect::<HashSet<_>>();
        for conversion in cards
            .iter()
            .filter(|card| !base_names.contains(&card.normalized_name))
            .filter(|card| card_has_typed_variable_creature_overrun(card))
            .take(4)
        {
            let mut derived = base.clone();
            derived.name = format!("{} + typed variable-mana creature overrun", base.name);
            derived.cards.push(conversion.name.clone());
            derived.compactness = derived.cards.len().min(usize::from(u8::MAX)) as u8;
            derived.is_infinite = true;
            // This flag is the legacy bounded simulator's first-attempt
            // signal. The endpoint-separated executor never treats it as
            // strict table-resolution proof.
            derived.table_lethal_if_resolved = true;
            derived.outcome = KnownLineOutcome::TableWin;
            derived.mana_needed = None;
            derived.prerequisites = vec![
                "The typed permanent cycle must produce unbounded colorless mana before the variable-X spell is cast.".into(),
                "The ordinary mana model must still pay every fixed colored pip in the spell's printed cost.".into(),
                "The spell must resolve in the precombat main phase with enough attack-capable creatures for a three-opponent combat attempt.".into(),
                "Opponent blockers, damage assignment, life totals, and the combat result are not modeled as a resolved table win.".into(),
            ];
            derived.model_confidence = base.model_confidence.min(0.94);
            derived.simulation_requirements = vec![
                LineRequirement::NamedCardsPayPrintedCosts,
                LineRequirement::ReviewedInfiniteManaLoop,
                LineRequirement::ExecutableInfiniteManaCreatureOverrunAttempt,
            ];

            if identities.insert(known_line_identity_key(&derived)) {
                additions.push(derived);
            }
        }
    }
    lines.extend(additions);
}

fn line_has_typed_infinite_mana_cycle(line: &KnownLine, cards: &[CompiledCard]) -> bool {
    let members = line
        .cards
        .iter()
        .filter_map(|name| {
            let normalized = normalize_card_name(name);
            let mut matches = cards
                .iter()
                .filter(|card| card.normalized_name == normalized);
            let card = matches.next()?;
            matches.next().is_none().then_some(card)
        })
        .collect::<Vec<_>>();
    if members.len() != line.cards.len()
        || members.iter().any(|card| {
            card.ability_program
                .unsupported_abilities()
                .next()
                .is_some()
        })
    {
        return false;
    }

    let has_nonland_plus_one = members.iter().any(|card| {
        card.ability_program.executable_abilities().any(|ability| {
            matches!(
                ability.timing,
                AbilityTiming::Triggered {
                    event: crate::ability_program::TriggerEvent {
                        kind: TriggerEventKind::PermanentTappedForMana,
                        actor: ControllerRelation::You,
                        ..
                    }
                }
            ) && ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    AbilityEffect::ModifyNonlandMana(modifier)
                        if modifier.additional_amount >= 1
                            && modifier.kind
                                == ProgramManaKind::AnyTypeProducedByTriggeringPermanent
                )
            })
        })
    });
    let has_repeatable_source = members.iter().any(|card| {
        if card.effects.card_types.is_land {
            return false;
        }
        let mut taps_for_at_least_three = false;
        let mut pays_three_to_untap_self = false;
        for ability in card.ability_program.executable_abilities() {
            if matches!(ability.timing, AbilityTiming::Activated { .. })
                && ability.costs == [AbilityCost::TapSelf]
                && ability.effects.iter().any(|effect| {
                    matches!(
                        effect,
                        AbilityEffect::AddMana(mana)
                            if mana.amount >= 3
                                && matches!(
                                    mana.kind,
                                    ProgramManaKind::Fixed(profile)
                                        if profile.colorless >= 3
                                )
                    )
                })
            {
                taps_for_at_least_three = true;
            }
            if matches!(ability.timing, AbilityTiming::Activated { .. })
                && ability.costs.len() == 1
                && matches!(
                    &ability.costs[0],
                    AbilityCost::Mana(ProgramManaCost::PrintedSymbols { profile, .. })
                        if profile.generic == 3
                            && profile.white == 0
                            && profile.blue == 0
                            && profile.black == 0
                            && profile.red == 0
                            && profile.green == 0
                            && profile.colorless == 0
                            && profile.variable_x == 0
                )
                && ability
                    .effects
                    .contains(&AbilityEffect::Untap(TargetSelector::SelfPermanent))
            {
                pays_three_to_untap_self = true;
            }
        }
        taps_for_at_least_three && pays_three_to_untap_self
    });
    has_nonland_plus_one && has_repeatable_source
}

fn card_has_typed_variable_creature_overrun(card: &CompiledCard) -> bool {
    let is_sorcery = card
        .type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case("Sorcery"));
    if !is_sorcery
        || card
            .ability_program
            .unsupported_abilities()
            .next()
            .is_some()
    {
        return false;
    }

    let mut tutor_count = 0;
    let mut overrun_count = 0;
    for ability in card.ability_program.executable_abilities() {
        if ability.timing != AbilityTiming::SpellResolution {
            return false;
        }
        for effect in &ability.effects {
            match effect {
                AbilityEffect::VariableCreatureTutor(VariableCreatureTutorEffect {
                    from_library: true,
                    from_graveyard: true,
                    destination: crate::ability_program::Zone::Battlefield,
                    mana_value_at_most_x: true,
                    shuffle_if_library_searched: true,
                }) => tutor_count += 1,
                AbilityEffect::VariableCreatureOverrun(VariableCreatureOverrunEffect {
                    minimum_x: 10,
                    creatures_you_control: true,
                    power_bonus_equals_x: true,
                    toughness_bonus_equals_x: true,
                    grants_haste: true,
                    until_end_of_turn: true,
                }) => overrun_count += 1,
                _ => return false,
            }
        }
    }
    tutor_count == 1 && overrun_count == 1
}

fn classify_card(card: &CardDefinition) -> (f32, u32, f32) {
    let type_line = card.type_line.to_ascii_lowercase();
    let oracle = card.oracle_text.to_ascii_lowercase();
    let name = card.name.to_ascii_lowercase();
    let mut roles = 0u32;
    let mut recognized_signals = 0u8;

    if type_line.contains("land") {
        roles |= role::LAND | role::MANA_SOURCE;
        recognized_signals += 1;
    }
    if type_line.contains("creature") {
        roles |= role::CREATURE;
    }
    if type_line.contains("artifact") {
        roles |= role::ARTIFACT;
    }
    if type_line.contains("enchantment") {
        roles |= role::ENCHANTMENT;
    }
    if type_line.contains("instant") || type_line.contains("sorcery") {
        roles |= role::INSTANT_SORCERY;
    }

    let adds_mana = oracle.contains(": add {")
        || oracle.contains("add one mana")
        || oracle.contains("add two mana")
        || oracle.contains("add three mana")
        || oracle.contains("add an amount of");
    let land_ramp = oracle.contains("search your library")
        && (oracle.contains("basic land") || oracle.contains("land card"))
        && oracle.contains("battlefield");
    if !type_line.contains("land") && (adds_mana || land_ramp) {
        roles |= role::RAMP | role::MANA_SOURCE;
        recognized_signals += 1;
        if card.mana_value <= 1.0
            || (card.mana_value <= 2.0
                && (oracle.contains("add two")
                    || oracle.contains("add {c}{c}")
                    || oracle.contains("sacrifice")))
            || known_fast_mana().contains(name.as_str())
        {
            roles |= role::FAST_MANA;
        }
    }

    if oracle.contains("draw a card")
        || oracle.contains("draw two cards")
        || oracle.contains("draw three cards")
        || oracle.contains("draw cards equal")
        || oracle.contains("draw that many cards")
    {
        roles |= role::DRAW;
        recognized_signals += 1;
    }

    let searches_library = oracle.contains("search your library");
    let only_land_search = searches_library
        && (oracle.contains("basic land") || oracle.contains("land card"))
        && !oracle.contains("and/or");
    if searches_library
        && !only_land_search
        && (oracle.contains("for a card")
            || oracle.contains("for an artifact")
            || oracle.contains("for a creature")
            || oracle.contains("for an enchantment")
            || oracle.contains("for an instant")
            || oracle.contains("for a sorcery")
            || oracle.contains("for up to one"))
    {
        roles |= role::TUTOR;
        recognized_signals += 1;
    }

    if oracle.contains("destroy target")
        || oracle.contains("exile target")
        || oracle.contains("return target") && oracle.contains("to its owner's hand")
        || oracle.contains("target creature gets -")
    {
        roles |= role::REMOVAL;
        recognized_signals += 1;
    }
    if oracle.contains("counter target spell") {
        roles |= role::COUNTERSPELL;
        recognized_signals += 1;
    }
    if oracle.contains("destroy all")
        || contains_mass_battlefield_exile(&oracle)
        || oracle.contains("all creatures get -")
        || oracle.contains("each player sacrifices all")
    {
        roles |= role::BOARD_WIPE;
        recognized_signals += 1;
    }
    if oracle.contains("hexproof")
        || oracle.contains("indestructible")
        || oracle.contains("phase out")
        || oracle.contains("counter target spell or ability that targets")
    {
        roles |= role::PROTECTION;
        recognized_signals += 1;
    }

    if oracle.contains("create ") && oracle.contains(" token") {
        roles |= role::TOKEN | role::ENABLER;
        recognized_signals += 1;
    }
    if has_token_matter_text(&oracle) {
        roles |= role::TOKEN_MATTERS;
    }
    if oracle.contains("sacrifice a creature")
        || oracle.contains("sacrifice another creature")
        || oracle.contains("sacrifice a permanent")
        || oracle.contains("sacrifice another permanent")
    {
        roles |= role::SACRIFICE | role::ENABLER;
        recognized_signals += 1;
    }
    if has_death_matter_text(&oracle) {
        roles |= role::DEATH_MATTERS;
    }
    if oracle.contains("from your graveyard")
        || oracle.contains("in your graveyard")
        || oracle.contains("return target") && oracle.contains("graveyard")
        || oracle.contains("cast target") && oracle.contains("graveyard")
    {
        roles |= role::GRAVEYARD;
        if oracle.contains("return") || oracle.contains("cast") {
            roles |= role::RECURSION;
        }
        recognized_signals += 1;
    }
    if oracle.contains("players can't")
        || oracle.contains("your opponents can't")
        || oracle.contains("spells your opponents cast cost")
        || oracle.contains("enter the battlefield tapped")
        || oracle.contains("enters tapped")
    {
        roles |= role::STAX;
        recognized_signals += 1;
    }
    if has_spell_matter_text(&oracle) {
        roles |= role::SPELL_MATTERS;
    }
    if has_artifact_matter_text(&oracle) {
        roles |= role::ARTIFACT_MATTERS;
    }
    if has_enchantment_matter_text(&oracle) {
        roles |= role::ENCHANTMENT_MATTERS;
    }
    if has_creature_matter_text(&oracle) {
        roles |= role::CREATURE_MATTERS;
    }

    if has_explicit_table_win_text(&oracle) {
        roles |= role::WIN_CONDITION | role::PAYOFF;
        recognized_signals += 1;
    } else if has_incremental_opponent_pressure(&oracle)
        || oracle.contains("whenever") && oracle.contains("deals damage")
        || oracle.contains("gets +1/+1 for each")
        || oracle.contains("double") && oracle.contains("damage")
    {
        roles |= role::PAYOFF;
        recognized_signals += 1;
    }

    let repeatable_trigger = oracle.contains("whenever")
        || oracle.contains("at the beginning of")
        || oracle.contains("the first time")
        || oracle.contains("once each turn");
    if repeatable_trigger
        && roles & (role::DRAW | role::TOKEN | role::RAMP | role::PAYOFF | role::RECURSION) != 0
    {
        roles |= role::ENGINE;
    }
    if oracle.contains("you may play")
        || oracle.contains("you may cast")
        || oracle.contains("look at the top") && oracle.contains("you may")
    {
        roles |= role::ENGINE;
        recognized_signals += 1;
    }

    if known_tutors().contains(name.as_str()) {
        roles |= role::TUTOR;
        recognized_signals += 1;
    }
    if known_fast_mana().contains(name.as_str()) {
        roles |= role::RAMP | role::MANA_SOURCE | role::FAST_MANA;
        recognized_signals += 1;
    }

    let confidence = if card.oracle_text.trim().is_empty() {
        if roles & role::LAND != 0 { 0.96 } else { 0.48 }
    } else {
        let complexity_penalty =
            (card.oracle_text.len().saturating_sub(280) as f32 / 1_200.0).clamp(0.0, 0.22);
        (0.64 + recognized_signals.min(3) as f32 * 0.10 - complexity_penalty).clamp(0.42, 0.94)
    };

    (card.mana_value, roles, confidence)
}

fn contains_mass_battlefield_exile(oracle: &str) -> bool {
    const BATTLEFIELD_OBJECT_WORDS: [&str; 16] = [
        "artifact",
        "artifacts",
        "battle",
        "battles",
        "creature",
        "creatures",
        "enchantment",
        "enchantments",
        "land",
        "lands",
        "permanent",
        "permanents",
        "planeswalker",
        "planeswalkers",
        "token",
        "tokens",
    ];

    oracle.match_indices("exile all").any(|(offset, _)| {
        let clause = oracle[offset..]
            .split(['.', ',', ';', '\n'])
            .next()
            .unwrap_or_default();
        let words = clause
            .split(|character: char| !character.is_ascii_alphabetic())
            .filter(|word| !word.is_empty())
            .collect::<HashSet<_>>();
        !words.contains("card")
            && !words.contains("cards")
            && BATTLEFIELD_OBJECT_WORDS
                .iter()
                .any(|object| words.contains(object))
    })
}

/// A WIN_CONDITION role is strategic metadata, not proof that its condition is
/// currently satisfied. Keep the role limited to text that explicitly ends
/// the whole Commander game for this card's controller. Single-opponent loss,
/// finite life loss/damage, and mill are pressure or payoff effects instead.
fn has_explicit_table_win_text(oracle: &str) -> bool {
    oracle.contains("you win the game")
        || oracle.contains("each opponent loses the game")
        || oracle.contains("all opponents lose the game")
        || oracle.contains("your opponents lose the game")
}

fn has_incremental_opponent_pressure(oracle: &str) -> bool {
    oracle.contains("opponent loses ")
        || oracle.contains("opponents lose ")
        || oracle.contains("player loses ")
        || oracle.contains("players lose ")
        || oracle.contains("damage to each opponent")
        || oracle.contains("damage to target opponent")
        || oracle.contains("damage to that opponent")
        || oracle.contains("damage to that player")
        || oracle.contains("opponent mills")
        || oracle.contains("opponents mill")
        || oracle.contains("their library into their graveyard")
}

fn has_spell_matter_text(oracle: &str) -> bool {
    [
        "instant or sorcery spell you cast",
        "instant and sorcery spells you cast",
        "whenever you cast an instant or sorcery",
        "whenever you cast a noncreature spell",
        "noncreature spells you cast",
        "noncreature spell you cast",
        "instant and/or sorcery",
        "instants and sorceries",
        "instant or sorcery card",
        "instant and sorcery card",
        "magecraft",
        "copy target instant or sorcery",
        "copy each instant and sorcery",
    ]
    .iter()
    .any(|signal| oracle.contains(signal))
}

fn has_artifact_matter_text(oracle: &str) -> bool {
    [
        "artifacts you control",
        "artifact you control",
        "artifact card in your",
        "artifact cards in your",
        "artifact spell you cast",
        "artifact spells you cast",
        "cast an artifact spell",
        "whenever an artifact",
        "whenever one or more artifacts",
        "for each artifact",
        "number of artifacts",
        "sacrifice an artifact",
        "affinity for artifacts",
        "metalcraft",
        "improvise",
    ]
    .iter()
    .any(|signal| oracle.contains(signal))
}

fn has_enchantment_matter_text(oracle: &str) -> bool {
    [
        "enchantments you control",
        "enchantment you control",
        "enchantment card in your",
        "enchantment cards in your",
        "enchantment spell you cast",
        "enchantment spells you cast",
        "cast an enchantment spell",
        "whenever an enchantment",
        "whenever one or more enchantments",
        "for each enchantment",
        "number of enchantments",
        "constellation",
    ]
    .iter()
    .any(|signal| oracle.contains(signal))
}

fn has_token_matter_text(oracle: &str) -> bool {
    [
        "tokens you control",
        "token you control",
        "one or more tokens",
        "for each token",
        "number of tokens",
        "token enters",
        "tokens enter",
        "token dies",
        "tokens die",
        "double the number of",
        "populate",
    ]
    .iter()
    .any(|signal| oracle.contains(signal))
}

fn has_death_matter_text(oracle: &str) -> bool {
    let references_death = oracle.contains(" dies")
        || oracle.contains(" die")
        || oracle.contains("died this turn")
        || oracle.contains("have died");
    references_death
        && (oracle.contains("when ")
            || oracle.contains("whenever ")
            || oracle.contains("if ")
            || oracle.contains("for each ")
            || oracle.contains("died this turn")
            || oracle.contains("have died"))
}

fn has_creature_matter_text(oracle: &str) -> bool {
    [
        "creatures you control get",
        "creatures you control have",
        "other creatures you control",
        "creature spells you cast",
        "whenever a creature enters under your control",
        "whenever one or more creatures",
        "for each creature you control",
        "number of creatures you control",
    ]
    .iter()
    .any(|signal| oracle.contains(signal))
}

fn infer_roles_from_name(name: &str) -> u32 {
    let lowercase = name.to_ascii_lowercase();
    if matches!(
        lowercase.as_str(),
        "plains" | "island" | "swamp" | "mountain" | "forest" | "wastes"
    ) {
        role::LAND | role::MANA_SOURCE
    } else if known_fast_mana().contains(lowercase.as_str()) {
        role::RAMP | role::MANA_SOURCE | role::FAST_MANA
    } else if known_tutors().contains(lowercase.as_str()) {
        role::TUTOR
    } else {
        0
    }
}

fn known_fast_mana() -> &'static HashSet<&'static str> {
    use std::sync::LazyLock;
    static NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        [
            "mana crypt",
            "mana vault",
            "jeweled lotus",
            "chrome mox",
            "mox diamond",
            "mox opal",
            "lotus petal",
            "lion's eye diamond",
            "grim monolith",
            "sol ring",
            "dark ritual",
            "cabal ritual",
            "rite of flame",
        ]
        .into_iter()
        .collect()
    });
    &NAMES
}

fn known_tutors() -> &'static HashSet<&'static str> {
    use std::sync::LazyLock;
    static NAMES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
        [
            "demonic tutor",
            "vampiric tutor",
            "imperial seal",
            "mystical tutor",
            "worldly tutor",
            "enlightened tutor",
            "gamble",
            "diabolic intent",
            "finale of devastation",
            "transmute artifact",
            "entomb",
            "survival of the fittest",
            "birthing pod",
        ]
        .into_iter()
        .collect()
    });
    &NAMES
}

struct KnownLineSpec {
    name: &'static str,
    cards: &'static [&'static str],
    is_infinite: bool,
    table_lethal_if_resolved: bool,
    outcome: KnownLineOutcome,
    mana_needed: Option<&'static str>,
    prerequisites: &'static [&'static str],
    simulation_requirements: &'static [LineRequirement],
    model_confidence: f32,
}

fn detect_known_lines(cards: &[CompiledCard]) -> Vec<KnownLine> {
    let available = cards
        .iter()
        .map(|card| card.normalized_name.as_str())
        .collect::<HashSet<_>>();
    let candidates = [
        KnownLineSpec {
            name: "Oracle consultation",
            cards: &["Thassa's Oracle", "Demonic Consultation"],
            is_infinite: false,
            table_lethal_if_resolved: true,
            outcome: KnownLineOutcome::TableWin,
            mana_needed: Some("{U}{U}{B}"),
            prerequisites: &[
                "Resolve the library-exile spell while Thassa's Oracle's trigger can convert the empty-library state.",
            ],
            simulation_requirements: &[
                LineRequirement::NamedCardsPayPrintedCosts,
                LineRequirement::ReviewedEmptyLibrarySequence,
            ],
            model_confidence: 0.98,
        },
        KnownLineSpec {
            name: "Oracle pact",
            cards: &["Thassa's Oracle", "Tainted Pact"],
            is_infinite: false,
            table_lethal_if_resolved: true,
            outcome: KnownLineOutcome::TableWin,
            mana_needed: Some("{1}{U}{U}{B}"),
            prerequisites: &[
                "Tainted Pact must be able to exile enough of the library before a repeated card name stops it.",
            ],
            simulation_requirements: &[
                LineRequirement::SingletonLibrary,
                LineRequirement::NamedCardsPayPrintedCosts,
                LineRequirement::ReviewedEmptyLibrarySequence,
            ],
            model_confidence: 0.94,
        },
        KnownLineSpec {
            name: "Kinnan basalt loop",
            cards: &["Kinnan, Bonder Prodigy", "Basalt Monolith"],
            is_infinite: true,
            table_lethal_if_resolved: false,
            outcome: KnownLineOutcome::InfiniteMana,
            mana_needed: None,
            prerequisites: &[
                "Kinnan increases Basalt Monolith's nonland mana output so it can repeatedly pay its own untap cost and net colorless mana.",
                "A compatible outlet or colored-mana conversion is still required to turn infinite colorless mana into a table win.",
            ],
            simulation_requirements: &[LineRequirement::ReviewedInfiniteManaLoop],
            model_confidence: 0.98,
        },
        KnownLineSpec {
            name: "Breach loop",
            cards: &["Underworld Breach", "Lion's Eye Diamond", "Brain Freeze"],
            is_infinite: true,
            table_lethal_if_resolved: true,
            outcome: KnownLineOutcome::TableWin,
            mana_needed: Some("{1}{R}"),
            prerequisites: &[
                "The graveyard must contain enough cards to establish the escape loop and support the chosen conversion line.",
            ],
            simulation_requirements: &[
                LineRequirement::GraveyardSetup {
                    minimum_cast_cards: 4,
                },
                LineRequirement::NamedCardsPayPrintedCosts,
                LineRequirement::ExecutableGraveyardStormLoop,
            ],
            model_confidence: 0.86,
        },
        KnownLineSpec {
            name: "Scepter reversal",
            cards: &["Isochron Scepter", "Dramatic Reversal"],
            is_infinite: true,
            table_lethal_if_resolved: false,
            outcome: KnownLineOutcome::InfiniteMana,
            mana_needed: Some("{2}"),
            prerequisites: &[
                "Nonland mana permanents must produce more than the Scepter activation costs; a separate payoff is required.",
            ],
            simulation_requirements: &[
                LineRequirement::NonlandManaCapacity { minimum: 3 },
                LineRequirement::AdditionalActivationMana { cost: "{2}" },
                LineRequirement::Unmodeled,
            ],
            model_confidence: 0.92,
        },
        KnownLineSpec {
            name: "Chatterfang treasure loop",
            cards: &["Chatterfang, Squirrel General", "Pitiless Plunderer"],
            is_infinite: true,
            table_lethal_if_resolved: false,
            outcome: KnownLineOutcome::InfiniteEngine,
            mana_needed: Some("{B}"),
            prerequisites: &[
                "You control an additional Squirrel.",
                "A downstream death, token, mana, or sacrifice payoff is required to convert the loop into a table win.",
            ],
            simulation_requirements: &[
                LineRequirement::AdditionalCreature { count: 1 },
                LineRequirement::AdditionalActivationMana { cost: "{B}" },
                LineRequirement::Unmodeled,
            ],
            model_confidence: 0.97,
        },
        KnownLineSpec {
            name: "Exquisite blood loop",
            cards: &["Exquisite Blood", "Sanguine Bond"],
            is_infinite: true,
            table_lethal_if_resolved: true,
            outcome: KnownLineOutcome::TableWin,
            mana_needed: None,
            prerequisites: &["A life-gain or opponent-life-loss event must start the loop."],
            simulation_requirements: &[
                LineRequirement::ExternalEnabler,
                LineRequirement::Unmodeled,
            ],
            model_confidence: 0.98,
        },
        KnownLineSpec {
            name: "Exquisite blood loop",
            cards: &["Exquisite Blood", "Vito, Thorn of the Dusk Rose"],
            is_infinite: true,
            table_lethal_if_resolved: true,
            outcome: KnownLineOutcome::TableWin,
            mana_needed: None,
            prerequisites: &["A life-gain or opponent-life-loss event must start the loop."],
            simulation_requirements: &[
                LineRequirement::ExternalEnabler,
                LineRequirement::Unmodeled,
            ],
            model_confidence: 0.98,
        },
        KnownLineSpec {
            name: "Kiki-Jiki combo",
            cards: &["Kiki-Jiki, Mirror Breaker", "Zealous Conscripts"],
            is_infinite: true,
            table_lethal_if_resolved: true,
            outcome: KnownLineOutcome::TableWin,
            mana_needed: None,
            prerequisites: &[
                "The hasty creature tokens must be able to attack and deal combat damage.",
            ],
            simulation_requirements: &[LineRequirement::CombatAccess],
            model_confidence: 0.96,
        },
        KnownLineSpec {
            name: "Heliod ballista",
            cards: &["Heliod, Sun-Crowned", "Walking Ballista"],
            is_infinite: true,
            table_lethal_if_resolved: true,
            outcome: KnownLineOutcome::TableWin,
            mana_needed: Some("{1}{W}"),
            prerequisites: &[
                "Walking Ballista needs at least two +1/+1 counters before Heliod grants it lifelink.",
            ],
            simulation_requirements: &[
                LineRequirement::AdditionalActivationMana { cost: "{1}{W}" },
                LineRequirement::Unmodeled,
            ],
            model_confidence: 0.96,
        },
        KnownLineSpec {
            name: "Devoted mana loop",
            cards: &["Devoted Druid", "Vizier of Remedies"],
            is_infinite: true,
            table_lethal_if_resolved: false,
            outcome: KnownLineOutcome::InfiniteMana,
            mana_needed: None,
            prerequisites: &[
                "Devoted Druid must be able to activate, and a separate green-mana payoff is required.",
            ],
            simulation_requirements: &[LineRequirement::Unmodeled],
            model_confidence: 0.96,
        },
    ];

    candidates
        .iter()
        .filter(|candidate| {
            candidate
                .cards
                .iter()
                .all(|name| available.contains(normalize_card_name(name).as_str()))
        })
        .map(|candidate| KnownLine {
            name: candidate.name.into(),
            cards: candidate
                .cards
                .iter()
                .map(|card| (*card).to_string())
                .collect(),
            compactness: candidate.cards.len() as u8,
            is_infinite: candidate.is_infinite,
            table_lethal_if_resolved: candidate.table_lethal_if_resolved,
            outcome: candidate.outcome,
            mana_needed: candidate.mana_needed.map(str::to_string),
            prerequisites: candidate
                .prerequisites
                .iter()
                .map(|prerequisite| (*prerequisite).to_string())
                .collect(),
            model_confidence: candidate.model_confidence,
            simulation_requirements: candidate.simulation_requirements.to_vec(),
        })
        .collect()
}

fn build_synergy_report(
    cards: &[CompiledCard],
    known_lines: Vec<KnownLine>,
    graph: SynergyGraph,
) -> SynergyReport {
    let role_specs = [
        ("Lands", role::LAND),
        ("Ramp", role::RAMP),
        ("Fast mana", role::FAST_MANA),
        ("Card advantage", role::DRAW),
        ("Tutors", role::TUTOR),
        ("Spot interaction", role::REMOVAL | role::COUNTERSPELL),
        ("Board wipes", role::BOARD_WIPE),
        ("Protection", role::PROTECTION),
        ("Engines", role::ENGINE),
        ("Payoffs", role::PAYOFF),
        ("Win conditions", role::WIN_CONDITION),
        ("Combo pieces", role::COMBO_PIECE),
    ];
    let role_counts = role_specs
        .iter()
        .map(|(label, mask)| RoleCount {
            role: (*label).into(),
            count: count_cards(cards, *mask),
        })
        .collect::<Vec<_>>();

    let total_nonlands = cards
        .iter()
        .filter(|card| !card.has(role::LAND))
        .map(|card| card.quantity as u32)
        .sum::<u32>()
        .max(1);
    let mut detected_plans = Vec::new();

    // Plan labels need both density and plan-specific evidence. Generic value
    // engines/payoffs are intentionally not interchangeable: a pile of mana
    // rocks plus removal spells is neither an artifact deck nor spellslinger.
    let token_sources = count_cards(cards, role::TOKEN);
    let token_payoffs = count_cards(cards, role::TOKEN_MATTERS);
    let sacrifice_outlets = count_cards(cards, role::SACRIFICE);
    let death_payoffs = count_cards(cards, role::DEATH_MATTERS);
    if (token_sources >= 4 && token_payoffs >= 1) || token_sources >= 8 {
        detected_plans.push(StrategyPlan {
            name: "Token value".into(),
            confidence: (0.22
                + token_sources as f32 / 18.0 * 0.45
                + token_payoffs as f32 / 8.0 * 0.25
                + sacrifice_outlets as f32 / 8.0 * 0.08)
                .min(0.96),
            supporting_cards: plan_supporting_cards(cards, role::TOKEN_MATTERS, role::TOKEN),
        });
    }
    if sacrifice_outlets >= 2 && death_payoffs >= 1 || (death_payoffs >= 2 && token_sources >= 3) {
        detected_plans.push(StrategyPlan {
            name: "Aristocrats".into(),
            confidence: (0.18
                + sacrifice_outlets as f32 / 8.0 * 0.32
                + death_payoffs as f32 / 8.0 * 0.32
                + token_sources as f32 / 18.0 * 0.16)
                .min(0.96),
            supporting_cards: plan_supporting_cards(
                cards,
                role::SACRIFICE | role::DEATH_MATTERS,
                role::TOKEN,
            ),
        });
    }

    let graveyard_cards = count_cards(cards, role::GRAVEYARD);
    let recursion_cards = count_cards(cards, role::RECURSION);
    if graveyard_cards >= 4 && recursion_cards >= 2 {
        detected_plans.push(StrategyPlan {
            name: "Graveyard recursion".into(),
            confidence: (0.20
                + graveyard_cards as f32 / 18.0 * 0.42
                + recursion_cards as f32 / 10.0 * 0.34)
                .min(0.96),
            supporting_cards: plan_supporting_cards(cards, role::RECURSION, role::GRAVEYARD),
        });
    }

    let spells = count_cards(cards, role::INSTANT_SORCERY);
    let spell_anchors = count_cards(cards, role::SPELL_MATTERS);
    if spell_anchors >= 4 && spells >= 12 {
        detected_plans.push(StrategyPlan {
            name: "Spellslinger".into(),
            confidence: (0.18 + spell_anchors as f32 / 7.0 * 0.48 + spells as f32 / 30.0 * 0.30)
                .min(0.96),
            supporting_cards: plan_supporting_cards(
                cards,
                role::SPELL_MATTERS,
                role::INSTANT_SORCERY,
            ),
        });
    }

    let artifacts = count_cards(cards, role::ARTIFACT);
    let artifact_anchors = count_cards(cards, role::ARTIFACT_MATTERS);
    if (artifact_anchors >= 2 && artifacts >= 15) || (artifact_anchors >= 1 && artifacts >= 25) {
        detected_plans.push(StrategyPlan {
            name: "Artifact engine".into(),
            confidence: (0.18
                + artifact_anchors as f32 / 7.0 * 0.48
                + artifacts as f32 / 30.0 * 0.30)
                .min(0.96),
            supporting_cards: plan_supporting_cards(cards, role::ARTIFACT_MATTERS, role::ARTIFACT),
        });
    }

    let enchantments = count_cards(cards, role::ENCHANTMENT);
    let enchantment_anchors = count_cards(cards, role::ENCHANTMENT_MATTERS);
    if (enchantment_anchors >= 2 && enchantments >= 12)
        || (enchantment_anchors >= 1 && enchantments >= 20)
    {
        detected_plans.push(StrategyPlan {
            name: "Enchantress".into(),
            confidence: (0.18
                + enchantment_anchors as f32 / 7.0 * 0.48
                + enchantments as f32 / 26.0 * 0.30)
                .min(0.96),
            supporting_cards: plan_supporting_cards(
                cards,
                role::ENCHANTMENT_MATTERS,
                role::ENCHANTMENT,
            ),
        });
    }

    let creatures = count_cards(cards, role::CREATURE);
    let creature_anchors = count_cards(cards, role::CREATURE_MATTERS);
    let creature_payoffs = cards
        .iter()
        .filter(|card| card.has(role::CREATURE) && card.has(role::PAYOFF))
        .fold(0u16, |total, card| total.saturating_add(card.quantity));
    if creatures >= 20 && (creature_anchors >= 5 || creature_payoffs >= 8) {
        detected_plans.push(StrategyPlan {
            name: "Creature pressure".into(),
            confidence: (0.16
                + creatures as f32 / 38.0 * 0.55
                + creature_anchors as f32 / 8.0 * 0.20
                + creature_payoffs as f32 / 12.0 * 0.08)
                .min(0.96),
            supporting_cards: plan_supporting_cards(
                cards,
                role::CREATURE_MATTERS | role::PAYOFF,
                role::CREATURE,
            ),
        });
    }
    if !known_lines.is_empty() {
        detected_plans.push(StrategyPlan {
            name: "Compact combo".into(),
            confidence: (0.65 + known_lines.len() as f32 * 0.1).min(0.98),
            supporting_cards: known_lines
                .iter()
                .flat_map(|line| line.cards.iter().cloned())
                .take(8)
                .collect(),
        });
    }
    detected_plans.sort_by(|left, right| right.confidence.total_cmp(&left.confidence));
    detected_plans.truncate(4);

    let top_plan_density = detected_plans
        .first()
        .map(|plan| plan.confidence)
        .unwrap_or(0.2);
    let redundancy = (count_cards(cards, role::ENGINE | role::ENABLER | role::PAYOFF) as f32
        / total_nonlands as f32)
        .min(1.0);
    let cohesion_score = ((top_plan_density * 62.0 + redundancy * 38.0).round() as u8).min(100);

    let commander_cards = cards
        .iter()
        .filter(|card| card.is_commander)
        .collect::<Vec<_>>();
    let commander_dependence = if commander_cards.is_empty() {
        0.35
    } else {
        let commander_engine_weight = commander_cards
            .iter()
            .filter(|card| card.has(role::ENGINE | role::ENABLER | role::PAYOFF))
            .count() as f32
            / commander_cards.len() as f32;
        let independent_engines = count_cards(
            cards
                .iter()
                .filter(|card| !card.is_commander)
                .cloned()
                .collect::<Vec<_>>()
                .as_slice(),
            role::ENGINE,
        ) as f32;
        (0.30 + commander_engine_weight * 0.48 - (independent_engines / 30.0)).clamp(0.12, 0.88)
    };

    let orphaned_cards = cards
        .iter()
        .filter(|card| {
            !card.is_commander
                && !card.has(
                    role::LAND
                        | role::RAMP
                        | role::DRAW
                        | role::TUTOR
                        | role::REMOVAL
                        | role::COUNTERSPELL
                        | role::BOARD_WIPE
                        | role::PROTECTION
                        | role::ENGINE
                        | role::ENABLER
                        | role::PAYOFF
                        | role::WIN_CONDITION
                        | role::COMBO_PIECE,
                )
                && card.semantic_confidence >= 0.6
        })
        .take(12)
        .map(|card| card.name.clone())
        .collect();

    SynergyReport {
        detected_plans,
        known_lines,
        role_counts,
        strategic_profile: None,
        graph,
        commander_dependence,
        cohesion_score,
        orphaned_cards,
    }
}

fn count_cards(cards: &[CompiledCard], mask: u32) -> u16 {
    cards
        .iter()
        .filter(|card| card.has(mask))
        .fold(0u16, |total, card| total.saturating_add(card.quantity))
}

fn plan_supporting_cards(
    cards: &[CompiledCard],
    preferred_mask: u32,
    foundation_mask: u32,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut supporting = Vec::new();
    for card in cards
        .iter()
        .filter(|card| card.has(preferred_mask))
        .chain(cards.iter().filter(|card| card.has(foundation_mask)))
    {
        if seen.insert(card.normalized_name.clone()) {
            supporting.push(card.name.clone());
            if supporting.len() == 8 {
                break;
            }
        }
    }
    supporting
}

/// Whether a library card is direct, plan-specific opening evidence. This is
/// deliberately narrower than `ENGINE`: generic card advantage and tutors do
/// not automatically establish access to every strategy in the deck.
pub(crate) fn card_supports_strategy_plan(card: &CompiledCard, plan_name: &str) -> bool {
    match plan_name {
        "Token value" => card.has(role::TOKEN | role::TOKEN_MATTERS),
        "Aristocrats" => {
            card.has(role::SACRIFICE | role::DEATH_MATTERS)
                || card.has(role::TOKEN) && card.has(role::ENABLER)
        }
        "Graveyard recursion" => {
            card.has(role::RECURSION)
                || card.has(role::GRAVEYARD)
                    && card.has(role::ENGINE | role::ENABLER | role::PAYOFF)
        }
        "Spellslinger" => card.has(role::SPELL_MATTERS),
        "Artifact engine" => card.has(role::ARTIFACT_MATTERS),
        "Enchantress" => card.has(role::ENCHANTMENT_MATTERS),
        "Creature pressure" => {
            card.has(role::CREATURE_MATTERS) || card.has(role::CREATURE) && card.has(role::PAYOFF)
        }
        "Compact combo" => card.has(role::COMBO_PIECE),
        _ => false,
    }
}
