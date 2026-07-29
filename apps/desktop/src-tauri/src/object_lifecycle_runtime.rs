//! Exact object lifecycle and replacement programs for reviewed Alela cards.
//!
//! Programs are selected from complete Oracle structure and source types.
//! Card names are not classifier inputs. Links use physical card or permanent
//! instances so a later object with the same card identity cannot inherit an
//! earlier delayed effect.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectLifecycleCardInput<'a> {
    pub type_line: &'a str,
    pub oracle_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceZone {
    Battlefield,
    Stack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectZone {
    Battlefield,
    Exile,
    Graveyard,
    Hand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardType {
    Creature,
    Enchantment,
    Instant,
    Sorcery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardSubtype {
    Angel,
    Arcane,
    Aura,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceRequirement {
    pub zone: SourceZone,
    pub card_types: Vec<CardType>,
    pub required_subtypes: Vec<CardSubtype>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerScope {
    You,
    Owner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OracleOwnership {
    CompleteRoot { clause_count: u16 },
    ExactClauseSet { clause_indices: Vec<u16> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetPredicate {
    NonlandPermanentAnOpponentControls,
    Creature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkedIdentity {
    CardExiledByThisResolution,
    CardExiledByThisSourceInstance,
    SourceCardThatTriggeredFromDeath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleEvent {
    SourceEntersBattlefield,
    SourceLeavesBattlefield,
    BeginningOfNextEndStep,
    SourceDies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReturnMechanism {
    ImmediateWithoutStack,
    DelayedTriggeredAbility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkedExileProgram {
    pub source: SourceRequirement,
    pub trigger: LifecycleEvent,
    pub target: TargetPredicate,
    pub exile_from: ObjectZone,
    pub exile_to: ObjectZone,
    pub identity: LinkedIdentity,
    pub return_event: LifecycleEvent,
    pub return_from: ObjectZone,
    pub return_to: ObjectZone,
    pub return_controller: ControllerScope,
    pub return_mechanism: ReturnMechanism,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterKind {
    PlusOnePlusOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DelayedExileReturnStep {
    ExileTargetCreature {
        from: ObjectZone,
        to: ObjectZone,
    },
    ScheduleReturn {
        event: LifecycleEvent,
        identity: LinkedIdentity,
    },
    ReturnLinkedCardWithCounter {
        from: ObjectZone,
        to: ObjectZone,
        controller: ControllerScope,
        counter: CounterKind,
        count: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DelayedExileReturnProgram {
    pub source: SourceRequirement,
    pub target: TargetPredicate,
    pub return_mechanism: ReturnMechanism,
    pub ordered_steps: Vec<DelayedExileReturnStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeathCondition {
    SourceWasCreatureWhenItDied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConditionEvidence {
    LastKnownInformationAtDeath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeChangeDuration {
    WhileReturnedObjectRemainsOnBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelfReturnStep {
    ReturnTriggeredSourceCard {
        identity: LinkedIdentity,
        from: ObjectZone,
        to: ObjectZone,
        controller: ControllerScope,
    },
    SetReturnedCardTypes {
        card_types: &'static [CardType],
        duration: TypeChangeDuration,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConditionalSelfReturnProgram {
    pub source: SourceRequirement,
    pub trigger: LifecycleEvent,
    pub condition: DeathCondition,
    pub condition_evidence: ConditionEvidence,
    pub ordered_steps: Vec<SelfReturnStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Color {
    White,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keyword {
    Flying,
    Vigilance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatureTokenDefinition {
    pub card_types: Vec<CardType>,
    pub subtypes: Vec<CardSubtype>,
    pub colors: Vec<Color>,
    pub power: i16,
    pub toughness: i16,
    pub keywords: Vec<Keyword>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplacementEvent {
    OneOrMoreCreatureTokensWouldBeCreated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatureTokenReplacementProgram {
    pub source: SourceRequirement,
    pub event: ReplacementEvent,
    pub token_controller: ControllerScope,
    pub preserve_original_count: bool,
    pub replacement: CreatureTokenDefinition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryTriggerSubject {
    EachCreatureYouControlEntering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerMultiplicity {
    OncePerEnteringCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterRecipient {
    EachCreatureYouControlAtResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatureEntryCounterProgram {
    pub source: SourceRequirement,
    pub trigger: EntryTriggerSubject,
    pub multiplicity: TriggerMultiplicity,
    pub recipient: CounterRecipient,
    pub counter: CounterKind,
    pub count_per_trigger: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalChoice {
    OneOrBoth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraveyardCardPredicate {
    ArtifactCard,
    CreatureCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraveyardReturnMode {
    pub target_owner: ControllerScope,
    pub from: ObjectZone,
    pub card: GraveyardCardPredicate,
    pub to: ObjectZone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModalGraveyardReturnProgram {
    pub source: SourceRequirement,
    pub choice: ModalChoice,
    pub ordered_modes: Vec<GraveyardReturnMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraTarget {
    Creature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChoiceTiming {
    AsSourceEntersBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoredChoice {
    AnyLandType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoredChoiceIdentity {
    SourcePermanentInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuraChoiceStep {
    ChooseAndStore {
        timing: ChoiceTiming,
        choice: StoredChoice,
        identity: StoredChoiceIdentity,
    },
    QueueEntryDraw {
        player: ControllerScope,
        cards: u16,
    },
    GrantChosenTypeLandwalkToEnchantedCreature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuraChoiceLifecycleProgram {
    pub source: SourceRequirement,
    pub legal_target: AuraTarget,
    pub ordered_steps: Vec<AuraChoiceStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObjectLifecycleProgram {
    LinkedExile(LinkedExileProgram),
    DelayedExileReturn(DelayedExileReturnProgram),
    ConditionalSelfReturn(ConditionalSelfReturnProgram),
    CreatureTokenReplacement(CreatureTokenReplacementProgram),
    CreatureEntryCounters(CreatureEntryCounterProgram),
    ModalGraveyardReturn(ModalGraveyardReturnProgram),
    AuraChoiceLifecycle(AuraChoiceLifecycleProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledObjectLifecycle {
    pub ownership: OracleOwnership,
    pub program: ObjectLifecycleProgram,
}

pub(crate) fn compile_object_lifecycle_runtime(
    input: ObjectLifecycleCardInput<'_>,
) -> Option<CompiledObjectLifecycle> {
    let clauses = normalize_oracle_root(input.oracle_text)?;

    compile_banishing_light(input.type_line, &clauses)
        .or_else(|| compile_otherworldly_journey(input.type_line, &clauses))
        .or_else(|| compile_enduring_curiosity(input.type_line, &clauses))
        .or_else(|| compile_divine_visitation(input.type_line, &clauses))
        .or_else(|| compile_cathars_crusade(input.type_line, &clauses))
        .or_else(|| compile_fortuitous_find(input.type_line, &clauses))
        .or_else(|| compile_travelers_cloak(input.type_line, &clauses))
}

fn compile_banishing_light(type_line: &str, clauses: &[String]) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Enchantment], &[])
        || clauses
            != [
                "when this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield",
            ]
    {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: complete_root(clauses),
        program: ObjectLifecycleProgram::LinkedExile(LinkedExileProgram {
            source: battlefield_source(&[CardType::Enchantment], &[]),
            trigger: LifecycleEvent::SourceEntersBattlefield,
            target: TargetPredicate::NonlandPermanentAnOpponentControls,
            exile_from: ObjectZone::Battlefield,
            exile_to: ObjectZone::Exile,
            identity: LinkedIdentity::CardExiledByThisSourceInstance,
            return_event: LifecycleEvent::SourceLeavesBattlefield,
            return_from: ObjectZone::Exile,
            return_to: ObjectZone::Battlefield,
            return_controller: ControllerScope::Owner,
            return_mechanism: ReturnMechanism::ImmediateWithoutStack,
        }),
    })
}

fn compile_otherworldly_journey(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Instant], &[CardSubtype::Arcane])
        || clauses
            != [
                "exile target creature. at the beginning of the next end step, return that card to the battlefield under its owner's control with a +1/+1 counter on it",
            ]
    {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: complete_root(clauses),
        program: ObjectLifecycleProgram::DelayedExileReturn(DelayedExileReturnProgram {
            source: stack_source(&[CardType::Instant], &[CardSubtype::Arcane]),
            target: TargetPredicate::Creature,
            return_mechanism: ReturnMechanism::DelayedTriggeredAbility,
            ordered_steps: vec![
                DelayedExileReturnStep::ExileTargetCreature {
                    from: ObjectZone::Battlefield,
                    to: ObjectZone::Exile,
                },
                DelayedExileReturnStep::ScheduleReturn {
                    event: LifecycleEvent::BeginningOfNextEndStep,
                    identity: LinkedIdentity::CardExiledByThisResolution,
                },
                DelayedExileReturnStep::ReturnLinkedCardWithCounter {
                    from: ObjectZone::Exile,
                    to: ObjectZone::Battlefield,
                    controller: ControllerScope::Owner,
                    counter: CounterKind::PlusOnePlusOne,
                    count: 1,
                },
            ],
        }),
    })
}

fn compile_enduring_curiosity(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Enchantment, CardType::Creature], &[])
        || clauses.len() != 3
        || clauses[0] != "flash"
        || clauses[1]
            != "whenever a creature you control deals combat damage to a player, draw a card"
        || !is_conditional_named_self_return_clause(&clauses[2])
    {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: OracleOwnership::ExactClauseSet {
            clause_indices: vec![2],
        },
        program: ObjectLifecycleProgram::ConditionalSelfReturn(ConditionalSelfReturnProgram {
            source: battlefield_source(&[CardType::Enchantment, CardType::Creature], &[]),
            trigger: LifecycleEvent::SourceDies,
            condition: DeathCondition::SourceWasCreatureWhenItDied,
            condition_evidence: ConditionEvidence::LastKnownInformationAtDeath,
            ordered_steps: vec![
                SelfReturnStep::ReturnTriggeredSourceCard {
                    identity: LinkedIdentity::SourceCardThatTriggeredFromDeath,
                    from: ObjectZone::Graveyard,
                    to: ObjectZone::Battlefield,
                    controller: ControllerScope::Owner,
                },
                SelfReturnStep::SetReturnedCardTypes {
                    card_types: &[CardType::Enchantment],
                    duration: TypeChangeDuration::WhileReturnedObjectRemainsOnBattlefield,
                },
            ],
        }),
    })
}

fn compile_divine_visitation(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Enchantment], &[])
        || clauses
            != [
                "if one or more creature tokens would be created under your control, that many 4/4 white angel creature tokens with flying and vigilance are created instead",
            ]
    {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: complete_root(clauses),
        program: ObjectLifecycleProgram::CreatureTokenReplacement(
            CreatureTokenReplacementProgram {
                source: battlefield_source(&[CardType::Enchantment], &[]),
                event: ReplacementEvent::OneOrMoreCreatureTokensWouldBeCreated,
                token_controller: ControllerScope::You,
                preserve_original_count: true,
                replacement: CreatureTokenDefinition {
                    card_types: vec![CardType::Creature],
                    subtypes: vec![CardSubtype::Angel],
                    colors: vec![Color::White],
                    power: 4,
                    toughness: 4,
                    keywords: vec![Keyword::Flying, Keyword::Vigilance],
                },
            },
        ),
    })
}

fn compile_cathars_crusade(type_line: &str, clauses: &[String]) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Enchantment], &[])
        || clauses
            != [
                "whenever a creature you control enters, put a +1/+1 counter on each creature you control",
            ]
    {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: complete_root(clauses),
        program: ObjectLifecycleProgram::CreatureEntryCounters(CreatureEntryCounterProgram {
            source: battlefield_source(&[CardType::Enchantment], &[]),
            trigger: EntryTriggerSubject::EachCreatureYouControlEntering,
            multiplicity: TriggerMultiplicity::OncePerEnteringCreature,
            recipient: CounterRecipient::EachCreatureYouControlAtResolution,
            counter: CounterKind::PlusOnePlusOne,
            count_per_trigger: 1,
        }),
    })
}

fn compile_fortuitous_find(type_line: &str, clauses: &[String]) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Sorcery], &[])
        || clauses
            != [
                "choose one or both -",
                "return target artifact card from your graveyard to your hand",
                "return target creature card from your graveyard to your hand",
            ]
    {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: complete_root(clauses),
        program: ObjectLifecycleProgram::ModalGraveyardReturn(ModalGraveyardReturnProgram {
            source: stack_source(&[CardType::Sorcery], &[]),
            choice: ModalChoice::OneOrBoth,
            ordered_modes: vec![
                GraveyardReturnMode {
                    target_owner: ControllerScope::You,
                    from: ObjectZone::Graveyard,
                    card: GraveyardCardPredicate::ArtifactCard,
                    to: ObjectZone::Hand,
                },
                GraveyardReturnMode {
                    target_owner: ControllerScope::You,
                    from: ObjectZone::Graveyard,
                    card: GraveyardCardPredicate::CreatureCard,
                    to: ObjectZone::Hand,
                },
            ],
        }),
    })
}

fn compile_travelers_cloak(type_line: &str, clauses: &[String]) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Enchantment], &[CardSubtype::Aura])
        || clauses
            != [
                "enchant creature",
                "as this aura enters, choose a land type",
                "when this aura enters, draw a card",
                "enchanted creature has landwalk of the chosen type",
            ]
    {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: complete_root(clauses),
        program: ObjectLifecycleProgram::AuraChoiceLifecycle(AuraChoiceLifecycleProgram {
            source: battlefield_source(&[CardType::Enchantment], &[CardSubtype::Aura]),
            legal_target: AuraTarget::Creature,
            ordered_steps: vec![
                AuraChoiceStep::ChooseAndStore {
                    timing: ChoiceTiming::AsSourceEntersBattlefield,
                    choice: StoredChoice::AnyLandType,
                    identity: StoredChoiceIdentity::SourcePermanentInstance,
                },
                AuraChoiceStep::QueueEntryDraw {
                    player: ControllerScope::You,
                    cards: 1,
                },
                AuraChoiceStep::GrantChosenTypeLandwalkToEnchantedCreature,
            ],
        }),
    })
}

fn complete_root(clauses: &[String]) -> OracleOwnership {
    OracleOwnership::CompleteRoot {
        clause_count: u16::try_from(clauses.len()).expect("reviewed roots fit in u16"),
    }
}

fn battlefield_source(
    card_types: &[CardType],
    required_subtypes: &[CardSubtype],
) -> SourceRequirement {
    SourceRequirement {
        zone: SourceZone::Battlefield,
        card_types: card_types.to_vec(),
        required_subtypes: required_subtypes.to_vec(),
    }
}

fn stack_source(card_types: &[CardType], required_subtypes: &[CardSubtype]) -> SourceRequirement {
    SourceRequirement {
        zone: SourceZone::Stack,
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
        CardType::Creature => "creature",
        CardType::Enchantment => "enchantment",
        CardType::Instant => "instant",
        CardType::Sorcery => "sorcery",
    }
}

fn subtype_word(subtype: CardSubtype) -> &'static str {
    match subtype {
        CardSubtype::Angel => "angel",
        CardSubtype::Arcane => "arcane",
        CardSubtype::Aura => "aura",
    }
}

fn has_type_line_word(type_line: &str, expected: &str) -> bool {
    type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn is_conditional_named_self_return_clause(clause: &str) -> bool {
    let Some(without_when) = clause.strip_prefix("when ") else {
        return false;
    };
    let suffix = " dies, if it was a creature, return it to the battlefield under its owner's control. it's an enchantment";
    let Some(reference) = without_when.strip_suffix(suffix) else {
        return false;
    };
    is_plausible_named_reference(reference)
}

fn is_plausible_named_reference(reference: &str) -> bool {
    let words = reference.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() || words.len() > 8 {
        return false;
    }
    const NON_NAME_STARTS: &[&str] = &[
        "a",
        "all",
        "an",
        "any",
        "each",
        "it",
        "permanent",
        "permanents",
        "source",
        "target",
        "that",
        "the",
        "them",
        "this",
        "those",
        "you",
        "your",
    ];
    if NON_NAME_STARTS.contains(&words[0]) {
        return false;
    }
    reference.chars().all(|character| {
        character.is_alphanumeric()
            || character.is_whitespace()
            || matches!(character, '\'' | ',' | '-' | ':')
    })
}

fn normalize_oracle_root(oracle_text: &str) -> Option<Vec<String>> {
    let normalized_text = oracle_text
        .trim()
        .replace('’', "'")
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
        let without_bullet = collapsed
            .strip_prefix('\u{2022}')
            .unwrap_or(&collapsed)
            .trim();
        let without_reminders = remove_reviewed_reminder_text(without_bullet);
        let clause = without_reminders
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let clause = clause.strip_suffix('.').unwrap_or(&clause).trim();
        if clause.is_empty() {
            return None;
        }
        clauses.push(clause.to_string());
    }
    (!clauses.is_empty()).then_some(clauses)
}

fn remove_reviewed_reminder_text(value: &str) -> String {
    const REMINDERS: &[&str] = &[
        " (it's not a creature.)",
        " (it can't be blocked as long as defending player controls a land of that type.)",
    ];
    REMINDERS.iter().fold(value.to_string(), |text, reminder| {
        text.replace(reminder, "")
    })
}
