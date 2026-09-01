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
    AnotherCreature,
    GreenOrWhiteCreatureAnOpponentControls,
    ArtifactOrEnchantment,
    Land,
    LandYouControl,
    PermanentYouControlOtherThanSource,
    CardFromSingleGraveyard,
    CardInYourHand,
    LandPermanent,
    OtherPermanentYouControl,
    NonlandCardInTargetOpponentsHand,
    CardInTargetOpponentsHand,
    OtherCreatureFromBattlefieldOrGraveyard,
    ControlledPermanentOrOwnHandCard,
    CreatureYouControlOtherThanSource,
    AnotherNonlandPermanent,
    AnotherCreatureWithShadow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkedExileSelection {
    Targeted,
    ChosenBySourceController,
    ChosenByOpponent,
    ChosenFromTargetOpponentsRevealedHand,
    AllMatching,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkedExileQuantity {
    One,
    Exactly(u16),
    UpTo(u16),
    UpToX,
    All,
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
    SourceTurnedFaceUp,
    SourceCastAdditionalCost,
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
    pub exile_clause_index: u16,
    pub return_clause_index: u16,
    pub trigger: LifecycleEvent,
    pub target: TargetPredicate,
    pub selection: LinkedExileSelection,
    pub quantity: LinkedExileQuantity,
    pub exile_optional: bool,
    pub exile_from: Vec<ObjectZone>,
    pub exile_to: ObjectZone,
    pub identity: LinkedIdentity,
    pub return_event: LifecycleEvent,
    pub return_from: ObjectZone,
    pub return_to: ObjectZone,
    pub return_controller: ControllerScope,
    pub return_tapped: bool,
    pub requires_cast_from_hand: bool,
    pub required_exile_subtype: Option<String>,
    pub required_exile_alternative_subtype: Option<String>,
    pub sacrifice_source_if_no_exile: bool,
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
pub struct CompiledObjectLifecycle {
    pub(crate) ownership: OracleOwnership,
    pub(crate) program: ObjectLifecycleProgram,
}

impl CompiledObjectLifecycle {
    pub(crate) fn owns_clause(&self, clause_index: u16) -> bool {
        match &self.ownership {
            OracleOwnership::CompleteRoot { clause_count } => clause_index < *clause_count,
            OracleOwnership::ExactClauseSet { clause_indices } => {
                clause_indices.contains(&clause_index)
            }
        }
    }
}

pub(crate) fn compile_object_lifecycle_runtime(
    input: ObjectLifecycleCardInput<'_>,
) -> Option<CompiledObjectLifecycle> {
    let clauses = normalize_oracle_root(input.oracle_text)?;

    compile_banishing_light(input.type_line, &clauses)
        .or_else(|| compile_behold_linked_exile(input.type_line, &clauses))
        .or_else(|| compile_wormfang_drake(input.type_line, &clauses))
        .or_else(|| compile_champion_lifecycle(input.type_line, &clauses))
        .or_else(|| compile_separate_linked_exile(input.type_line, &clauses))
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
            exile_clause_index: 0,
            return_clause_index: 0,
            trigger: LifecycleEvent::SourceEntersBattlefield,
            target: TargetPredicate::NonlandPermanentAnOpponentControls,
            selection: LinkedExileSelection::Targeted,
            quantity: LinkedExileQuantity::One,
            exile_optional: false,
            exile_from: vec![ObjectZone::Battlefield],
            exile_to: ObjectZone::Exile,
            identity: LinkedIdentity::CardExiledByThisSourceInstance,
            return_event: LifecycleEvent::SourceLeavesBattlefield,
            return_from: ObjectZone::Exile,
            return_to: ObjectZone::Battlefield,
            return_controller: ControllerScope::Owner,
            return_tapped: false,
            requires_cast_from_hand: false,
            required_exile_subtype: None,
            required_exile_alternative_subtype: None,
            sacrifice_source_if_no_exile: false,
            return_mechanism: ReturnMechanism::ImmediateWithoutStack,
        }),
    })
}

fn compile_behold_linked_exile(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Creature], &[]) {
        return None;
    }
    let mut cost_matches = clauses.iter().enumerate().filter_map(|(index, clause)| {
        parse_behold_and_exile_subtype(clause).map(|subtype| (index, subtype))
    });
    let (cost_index, subtype) = cost_matches.next()?;
    if cost_matches.next().is_some() {
        return None;
    }
    let exact_return =
        "when this creature leaves the battlefield, return the exiled card to its owner's hand";
    let mut return_matches = clauses
        .iter()
        .enumerate()
        .filter(|(index, clause)| *index != cost_index && clause.as_str() == exact_return);
    let (return_index, _) = return_matches.next()?;
    if return_matches.next().is_some() {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: OracleOwnership::ExactClauseSet {
            clause_indices: vec![cost_index as u16, return_index as u16],
        },
        program: ObjectLifecycleProgram::LinkedExile(LinkedExileProgram {
            source: stack_source(&[CardType::Creature], &[]),
            exile_clause_index: cost_index as u16,
            return_clause_index: return_index as u16,
            trigger: LifecycleEvent::SourceCastAdditionalCost,
            target: TargetPredicate::ControlledPermanentOrOwnHandCard,
            selection: LinkedExileSelection::ChosenBySourceController,
            quantity: LinkedExileQuantity::One,
            exile_optional: false,
            exile_from: vec![ObjectZone::Battlefield, ObjectZone::Hand],
            exile_to: ObjectZone::Exile,
            identity: LinkedIdentity::CardExiledByThisSourceInstance,
            return_event: LifecycleEvent::SourceLeavesBattlefield,
            return_from: ObjectZone::Exile,
            return_to: ObjectZone::Hand,
            return_controller: ControllerScope::Owner,
            return_tapped: false,
            requires_cast_from_hand: false,
            required_exile_subtype: Some(subtype),
            required_exile_alternative_subtype: None,
            sacrifice_source_if_no_exile: false,
            return_mechanism: ReturnMechanism::DelayedTriggeredAbility,
        }),
    })
}

fn compile_wormfang_drake(type_line: &str, clauses: &[String]) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Creature], &[])
        || clauses
            != [
                "flying",
                "when this creature enters, sacrifice it unless you exile a creature you control other than this creature",
                "when this creature leaves the battlefield, return the exiled card to the battlefield under its owner's control",
            ]
    {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: OracleOwnership::ExactClauseSet {
            clause_indices: vec![1, 2],
        },
        program: ObjectLifecycleProgram::LinkedExile(LinkedExileProgram {
            source: battlefield_source(&[CardType::Creature], &[]),
            exile_clause_index: 1,
            return_clause_index: 2,
            trigger: LifecycleEvent::SourceEntersBattlefield,
            target: TargetPredicate::CreatureYouControlOtherThanSource,
            selection: LinkedExileSelection::ChosenBySourceController,
            quantity: LinkedExileQuantity::UpTo(1),
            exile_optional: false,
            exile_from: vec![ObjectZone::Battlefield],
            exile_to: ObjectZone::Exile,
            identity: LinkedIdentity::CardExiledByThisSourceInstance,
            return_event: LifecycleEvent::SourceLeavesBattlefield,
            return_from: ObjectZone::Exile,
            return_to: ObjectZone::Battlefield,
            return_controller: ControllerScope::Owner,
            return_tapped: false,
            requires_cast_from_hand: false,
            required_exile_subtype: None,
            required_exile_alternative_subtype: None,
            sacrifice_source_if_no_exile: true,
            return_mechanism: ReturnMechanism::DelayedTriggeredAbility,
        }),
    })
}

fn compile_champion_lifecycle(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledObjectLifecycle> {
    if !source_type_matches(type_line, &[CardType::Creature], &[])
        || !is_installed_complete_champion_root(clauses)
    {
        return None;
    }
    let mut champion_matches = clauses.iter().enumerate().filter_map(|(index, clause)| {
        parse_exact_champion_quality(clause).map(|quality| (index, quality))
    });
    let (champion_index, (required_subtype, alternative_subtype)) = champion_matches.next()?;
    if champion_matches.next().is_some() {
        return None;
    }
    let champion_index = champion_index as u16;

    Some(CompiledObjectLifecycle {
        ownership: OracleOwnership::ExactClauseSet {
            clause_indices: vec![champion_index],
        },
        program: ObjectLifecycleProgram::LinkedExile(LinkedExileProgram {
            source: battlefield_source(&[CardType::Creature], &[]),
            exile_clause_index: champion_index,
            return_clause_index: champion_index,
            trigger: LifecycleEvent::SourceEntersBattlefield,
            target: TargetPredicate::CreatureYouControlOtherThanSource,
            selection: LinkedExileSelection::ChosenBySourceController,
            quantity: LinkedExileQuantity::UpTo(1),
            exile_optional: false,
            exile_from: vec![ObjectZone::Battlefield],
            exile_to: ObjectZone::Exile,
            identity: LinkedIdentity::CardExiledByThisSourceInstance,
            return_event: LifecycleEvent::SourceLeavesBattlefield,
            return_from: ObjectZone::Exile,
            return_to: ObjectZone::Battlefield,
            return_controller: ControllerScope::Owner,
            return_tapped: false,
            requires_cast_from_hand: false,
            required_exile_subtype: required_subtype,
            required_exile_alternative_subtype: alternative_subtype,
            sacrifice_source_if_no_exile: true,
            return_mechanism: ReturnMechanism::DelayedTriggeredAbility,
        }),
    })
}

fn parse_exact_champion_quality(clause: &str) -> Option<(Option<String>, Option<String>)> {
    let (first, second) = match clause {
        "champion a creature (when this enters, sacrifice it unless you exile another creature you control. when this leaves the battlefield, that card returns to the battlefield.)" =>
        {
            return Some((None, None));
        }
        "champion a faerie (when this enters, sacrifice it unless you exile another faerie you control. when this leaves the battlefield, that card returns to the battlefield.)" => {
            ("faerie", None)
        }
        "champion a goblin (when this enters, sacrifice it unless you exile another goblin you control. when this leaves the battlefield, that card returns to the battlefield.)" => {
            ("goblin", None)
        }
        "champion a goblin or shaman (when this enters, sacrifice it unless you exile another goblin or shaman you control. when this leaves the battlefield, that card returns to the battlefield.)" => {
            ("goblin", Some("shaman"))
        }
        "champion a kithkin (when this enters, sacrifice it unless you exile another kithkin you control. when this leaves the battlefield, that card returns to the battlefield.)" => {
            ("kithkin", None)
        }
        "champion a merfolk (when this enters, sacrifice it unless you exile another merfolk you control. when this leaves the battlefield, that card returns to the battlefield.)" => {
            ("merfolk", None)
        }
        "champion a treefolk or warrior (when this enters, sacrifice it unless you exile another treefolk or warrior you control. when this leaves the battlefield, that card returns to the battlefield.)" => {
            ("treefolk", Some("warrior"))
        }
        "champion an elemental (when this enters, sacrifice it unless you exile another elemental you control. when this leaves the battlefield, that card returns to the battlefield.)" => {
            ("elemental", None)
        }
        "champion an elf (when this creature enters, sacrifice it unless you exile another elf you control. when this creature leaves the battlefield, that card returns to the battlefield.)" => {
            ("elf", None)
        }
        _ => return None,
    };
    Some((Some(first.to_owned()), second.map(str::to_owned)))
}

fn is_installed_complete_champion_root(clauses: &[String]) -> bool {
    let root = clauses.join("\n");
    matches!(
        root.as_str(),
        "champion a goblin (when this enters, sacrifice it unless you exile another goblin you control. when this leaves the battlefield, that card returns to the battlefield.)\nwhenever a goblin you control deals combat damage to a player, you may create a 1/1 black goblin rogue creature token"
            | "changeling (this card is every creature type.)\nhaste\nchampion a creature (when this enters, sacrifice it unless you exile another creature you control. when this leaves the battlefield, that card returns to the battlefield.)"
            | "changeling (this card is every creature type.)\nchampion a creature (when this enters, sacrifice it unless you exile another creature you control. when this leaves the battlefield, that card returns to the battlefield.)\nlifelink (damage dealt by this creature also causes you to gain that much life.)"
            | "changeling (this card is every creature type.)\nchampion a creature (when this enters, sacrifice it unless you exile another creature you control. when this leaves the battlefield, that card returns to the battlefield.)"
            | "champion a goblin or shaman (when this enters, sacrifice it unless you exile another goblin or shaman you control. when this leaves the battlefield, that card returns to the battlefield.)\n{t}: this creature deals 3 damage to any target"
            | "flash\nflying\nchampion a faerie (when this enters, sacrifice it unless you exile another faerie you control. when this leaves the battlefield, that card returns to the battlefield.)\nwhen a faerie is championed with this creature, tap all lands target player controls"
            | "trample\nchampion an elemental (when this enters, sacrifice it unless you exile another elemental you control. when this leaves the battlefield, that card returns to the battlefield.)"
            | "flying\nchampion an elemental (when this enters, sacrifice it unless you exile another elemental you control. when this leaves the battlefield, that card returns to the battlefield.)"
            | "first strike, vigilance\nchampion a kithkin (when this enters, sacrifice it unless you exile another kithkin you control. when this leaves the battlefield, that card returns to the battlefield.)\nthis creature can block any number of creatures"
            | "trample\nchampion a treefolk or warrior (when this enters, sacrifice it unless you exile another treefolk or warrior you control. when this leaves the battlefield, that card returns to the battlefield.)\nwhenever a creature you control becomes blocked, it gets +0/+5 until end of turn"
            | "champion a merfolk (when this enters, sacrifice it unless you exile another merfolk you control. when this leaves the battlefield, that card returns to the battlefield.)\nwhenever this creature deals combat damage to a player, you may sacrifice a merfolk. if you do, take an extra turn after this one"
            | "champion an elf (when this creature enters, sacrifice it unless you exile another elf you control. when this creature leaves the battlefield, that card returns to the battlefield.)\n{2}{g}: create a 2/2 green wolf creature token\nwolves you control have deathtouch"
    )
}

fn parse_behold_and_exile_subtype(clause: &str) -> Option<String> {
    let prefix = "as an additional cost to cast this spell, behold ";
    let body = clause.strip_prefix(prefix)?;
    for article in ["a ", "an "] {
        let Some(article_body) = body.strip_prefix(article) else {
            continue;
        };
        let Some((subtype, _)) = article_body.split_once(" and exile it. (exile ") else {
            continue;
        };
        if subtype.is_empty()
            || !subtype
                .chars()
                .all(|character| character.is_ascii_alphabetic() || character == '-')
        {
            return None;
        }
        let expected = format!(
            "{prefix}{article}{subtype} and exile it. (exile {article}{subtype} you control or {article}{subtype} card from your hand.)"
        );
        if clause == expected {
            return Some(subtype.to_owned());
        }
    }
    None
}

fn compile_separate_linked_exile(
    type_line: &str,
    clauses: &[String],
) -> Option<CompiledObjectLifecycle> {
    let (source_types, source_word) = if source_type_matches(type_line, &[CardType::Creature], &[])
    {
        (vec![CardType::Creature], "creature")
    } else if source_type_matches(type_line, &[CardType::Enchantment], &[]) {
        (vec![CardType::Enchantment], "enchantment")
    } else {
        return None;
    };

    let mut entry_matches = clauses.iter().enumerate().filter_map(|(index, clause)| {
        let (
            target,
            selection,
            quantity,
            optional,
            trigger,
            exile_from,
            requires_cast_from_hand,
            expected_return,
            return_to,
            return_tapped,
        ) = match clause.as_str() {
            "when this creature enters, exile another target creature" => {
                (
                    TargetPredicate::AnotherCreature,
                    LinkedExileSelection::Targeted,
                    LinkedExileQuantity::One,
                    false,
                    LifecycleEvent::SourceEntersBattlefield,
                    vec![ObjectZone::Battlefield],
                    false,
                    "return the exiled card to the battlefield under its owner's control",
                    ObjectZone::Battlefield,
                    false,
                )
            }
            "when this creature enters, you may exile another target creature" => {
                (
                    TargetPredicate::AnotherCreature,
                    LinkedExileSelection::Targeted,
                    LinkedExileQuantity::One,
                    true,
                    LifecycleEvent::SourceEntersBattlefield,
                    vec![ObjectZone::Battlefield],
                    false,
                    "return the exiled card to the battlefield under its owner's control",
                    ObjectZone::Battlefield,
                    false,
                )
            }
            "when this creature enters, exile target green or white creature an opponent controls" => {
                (
                    TargetPredicate::GreenOrWhiteCreatureAnOpponentControls,
                    LinkedExileSelection::Targeted,
                    LinkedExileQuantity::One,
                    false,
                    LifecycleEvent::SourceEntersBattlefield,
                    vec![ObjectZone::Battlefield],
                    false,
                    "return the exiled card to the battlefield under its owner's control",
                    ObjectZone::Battlefield,
                    false,
                )
            }
            "when this creature enters, you may exile target artifact or enchantment" => {
                (
                    TargetPredicate::ArtifactOrEnchantment,
                    LinkedExileSelection::Targeted,
                    LinkedExileQuantity::One,
                    true,
                    LifecycleEvent::SourceEntersBattlefield,
                    vec![ObjectZone::Battlefield],
                    false,
                    "return the exiled card to the battlefield under its owner's control",
                    ObjectZone::Battlefield,
                    false,
                )
            }
            "when this creature enters, exile target land" => (
                TargetPredicate::Land,
                LinkedExileSelection::Targeted,
                LinkedExileQuantity::One,
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Battlefield],
                false,
                "return the exiled card to the battlefield under its owner's control",
                ObjectZone::Battlefield,
                false,
            ),
            "when this enchantment enters, exile target creature" => {
                (
                    TargetPredicate::Creature,
                    LinkedExileSelection::Targeted,
                    LinkedExileQuantity::One,
                    false,
                    LifecycleEvent::SourceEntersBattlefield,
                    vec![ObjectZone::Battlefield],
                    false,
                    "return the exiled card to the battlefield under its owner's control",
                    ObjectZone::Battlefield,
                    false,
                )
            }
            "when this enchantment enters, exile another target nonland permanent" => {
                (
                    TargetPredicate::AnotherNonlandPermanent,
                    LinkedExileSelection::Targeted,
                    LinkedExileQuantity::One,
                    false,
                    LifecycleEvent::SourceEntersBattlefield,
                    vec![ObjectZone::Battlefield],
                    false,
                    "return the exiled card to the battlefield under its owner's control",
                    ObjectZone::Battlefield,
                    false,
                )
            }
            "when this creature enters, exile another target creature with shadow" => {
                (
                    TargetPredicate::AnotherCreatureWithShadow,
                    LinkedExileSelection::Targeted,
                    LinkedExileQuantity::One,
                    false,
                    LifecycleEvent::SourceEntersBattlefield,
                    vec![ObjectZone::Battlefield],
                    false,
                    "return the exiled card to the battlefield under its owner's control",
                    ObjectZone::Battlefield,
                    false,
                )
            }
            "when this creature enters, exile a land you control" => (
                TargetPredicate::LandYouControl,
                LinkedExileSelection::ChosenBySourceController,
                LinkedExileQuantity::One,
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Battlefield],
                false,
                "return the exiled card to the battlefield under its owner's control",
                ObjectZone::Battlefield,
                false,
            ),
            "when this creature enters, an opponent chooses a permanent you control other than this creature and exiles it" => (
                TargetPredicate::PermanentYouControlOtherThanSource,
                LinkedExileSelection::ChosenByOpponent,
                LinkedExileQuantity::One,
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Battlefield],
                false,
                "return the exiled card to the battlefield under its owner's control",
                ObjectZone::Battlefield,
                false,
            ),
            "when this creature enters, exile up to two target cards from a single graveyard" => (
                TargetPredicate::CardFromSingleGraveyard,
                LinkedExileSelection::Targeted,
                LinkedExileQuantity::UpTo(2),
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Graveyard],
                false,
                "return the exiled cards to their owner's graveyard",
                ObjectZone::Graveyard,
                false,
            ),
            "when this creature is turned face up, exile up to x other target creatures from the battlefield and/or creature cards from graveyards" => (
                TargetPredicate::OtherCreatureFromBattlefieldOrGraveyard,
                LinkedExileSelection::Targeted,
                LinkedExileQuantity::UpToX,
                false,
                LifecycleEvent::SourceTurnedFaceUp,
                vec![ObjectZone::Battlefield, ObjectZone::Graveyard],
                false,
                "return the exiled cards to their owners' hands",
                ObjectZone::Hand,
                false,
            ),
            "when this creature enters, target opponent reveals their hand and you choose a nonland card from it. exile that card" => (
                TargetPredicate::NonlandCardInTargetOpponentsHand,
                LinkedExileSelection::ChosenFromTargetOpponentsRevealedHand,
                LinkedExileQuantity::One,
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Hand],
                false,
                "return the exiled card to its owner's hand",
                ObjectZone::Hand,
                false,
            ),
            "when this creature enters, exile all cards from your hand" => (
                TargetPredicate::CardInYourHand,
                LinkedExileSelection::AllMatching,
                LinkedExileQuantity::All,
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Hand],
                false,
                "return the exiled cards to their owner's hand",
                ObjectZone::Hand,
                false,
            ),
            "when this creature enters, exile all lands" => (
                TargetPredicate::LandPermanent,
                LinkedExileSelection::AllMatching,
                LinkedExileQuantity::All,
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Battlefield],
                false,
                "return the exiled cards to the battlefield tapped under their owners' control",
                ObjectZone::Battlefield,
                true,
            ),
            "when this creature enters, exile all other permanents you control" => (
                TargetPredicate::OtherPermanentYouControl,
                LinkedExileSelection::AllMatching,
                LinkedExileQuantity::All,
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Battlefield],
                false,
                "return the exiled cards to the battlefield under their owners' control",
                ObjectZone::Battlefield,
                false,
            ),
            "when this creature enters, if you cast it from your hand, exile all cards from target opponent's hand" => (
                TargetPredicate::CardInTargetOpponentsHand,
                LinkedExileSelection::AllMatching,
                LinkedExileQuantity::All,
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Hand],
                true,
                "return the exiled cards to their owner's hand",
                ObjectZone::Hand,
                false,
            ),
            "when this creature enters, exile two target lands" => (
                TargetPredicate::Land,
                LinkedExileSelection::Targeted,
                LinkedExileQuantity::Exactly(2),
                false,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Battlefield],
                false,
                "return the exiled cards to the battlefield under their owners' control",
                ObjectZone::Battlefield,
                false,
            ),
            "when this creature enters, you may exile up to three other target creatures from the battlefield and/or creature cards from graveyards" => (
                TargetPredicate::OtherCreatureFromBattlefieldOrGraveyard,
                LinkedExileSelection::Targeted,
                LinkedExileQuantity::UpTo(3),
                true,
                LifecycleEvent::SourceEntersBattlefield,
                vec![ObjectZone::Battlefield, ObjectZone::Graveyard],
                false,
                "return the exiled cards to their owners' hands",
                ObjectZone::Hand,
                false,
            ),
            _ => return None,
        };
        Some((
            index,
            target,
            selection,
            quantity,
            optional,
            trigger,
            exile_from,
            requires_cast_from_hand,
            expected_return,
            return_to,
            return_tapped,
        ))
    });
    let (
        entry_index,
        target,
        selection,
        quantity,
        exile_optional,
        trigger,
        exile_from,
        requires_cast_from_hand,
        expected_return,
        return_to,
        return_tapped,
    ) = entry_matches.next()?;
    if entry_matches.next().is_some() {
        return None;
    }
    let exact_return = format!("when this {source_word} leaves the battlefield, {expected_return}");
    let mut return_matches = clauses
        .iter()
        .enumerate()
        .filter(|(index, clause)| *index != entry_index && clause.as_str() == exact_return);
    let (return_index, _) = return_matches.next()?;
    if return_matches.next().is_some() {
        return None;
    }

    Some(CompiledObjectLifecycle {
        ownership: OracleOwnership::ExactClauseSet {
            clause_indices: vec![entry_index as u16, return_index as u16],
        },
        program: ObjectLifecycleProgram::LinkedExile(LinkedExileProgram {
            source: battlefield_source(&source_types, &[]),
            exile_clause_index: entry_index as u16,
            return_clause_index: return_index as u16,
            trigger,
            target,
            selection,
            quantity,
            exile_optional,
            exile_from,
            exile_to: ObjectZone::Exile,
            identity: LinkedIdentity::CardExiledByThisSourceInstance,
            return_event: LifecycleEvent::SourceLeavesBattlefield,
            return_from: ObjectZone::Exile,
            return_to,
            return_controller: ControllerScope::Owner,
            return_tapped,
            requires_cast_from_hand,
            required_exile_subtype: None,
            required_exile_alternative_subtype: None,
            sacrifice_source_if_no_exile: false,
            return_mechanism: ReturnMechanism::DelayedTriggeredAbility,
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
