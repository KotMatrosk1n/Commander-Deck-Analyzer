//! Exact structural programs for bounded utility and modal effects.
//!
//! Card names are not classifier inputs. A program is selected only from the
//! normal layout, the exact source type envelope, and the complete normalized
//! Oracle root. Programs that share a root with independently modeled clauses
//! claim only the clause indices consumed by this module.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UtilityModalCardInput<'a> {
    pub layout: &'a str,
    pub type_line: &'a str,
    pub oracle_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CardType {
    Artifact,
    Battle,
    Conspiracy,
    Creature,
    Dungeon,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Phenomenon,
    Plane,
    Planeswalker,
    Scheme,
    Sorcery,
    Vanguard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreatureSubtype {
    Faerie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceZone {
    Battlefield,
    Stack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceRequirement {
    pub zone: SourceZone,
    pub card_types: Vec<CardType>,
    pub required_creature_subtype: Option<CreatureSubtype>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ManaPayment {
    pub generic: u16,
    pub white: u8,
    pub blue: u8,
    pub black: u8,
    pub red: u8,
    pub green: u8,
    pub colorless: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationWindow {
    NormalPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActivatedAbilityCost {
    pub mana: ManaPayment,
    pub tap_source: bool,
    pub sacrifice_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Controller {
    You,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibraryPosition {
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DestinationController {
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerEvent {
    SourceEntersBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScryOperation {
    pub player: Controller,
    pub count: u8,
    pub may_put_any_number_on_bottom: bool,
    pub may_order_cards_left_on_top: bool,
    pub may_order_cards_put_on_bottom: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopLibraryStep {
    LookAtTopCards {
        player: Controller,
        count: u8,
    },
    ReturnLookedAtCardsInAnyOrder {
        player: Controller,
        position: LibraryPosition,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawRelocateStep {
    DrawCards {
        player: Controller,
        count: u8,
    },
    MoveSourceToLibrary {
        destination_controller: DestinationController,
        position: LibraryPosition,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopLibraryProgram {
    pub source: SourceRequirement,
    pub reorder_cost: ActivatedAbilityCost,
    pub reorder_window: ActivationWindow,
    pub reorder_steps: Vec<TopLibraryStep>,
    pub draw_relocate_cost: ActivatedAbilityCost,
    pub draw_relocate_window: ActivationWindow,
    pub draw_relocate_steps: Vec<DrawRelocateStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellScryDrawStep {
    Scry(ScryOperation),
    DrawCards { player: Controller, count: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpellScryDrawProgram {
    pub source: SourceRequirement,
    pub ordered_steps: Vec<SpellScryDrawStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryScryProgram {
    pub source: SourceRequirement,
    pub trigger: TriggerEvent,
    pub scry: ScryOperation,
    pub draw_cost: ActivatedAbilityCost,
    pub draw_window: ActivationWindow,
    pub draw_count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetController {
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CreatureTarget {
    pub controller: TargetController,
    pub must_be_legendary: bool,
    pub maximum_mana_value: Option<u16>,
    pub must_have_dealt_damage_to_you_this_turn: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreventionScope {
    NextDamageEventThisTurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetedDamagePreventionProgram {
    pub source: SourceRequirement,
    pub cost: ActivatedAbilityCost,
    pub window: ActivationWindow,
    pub target: CreatureTarget,
    pub amount: u8,
    pub scope: PreventionScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermanentType {
    Artifact,
    Creature,
    Enchantment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermanentAction {
    Destroy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivatedWipeProgram {
    pub source: SourceRequirement,
    pub source_enters_tapped: bool,
    pub cost: ActivatedAbilityCost,
    pub window: ActivationWindow,
    pub action: PermanentAction,
    pub affected_types: Vec<PermanentType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetaliatoryDestroyProgram {
    pub source: SourceRequirement,
    pub cost: ActivatedAbilityCost,
    pub window: ActivationWindow,
    pub target: CreatureTarget,
    pub action: PermanentAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalChoice {
    ChooseExactlyOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalCreatureMode {
    Destroy { target: CreatureTarget },
    ReturnToOwnersHand { target: CreatureTarget },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModalCreatureInteractionProgram {
    pub source: SourceRequirement,
    pub choice: ModalChoice,
    pub modes: Vec<ModalCreatureMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DynamicThreshold {
    PermanentsYouControlWithCreatureSubtype(CreatureSubtype),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThresholdEvaluation {
    TargetSelectionAndResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpellTarget {
    pub maximum_mana_value: DynamicThreshold,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaerieThresholdCounterProgram {
    pub source: SourceRequirement,
    pub trigger: TriggerEvent,
    pub target: SpellTarget,
    pub threshold_evaluation: ThresholdEvaluation,
    pub counter_target_spell: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UtilityModalRuntimeProgram {
    TopLibrary(TopLibraryProgram),
    SpellScryDraw(SpellScryDrawProgram),
    EntryScry(EntryScryProgram),
    TargetedDamagePrevention(TargetedDamagePreventionProgram),
    ActivatedWipe(ActivatedWipeProgram),
    RetaliatoryDestroy(RetaliatoryDestroyProgram),
    ModalCreatureInteraction(ModalCreatureInteractionProgram),
    FaerieThresholdCounter(FaerieThresholdCounterProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OracleOwnership {
    CompleteRoot { clause_count: u16 },
    ExactClauseSet { clause_indices: Vec<u16> },
}

impl OracleOwnership {
    pub(crate) fn owned_clause_indices(&self) -> Vec<u16> {
        match self {
            Self::CompleteRoot { clause_count } => (0..*clause_count).collect(),
            Self::ExactClauseSet { clause_indices } => clause_indices.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledUtilityModal {
    pub normalized_oracle_clauses: Vec<String>,
    pub ownership: OracleOwnership,
    pub program: UtilityModalRuntimeProgram,
}

pub(crate) fn compile_utility_modal_runtime(
    input: UtilityModalCardInput<'_>,
) -> Option<CompiledUtilityModal> {
    if !input.layout.trim().eq_ignore_ascii_case("normal") {
        return None;
    }
    let clauses = normalize_oracle_root(input.oracle_text)?;

    compile_top_library(input.type_line, &clauses)
        .or_else(|| compile_spell_scry_draw(input.type_line, &clauses))
        .or_else(|| compile_entry_scry(input.type_line, &clauses))
        .or_else(|| compile_targeted_damage_prevention(input.type_line, &clauses))
        .or_else(|| compile_activated_wipe(input.type_line, &clauses))
        .or_else(|| compile_retaliatory_destroy(input.type_line, &clauses))
        .or_else(|| compile_modal_creature_interaction(input.type_line, &clauses))
        .or_else(|| compile_faerie_threshold_counter(input.type_line, &clauses))
}

fn compile_top_library(type_line: &str, clauses: &[String]) -> Option<CompiledUtilityModal> {
    if !exact_source_type(type_line, &[CardType::Artifact], None)
        || clauses
            != [
                "{1}: look at the top three cards of your library, then put them back in any order",
                "{t}: draw a card, then put this artifact on top of its owner's library",
            ]
    {
        return None;
    }

    Some(compiled(
        clauses,
        OracleOwnership::CompleteRoot { clause_count: 2 },
        UtilityModalRuntimeProgram::TopLibrary(TopLibraryProgram {
            source: battlefield_source(&[CardType::Artifact], None),
            reorder_cost: cost(mana(1, 0, 0, 0, 0, 0), false, false),
            reorder_window: ActivationWindow::NormalPriority,
            reorder_steps: vec![
                TopLibraryStep::LookAtTopCards {
                    player: Controller::You,
                    count: 3,
                },
                TopLibraryStep::ReturnLookedAtCardsInAnyOrder {
                    player: Controller::You,
                    position: LibraryPosition::Top,
                },
            ],
            draw_relocate_cost: cost(ManaPayment::default(), true, false),
            draw_relocate_window: ActivationWindow::NormalPriority,
            draw_relocate_steps: vec![
                DrawRelocateStep::DrawCards {
                    player: Controller::You,
                    count: 1,
                },
                DrawRelocateStep::MoveSourceToLibrary {
                    destination_controller: DestinationController::Owner,
                    position: LibraryPosition::Top,
                },
            ],
        }),
    ))
}

fn compile_spell_scry_draw(type_line: &str, clauses: &[String]) -> Option<CompiledUtilityModal> {
    if !exact_source_type(type_line, &[CardType::Instant], None)
        || clauses != ["scry 1", "draw a card"]
    {
        return None;
    }

    Some(compiled(
        clauses,
        OracleOwnership::CompleteRoot { clause_count: 2 },
        UtilityModalRuntimeProgram::SpellScryDraw(SpellScryDrawProgram {
            source: stack_source(&[CardType::Instant]),
            ordered_steps: vec![
                SpellScryDrawStep::Scry(exact_scry(1)),
                SpellScryDrawStep::DrawCards {
                    player: Controller::You,
                    count: 1,
                },
            ],
        }),
    ))
}

fn compile_entry_scry(type_line: &str, clauses: &[String]) -> Option<CompiledUtilityModal> {
    if !exact_source_type(type_line, &[CardType::Artifact], None)
        || clauses
            != [
                "when this artifact enters, scry 2",
                "{3}{u}, sacrifice this artifact: draw two cards",
            ]
    {
        return None;
    }

    Some(compiled(
        clauses,
        OracleOwnership::CompleteRoot { clause_count: 2 },
        UtilityModalRuntimeProgram::EntryScry(EntryScryProgram {
            source: battlefield_source(&[CardType::Artifact], None),
            trigger: TriggerEvent::SourceEntersBattlefield,
            scry: exact_scry(2),
            draw_cost: cost(mana(3, 0, 1, 0, 0, 0), false, true),
            draw_window: ActivationWindow::NormalPriority,
            draw_count: 2,
        }),
    ))
}

fn compile_targeted_damage_prevention(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledUtilityModal> {
    if !exact_source_type(type_line, &[CardType::Land], None)
        || clauses
            != [
                "{t}: add {w}",
                "{w}, {t}: prevent the next 2 damage that would be dealt to target legendary creature this turn",
            ]
    {
        return None;
    }

    Some(compiled(
        clauses,
        OracleOwnership::ExactClauseSet {
            clause_indices: vec![1],
        },
        UtilityModalRuntimeProgram::TargetedDamagePrevention(TargetedDamagePreventionProgram {
            source: battlefield_source(&[CardType::Land], None),
            cost: cost(mana(0, 1, 0, 0, 0, 0), true, false),
            window: ActivationWindow::NormalPriority,
            target: CreatureTarget {
                controller: TargetController::Any,
                must_be_legendary: true,
                maximum_mana_value: None,
                must_have_dealt_damage_to_you_this_turn: false,
            },
            amount: 2,
            scope: PreventionScope::NextDamageEventThisTurn,
        }),
    ))
}

fn compile_activated_wipe(type_line: &str, clauses: &[String]) -> Option<CompiledUtilityModal> {
    if !exact_source_type(type_line, &[CardType::Artifact], None)
        || clauses
            != [
                "this artifact enters tapped",
                "{1}, {t}: destroy all artifacts, creatures, and enchantments",
            ]
    {
        return None;
    }

    Some(compiled(
        clauses,
        OracleOwnership::CompleteRoot { clause_count: 2 },
        UtilityModalRuntimeProgram::ActivatedWipe(ActivatedWipeProgram {
            source: battlefield_source(&[CardType::Artifact], None),
            source_enters_tapped: true,
            cost: cost(mana(1, 0, 0, 0, 0, 0), true, false),
            window: ActivationWindow::NormalPriority,
            action: PermanentAction::Destroy,
            affected_types: vec![
                PermanentType::Artifact,
                PermanentType::Creature,
                PermanentType::Enchantment,
            ],
        }),
    ))
}

fn compile_retaliatory_destroy(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledUtilityModal> {
    if !exact_source_type(
        type_line,
        &[CardType::Artifact, CardType::Enchantment],
        None,
    ) || clauses
        != [
            "creatures you control get +1/+1",
            "{1}{w}{w}, {t}: destroy target creature that dealt damage to you this turn",
        ]
    {
        return None;
    }

    Some(compiled(
        clauses,
        OracleOwnership::ExactClauseSet {
            clause_indices: vec![1],
        },
        UtilityModalRuntimeProgram::RetaliatoryDestroy(RetaliatoryDestroyProgram {
            source: battlefield_source(&[CardType::Artifact, CardType::Enchantment], None),
            cost: cost(mana(1, 2, 0, 0, 0, 0), true, false),
            window: ActivationWindow::NormalPriority,
            target: CreatureTarget {
                controller: TargetController::Any,
                must_be_legendary: false,
                maximum_mana_value: None,
                must_have_dealt_damage_to_you_this_turn: true,
            },
            action: PermanentAction::Destroy,
        }),
    ))
}

fn compile_modal_creature_interaction(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledUtilityModal> {
    if !exact_source_type(type_line, &[CardType::Instant], None)
        || clauses
            != [
                "choose one -",
                "destroy target creature with mana value 3 or less",
                "return target creature to its owner's hand",
            ]
    {
        return None;
    }

    Some(compiled(
        clauses,
        OracleOwnership::CompleteRoot { clause_count: 3 },
        UtilityModalRuntimeProgram::ModalCreatureInteraction(ModalCreatureInteractionProgram {
            source: stack_source(&[CardType::Instant]),
            choice: ModalChoice::ChooseExactlyOne,
            modes: vec![
                ModalCreatureMode::Destroy {
                    target: CreatureTarget {
                        controller: TargetController::Any,
                        must_be_legendary: false,
                        maximum_mana_value: Some(3),
                        must_have_dealt_damage_to_you_this_turn: false,
                    },
                },
                ModalCreatureMode::ReturnToOwnersHand {
                    target: CreatureTarget {
                        controller: TargetController::Any,
                        must_be_legendary: false,
                        maximum_mana_value: None,
                        must_have_dealt_damage_to_you_this_turn: false,
                    },
                },
            ],
        }),
    ))
}

fn compile_faerie_threshold_counter(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledUtilityModal> {
    if !exact_source_type(
        type_line,
        &[CardType::Creature],
        Some(CreatureSubtype::Faerie),
    ) || clauses
        != [
            "flash",
            "flying",
            "when this creature enters, counter target spell with mana value x or less, where x is the number of faeries you control",
        ]
    {
        return None;
    }

    Some(compiled(
        clauses,
        OracleOwnership::ExactClauseSet {
            clause_indices: vec![2],
        },
        UtilityModalRuntimeProgram::FaerieThresholdCounter(FaerieThresholdCounterProgram {
            source: battlefield_source(&[CardType::Creature], Some(CreatureSubtype::Faerie)),
            trigger: TriggerEvent::SourceEntersBattlefield,
            target: SpellTarget {
                maximum_mana_value: DynamicThreshold::PermanentsYouControlWithCreatureSubtype(
                    CreatureSubtype::Faerie,
                ),
            },
            threshold_evaluation: ThresholdEvaluation::TargetSelectionAndResolution,
            counter_target_spell: true,
        }),
    ))
}

fn compiled(
    clauses: &[String],
    ownership: OracleOwnership,
    program: UtilityModalRuntimeProgram,
) -> CompiledUtilityModal {
    CompiledUtilityModal {
        normalized_oracle_clauses: clauses.to_vec(),
        ownership,
        program,
    }
}

fn battlefield_source(
    card_types: &[CardType],
    required_creature_subtype: Option<CreatureSubtype>,
) -> SourceRequirement {
    SourceRequirement {
        zone: SourceZone::Battlefield,
        card_types: card_types.to_vec(),
        required_creature_subtype,
    }
}

fn stack_source(card_types: &[CardType]) -> SourceRequirement {
    SourceRequirement {
        zone: SourceZone::Stack,
        card_types: card_types.to_vec(),
        required_creature_subtype: None,
    }
}

fn mana(generic: u16, white: u8, blue: u8, black: u8, red: u8, green: u8) -> ManaPayment {
    ManaPayment {
        generic,
        white,
        blue,
        black,
        red,
        green,
        colorless: 0,
    }
}

fn cost(mana: ManaPayment, tap_source: bool, sacrifice_source: bool) -> ActivatedAbilityCost {
    ActivatedAbilityCost {
        mana,
        tap_source,
        sacrifice_source,
    }
}

fn exact_scry(count: u8) -> ScryOperation {
    ScryOperation {
        player: Controller::You,
        count,
        may_put_any_number_on_bottom: true,
        may_order_cards_left_on_top: true,
        may_order_cards_put_on_bottom: true,
    }
}

fn exact_source_type(
    type_line: &str,
    expected_card_types: &[CardType],
    required_creature_subtype: Option<CreatureSubtype>,
) -> bool {
    let normalized = type_line
        .trim()
        .replace(['\u{2014}', '\u{2013}'], "-")
        .to_ascii_lowercase();
    let mut parts = normalized.splitn(2, '-');
    let card_type_part = parts.next().unwrap_or_default();
    let subtype_part = parts.next().unwrap_or_default();
    let mut actual_card_types = Vec::new();
    for word in card_type_part
        .split(|character: char| !character.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
    {
        let card_type = match word {
            "artifact" => Some(CardType::Artifact),
            "battle" => Some(CardType::Battle),
            "conspiracy" => Some(CardType::Conspiracy),
            "creature" => Some(CardType::Creature),
            "dungeon" => Some(CardType::Dungeon),
            "enchantment" => Some(CardType::Enchantment),
            "instant" => Some(CardType::Instant),
            "kindred" | "tribal" => Some(CardType::Kindred),
            "land" => Some(CardType::Land),
            "phenomenon" => Some(CardType::Phenomenon),
            "plane" => Some(CardType::Plane),
            "planeswalker" => Some(CardType::Planeswalker),
            "scheme" => Some(CardType::Scheme),
            "sorcery" => Some(CardType::Sorcery),
            "vanguard" => Some(CardType::Vanguard),
            "basic" | "legendary" | "ongoing" | "snow" | "world" => None,
            _ => return false,
        };
        if let Some(card_type) = card_type {
            if actual_card_types.contains(&card_type) {
                return false;
            }
            actual_card_types.push(card_type);
        }
    }
    actual_card_types.sort();
    let mut expected_card_types = expected_card_types.to_vec();
    expected_card_types.sort();
    expected_card_types.dedup();
    if actual_card_types != expected_card_types {
        return false;
    }
    match required_creature_subtype {
        Some(CreatureSubtype::Faerie) => has_word(subtype_part, "faerie"),
        None => true,
    }
}

fn has_word(value: &str, expected: &str) -> bool {
    value
        .split(|character: char| !character.is_ascii_alphabetic())
        .any(|word| word == expected)
}

fn normalize_oracle_root(oracle_text: &str) -> Option<Vec<String>> {
    let mut clauses = Vec::new();
    for raw_clause in oracle_text.trim().lines() {
        let normalized = raw_clause
            .trim()
            .trim_start_matches('\u{2022}')
            .trim()
            .replace('\u{2019}', "'")
            .replace(['\u{2014}', '\u{2013}'], "-")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        let without_reminder = remove_exact_scry_reminder(&normalized);
        let clause = without_reminder
            .strip_suffix('.')
            .unwrap_or(&without_reminder)
            .trim();
        if clause.is_empty() {
            return None;
        }
        clauses.push(clause.to_string());
    }
    (!clauses.is_empty()).then_some(clauses)
}

fn remove_exact_scry_reminder(value: &str) -> String {
    const SCRY_ONE_REMINDER: &str =
        " (look at the top card of your library. you may put that card on the bottom.)";
    const SCRY_TWO_REMINDER: &str = " (look at the top two cards of your library, then put any number of them on the bottom and the rest on top in any order.)";
    value
        .strip_suffix(SCRY_ONE_REMINDER)
        .or_else(|| value.strip_suffix(SCRY_TWO_REMINDER))
        .unwrap_or(value)
        .to_string()
}
