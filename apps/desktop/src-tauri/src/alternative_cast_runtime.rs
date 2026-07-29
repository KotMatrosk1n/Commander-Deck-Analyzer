//! Exact alternative cast programs for reviewed Alela cards.
//!
//! Printed and alternative cast paths remain separate through payment,
//! selection, and resolution. Card names are not classifier inputs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlternativeCastCardInput<'a> {
    pub layout: &'a str,
    pub mana_cost: &'a str,
    pub type_line: &'a str,
    pub oracle_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardType {
    Enchantment,
    Instant,
    Sorcery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardSubtype {
    Aura,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceRequirement {
    pub card_types: Vec<CardType>,
    pub required_subtypes: Vec<CardSubtype>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectZone {
    Battlefield,
    Exile,
    Graveyard,
    Hand,
    Library,
    Stack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManaSymbol {
    Generic(u16),
    Blue,
    White,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManaCost {
    pub symbols: Vec<ManaSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CastOptionKind {
    Printed,
    Overload,
    CommanderControlledFree,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CastCondition {
    CommanderPermanentYouControlOnBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetScope {
    Creature,
    CreatureYouDoNotControl,
    NoncreatureSpell,
    NonlandPermanentYouDoNotControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AffectedSet {
    EachCreatureYouDoNotControl,
    EachNonlandPermanentYouDoNotControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Selection {
    OneTarget(TargetScope),
    Each(AffectedSet),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetSetTransformation {
    ReplaceTargetWithEach { from: TargetScope, to: AffectedSet },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdditionalCastCost {
    ExileOtherGraveyardCards {
        count: u16,
        source_zone: ObjectZone,
        destination_zone: ObjectZone,
        exclude_casting_card: bool,
        require_distinct_physical_objects: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CastOption {
    pub kind: CastOptionKind,
    pub source_zone: ObjectZone,
    pub destination_zone: ObjectZone,
    pub mana_cost: Option<ManaCost>,
    pub waives_printed_mana_cost: bool,
    pub condition: Option<CastCondition>,
    pub additional_costs: Vec<AdditionalCastCost>,
    pub selection: Selection,
    pub target_transformation: Option<TargetSetTransformation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CastChoice {
    pub printed_cost: ManaCost,
    pub options: Vec<CastOption>,
    pub choose_exactly_one: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClauseRole {
    Resolution,
    AlternativeCostCondition,
    AuraTarget,
    StaticGrant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeywordKind {
    Escape,
    Overload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClauseOwnershipKind {
    Clause(ClauseRole),
    KeywordAndAlternativeCost(KeywordKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClauseOwnership {
    pub clause_index: u16,
    pub kind: ClauseOwnershipKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootOwnership {
    pub complete_root_required: bool,
    pub clause_count: u16,
    pub clauses: Vec<ClauseOwnership>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerScope {
    ControllerOfEachExiledCreature,
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BounceResolutionStep {
    ReturnSelectedPermanentsToOwnersHands { from: ObjectZone, to: ObjectZone },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BounceCastProgram {
    pub source: SourceRequirement,
    pub cast: CastChoice,
    pub ordered_resolution: Vec<BounceResolutionStep>,
    pub destination_controller: ControllerScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchCardPredicate {
    BasicLand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HiddenZoneSearchRule {
    MandatorySearchMayFailToFindQualifiedCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompensationResolutionStep {
    ExileSelectedCreatures {
        from: ObjectZone,
        to: ObjectZone,
    },
    SearchOncePerCreatureActuallyExiled {
        searching_player: ControllerScope,
        source_zone: ObjectZone,
        card: SearchCardPredicate,
        hidden_zone_rule: HiddenZoneSearchRule,
    },
    PutFoundCardsOntoBattlefieldTapped {
        destination_zone: ObjectZone,
        enters_tapped: bool,
    },
    ShuffleEachSearchingPlayersLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExileCompensationCastProgram {
    pub source: SourceRequirement,
    pub cast: CastChoice,
    pub group_search_results_by_controller: bool,
    pub preserve_exiled_creature_controller_for_search: bool,
    pub ordered_resolution: Vec<CompensationResolutionStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterResolutionStep {
    CounterSelectedNoncreatureSpell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConditionalFreeCounterProgram {
    pub source: SourceRequirement,
    pub cast: CastChoice,
    pub ordered_resolution: Vec<CounterResolutionStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraDuration {
    WhileSourceRemainsAttached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrantedKeyword {
    Vigilance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuraStaticGrant {
    pub target: TargetScope,
    pub power: i16,
    pub toughness: i16,
    pub keywords: Vec<GrantedKeyword>,
    pub duration: AuraDuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EscapeAuraProgram {
    pub source: SourceRequirement,
    pub cast: CastChoice,
    pub aura_target_clause_index: u16,
    pub static_grant_clause_index: u16,
    pub static_grant: AuraStaticGrant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AlternativeCastRuntimeProgram {
    Bounce(BounceCastProgram),
    ExileWithControllerCompensation(ExileCompensationCastProgram),
    ConditionalFreeCounter(ConditionalFreeCounterProgram),
    EscapeAura(EscapeAuraProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledAlternativeCast {
    pub ownership: RootOwnership,
    pub program: AlternativeCastRuntimeProgram,
}

pub(crate) fn compile_alternative_cast_runtime(
    input: AlternativeCastCardInput<'_>,
) -> Option<CompiledAlternativeCast> {
    if input.layout != "normal" {
        return None;
    }
    let printed_cost = parse_mana_cost(input.mana_cost)?;
    let clauses = normalize_oracle_root(input.oracle_text)?;

    compile_cyclonic_rift(input.type_line, &printed_cost, &clauses)
        .or_else(|| compile_winds_of_abandon(input.type_line, &printed_cost, &clauses))
        .or_else(|| compile_fierce_guardianship(input.type_line, &printed_cost, &clauses))
        .or_else(|| compile_sentinels_eyes(input.type_line, &printed_cost, &clauses))
}

fn compile_cyclonic_rift(
    type_line: &str,
    printed_cost: &ManaCost,
    clauses: &[String],
) -> Option<CompiledAlternativeCast> {
    let expected_printed = mana_cost(&[ManaSymbol::Generic(1), ManaSymbol::Blue]);
    if !source_type_matches(type_line, &[CardType::Instant], &[])
        || printed_cost != &expected_printed
        || clauses
            != [
                "return target nonland permanent you don't control to its owner's hand",
                "overload {6}{u} (you may cast this spell for its overload cost. if you do, change \"target\" in its text to \"each.\")",
            ]
    {
        return None;
    }
    let printed_target = TargetScope::NonlandPermanentYouDoNotControl;
    let overloaded_set = AffectedSet::EachNonlandPermanentYouDoNotControl;
    Some(CompiledAlternativeCast {
        ownership: RootOwnership {
            complete_root_required: true,
            clause_count: 2,
            clauses: vec![
                ClauseOwnership {
                    clause_index: 0,
                    kind: ClauseOwnershipKind::Clause(ClauseRole::Resolution),
                },
                ClauseOwnership {
                    clause_index: 1,
                    kind: ClauseOwnershipKind::KeywordAndAlternativeCost(KeywordKind::Overload),
                },
            ],
        },
        program: AlternativeCastRuntimeProgram::Bounce(BounceCastProgram {
            source: source_requirement(&[CardType::Instant], &[]),
            cast: CastChoice {
                printed_cost: expected_printed.clone(),
                options: vec![
                    printed_cast_option(expected_printed, Selection::OneTarget(printed_target)),
                    CastOption {
                        kind: CastOptionKind::Overload,
                        source_zone: ObjectZone::Hand,
                        destination_zone: ObjectZone::Stack,
                        mana_cost: Some(mana_cost(&[ManaSymbol::Generic(6), ManaSymbol::Blue])),
                        waives_printed_mana_cost: true,
                        condition: None,
                        additional_costs: Vec::new(),
                        selection: Selection::Each(overloaded_set),
                        target_transformation: Some(
                            TargetSetTransformation::ReplaceTargetWithEach {
                                from: printed_target,
                                to: overloaded_set,
                            },
                        ),
                    },
                ],
                choose_exactly_one: true,
            },
            ordered_resolution: vec![
                BounceResolutionStep::ReturnSelectedPermanentsToOwnersHands {
                    from: ObjectZone::Battlefield,
                    to: ObjectZone::Hand,
                },
            ],
            destination_controller: ControllerScope::Owner,
        }),
    })
}

fn compile_winds_of_abandon(
    type_line: &str,
    printed_cost: &ManaCost,
    clauses: &[String],
) -> Option<CompiledAlternativeCast> {
    let expected_printed = mana_cost(&[ManaSymbol::Generic(1), ManaSymbol::White]);
    if !source_type_matches(type_line, &[CardType::Sorcery], &[])
        || printed_cost != &expected_printed
        || clauses
            != [
                "exile target creature you don't control. for each creature exiled this way, its controller searches their library for a basic land card. those players put those cards onto the battlefield tapped, then shuffle",
                "overload {4}{w}{w} (you may cast this spell for its overload cost. if you do, change \"target\" in its text to \"each.\")",
            ]
    {
        return None;
    }
    let printed_target = TargetScope::CreatureYouDoNotControl;
    let overloaded_set = AffectedSet::EachCreatureYouDoNotControl;
    Some(CompiledAlternativeCast {
        ownership: RootOwnership {
            complete_root_required: true,
            clause_count: 2,
            clauses: vec![
                ClauseOwnership {
                    clause_index: 0,
                    kind: ClauseOwnershipKind::Clause(ClauseRole::Resolution),
                },
                ClauseOwnership {
                    clause_index: 1,
                    kind: ClauseOwnershipKind::KeywordAndAlternativeCost(KeywordKind::Overload),
                },
            ],
        },
        program: AlternativeCastRuntimeProgram::ExileWithControllerCompensation(
            ExileCompensationCastProgram {
                source: source_requirement(&[CardType::Sorcery], &[]),
                cast: CastChoice {
                    printed_cost: expected_printed.clone(),
                    options: vec![
                        printed_cast_option(expected_printed, Selection::OneTarget(printed_target)),
                        CastOption {
                            kind: CastOptionKind::Overload,
                            source_zone: ObjectZone::Hand,
                            destination_zone: ObjectZone::Stack,
                            mana_cost: Some(mana_cost(&[
                                ManaSymbol::Generic(4),
                                ManaSymbol::White,
                                ManaSymbol::White,
                            ])),
                            waives_printed_mana_cost: true,
                            condition: None,
                            additional_costs: Vec::new(),
                            selection: Selection::Each(overloaded_set),
                            target_transformation: Some(
                                TargetSetTransformation::ReplaceTargetWithEach {
                                    from: printed_target,
                                    to: overloaded_set,
                                },
                            ),
                        },
                    ],
                    choose_exactly_one: true,
                },
                group_search_results_by_controller: true,
                preserve_exiled_creature_controller_for_search: true,
                ordered_resolution: vec![
                    CompensationResolutionStep::ExileSelectedCreatures {
                        from: ObjectZone::Battlefield,
                        to: ObjectZone::Exile,
                    },
                    CompensationResolutionStep::SearchOncePerCreatureActuallyExiled {
                        searching_player: ControllerScope::ControllerOfEachExiledCreature,
                        source_zone: ObjectZone::Library,
                        card: SearchCardPredicate::BasicLand,
                        hidden_zone_rule:
                            HiddenZoneSearchRule::MandatorySearchMayFailToFindQualifiedCard,
                    },
                    CompensationResolutionStep::PutFoundCardsOntoBattlefieldTapped {
                        destination_zone: ObjectZone::Battlefield,
                        enters_tapped: true,
                    },
                    CompensationResolutionStep::ShuffleEachSearchingPlayersLibrary,
                ],
            },
        ),
    })
}

fn compile_fierce_guardianship(
    type_line: &str,
    printed_cost: &ManaCost,
    clauses: &[String],
) -> Option<CompiledAlternativeCast> {
    let expected_printed = mana_cost(&[ManaSymbol::Generic(2), ManaSymbol::Blue]);
    if !source_type_matches(type_line, &[CardType::Instant], &[])
        || printed_cost != &expected_printed
        || clauses
            != [
                "if you control a commander, you may cast this spell without paying its mana cost",
                "counter target noncreature spell",
            ]
    {
        return None;
    }
    let target = Selection::OneTarget(TargetScope::NoncreatureSpell);
    Some(CompiledAlternativeCast {
        ownership: RootOwnership {
            complete_root_required: true,
            clause_count: 2,
            clauses: vec![
                ClauseOwnership {
                    clause_index: 0,
                    kind: ClauseOwnershipKind::Clause(ClauseRole::AlternativeCostCondition),
                },
                ClauseOwnership {
                    clause_index: 1,
                    kind: ClauseOwnershipKind::Clause(ClauseRole::Resolution),
                },
            ],
        },
        program: AlternativeCastRuntimeProgram::ConditionalFreeCounter(
            ConditionalFreeCounterProgram {
                source: source_requirement(&[CardType::Instant], &[]),
                cast: CastChoice {
                    printed_cost: expected_printed.clone(),
                    options: vec![
                        printed_cast_option(expected_printed, target),
                        CastOption {
                            kind: CastOptionKind::CommanderControlledFree,
                            source_zone: ObjectZone::Hand,
                            destination_zone: ObjectZone::Stack,
                            mana_cost: None,
                            waives_printed_mana_cost: true,
                            condition: Some(
                                CastCondition::CommanderPermanentYouControlOnBattlefield,
                            ),
                            additional_costs: Vec::new(),
                            selection: target,
                            target_transformation: None,
                        },
                    ],
                    choose_exactly_one: true,
                },
                ordered_resolution: vec![CounterResolutionStep::CounterSelectedNoncreatureSpell],
            },
        ),
    })
}

fn compile_sentinels_eyes(
    type_line: &str,
    printed_cost: &ManaCost,
    clauses: &[String],
) -> Option<CompiledAlternativeCast> {
    let expected_printed = mana_cost(&[ManaSymbol::White]);
    if !source_type_matches(type_line, &[CardType::Enchantment], &[CardSubtype::Aura])
        || printed_cost != &expected_printed
        || clauses
            != [
                "enchant creature",
                "enchanted creature gets +1/+1 and has vigilance",
                "escape-{w}, exile two other cards from your graveyard. (you may cast this card from your graveyard for its escape cost.)",
            ]
    {
        return None;
    }
    let target = Selection::OneTarget(TargetScope::Creature);
    Some(CompiledAlternativeCast {
        ownership: RootOwnership {
            complete_root_required: true,
            clause_count: 3,
            clauses: vec![
                ClauseOwnership {
                    clause_index: 0,
                    kind: ClauseOwnershipKind::Clause(ClauseRole::AuraTarget),
                },
                ClauseOwnership {
                    clause_index: 1,
                    kind: ClauseOwnershipKind::Clause(ClauseRole::StaticGrant),
                },
                ClauseOwnership {
                    clause_index: 2,
                    kind: ClauseOwnershipKind::KeywordAndAlternativeCost(KeywordKind::Escape),
                },
            ],
        },
        program: AlternativeCastRuntimeProgram::EscapeAura(EscapeAuraProgram {
            source: source_requirement(&[CardType::Enchantment], &[CardSubtype::Aura]),
            cast: CastChoice {
                printed_cost: expected_printed.clone(),
                options: vec![
                    printed_cast_option(expected_printed.clone(), target),
                    CastOption {
                        kind: CastOptionKind::Escape,
                        source_zone: ObjectZone::Graveyard,
                        destination_zone: ObjectZone::Stack,
                        mana_cost: Some(expected_printed),
                        waives_printed_mana_cost: true,
                        condition: None,
                        additional_costs: vec![AdditionalCastCost::ExileOtherGraveyardCards {
                            count: 2,
                            source_zone: ObjectZone::Graveyard,
                            destination_zone: ObjectZone::Exile,
                            exclude_casting_card: true,
                            require_distinct_physical_objects: true,
                        }],
                        selection: target,
                        target_transformation: None,
                    },
                ],
                choose_exactly_one: true,
            },
            aura_target_clause_index: 0,
            static_grant_clause_index: 1,
            static_grant: AuraStaticGrant {
                target: TargetScope::Creature,
                power: 1,
                toughness: 1,
                keywords: vec![GrantedKeyword::Vigilance],
                duration: AuraDuration::WhileSourceRemainsAttached,
            },
        }),
    })
}

fn printed_cast_option(cost: ManaCost, selection: Selection) -> CastOption {
    CastOption {
        kind: CastOptionKind::Printed,
        source_zone: ObjectZone::Hand,
        destination_zone: ObjectZone::Stack,
        mana_cost: Some(cost),
        waives_printed_mana_cost: false,
        condition: None,
        additional_costs: Vec::new(),
        selection,
        target_transformation: None,
    }
}

fn mana_cost(symbols: &[ManaSymbol]) -> ManaCost {
    ManaCost {
        symbols: symbols.to_vec(),
    }
}

fn parse_mana_cost(value: &str) -> Option<ManaCost> {
    let bytes = value.trim().as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut symbols = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'{' {
            return None;
        }
        let relative_end = bytes[cursor + 1..].iter().position(|byte| *byte == b'}')?;
        let end = cursor + 1 + relative_end;
        let symbol = std::str::from_utf8(&bytes[cursor + 1..end]).ok()?;
        let parsed = match symbol {
            "U" | "u" => ManaSymbol::Blue,
            "W" | "w" => ManaSymbol::White,
            _ => ManaSymbol::Generic(symbol.parse::<u16>().ok()?),
        };
        symbols.push(parsed);
        cursor = end + 1;
    }
    Some(ManaCost { symbols })
}

fn source_requirement(
    card_types: &[CardType],
    required_subtypes: &[CardSubtype],
) -> SourceRequirement {
    SourceRequirement {
        card_types: card_types.to_vec(),
        required_subtypes: required_subtypes.to_vec(),
    }
}

fn source_type_matches(
    type_line: &str,
    card_types: &[CardType],
    required_subtypes: &[CardSubtype],
) -> bool {
    card_types
        .iter()
        .copied()
        .all(|card_type| has_type_line_word(type_line, card_type_word(card_type)))
        && required_subtypes
            .iter()
            .copied()
            .all(|subtype| has_type_line_word(type_line, subtype_word(subtype)))
}

fn card_type_word(card_type: CardType) -> &'static str {
    match card_type {
        CardType::Enchantment => "enchantment",
        CardType::Instant => "instant",
        CardType::Sorcery => "sorcery",
    }
}

fn subtype_word(subtype: CardSubtype) -> &'static str {
    match subtype {
        CardSubtype::Aura => "aura",
    }
}

fn has_type_line_word(type_line: &str, expected: &str) -> bool {
    type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn normalize_oracle_root(oracle_text: &str) -> Option<Vec<String>> {
    let normalized_text = oracle_text
        .trim()
        .replace('’', "'")
        .replace(['“', '”'], "\"")
        .replace(['\u{2013}', '\u{2014}'], "-");
    let mut clauses = Vec::new();
    for raw_clause in normalized_text.lines() {
        let collapsed = raw_clause
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        if collapsed.is_empty() {
            continue;
        }
        let clause = collapsed.strip_suffix('.').unwrap_or(&collapsed).trim();
        if clause.is_empty() {
            return None;
        }
        clauses.push(clause.to_string());
    }
    (!clauses.is_empty()).then_some(clauses)
}
