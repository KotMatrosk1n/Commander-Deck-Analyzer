//! Exact continuous modifier and trigger programs for reviewed Alela cards.
//!
//! Compilation uses card layout, exact face structure, source types, and
//! complete normalized Oracle roots. Card names are not classifier inputs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContinuousTriggerFaceInput<'a> {
    pub type_line: &'a str,
    pub oracle_text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContinuousTriggerCardInput<'a> {
    pub layout: &'a str,
    pub type_line: &'a str,
    pub oracle_text: Option<&'a str>,
    pub faces: &'a [ContinuousTriggerFaceInput<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceZone {
    Battlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardType {
    Artifact,
    Creature,
    Enchantment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardSubtype {
    Aura,
    Equipment,
    Faerie,
    Pegasus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceRequirement {
    pub zone: SourceZone,
    pub face_index: u16,
    pub card_types: Vec<CardType>,
    pub required_subtypes: Vec<CardSubtype>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerScope {
    Any,
    Owner,
    You,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectDuration {
    WhileSourceRemainsOnBattlefield,
    WhileAuraRemainsAttached,
    UntilEndOfTurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OracleOwnership {
    CompleteFaceRoot { clause_count: u16 },
    ExactClauseSet { clause_indices: Vec<u16> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FaceOwnership {
    pub face_index: u16,
    pub oracle: OracleOwnership,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keyword {
    DoubleStrike,
    Flying,
    Lifelink,
    Vigilance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreatureRelation {
    ControlledCreatures,
    EnchantedCreature,
    EquippedCreature,
    SourceCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttachmentQualification {
    EnchantedOrEquipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatureSelector {
    pub relation: CreatureRelation,
    pub controller: ControllerScope,
    pub exclude_source: bool,
    pub required_keywords: Vec<Keyword>,
    pub required_subtypes: Vec<CardSubtype>,
    pub attachment: Option<AttachmentQualification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicPermanentCount {
    pub controller: ControllerScope,
    pub any_of_card_types: Vec<CardType>,
    pub count_each_matching_object_once: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StatMultiplier {
    Fixed,
    PerMatchingPermanent(DynamicPermanentCount),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatChange {
    pub power: i16,
    pub toughness: i16,
    pub multiplier: StatMultiplier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContinuousCreatureModifierProgram {
    pub source: SourceRequirement,
    pub subject: CreatureSelector,
    pub stats: StatChange,
    pub granted_keywords: Vec<Keyword>,
    pub duration: EffectDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenColor {
    Blue,
    White,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CreatureTokenDefinition {
    pub subtypes: Vec<CardSubtype>,
    pub colors: Vec<TokenColor>,
    pub power: i16,
    pub toughness: i16,
    pub keywords: Vec<Keyword>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenTriggerEvent {
    YouCastSpell {
        any_of_card_types: Vec<CardType>,
        trigger_once_when_multiple_types_match: bool,
    },
    EnchantmentYouControlEnters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerMultiplicity {
    OncePerMatchingEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenCreationTriggerProgram {
    pub source: SourceRequirement,
    pub event: TokenTriggerEvent,
    pub multiplicity: TriggerMultiplicity,
    pub token_controller: ControllerScope,
    pub count: u16,
    pub token: CreatureTokenDefinition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreatureLifecycleEvent {
    Enters,
    Dies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerEvidence {
    LastKnownInformationForDeath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifeGainTriggerProgram {
    pub source: SourceRequirement,
    pub events: Vec<CreatureLifecycleEvent>,
    pub creature_controller: ControllerScope,
    pub exclude_source: bool,
    pub death_controller_evidence: ControllerEvidence,
    pub life_per_event: u16,
    pub multiplicity: TriggerMultiplicity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CombatTiming {
    BeginningOfEachCombat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttachmentKind {
    AuraOrEquipment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttachmentIdentity {
    TargetAttachmentPermanentInstance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttachmentMoveTriggerProgram {
    pub source: SourceRequirement,
    pub timing: CombatTiming,
    pub optional: bool,
    pub attachment_kind: AttachmentKind,
    pub attachment_controller: ControllerScope,
    pub must_be_attached_to_creature_you_control: bool,
    pub destination_creature_controller: ControllerScope,
    pub identity: AttachmentIdentity,
    pub recheck_attachment_legality: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkedDeathIdentity {
    EquippedCreatureCardThatDied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectZone {
    Graveyard,
    Hand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EquippedDeathReturnProgram {
    pub source: SourceRequirement,
    pub identity: LinkedDeathIdentity,
    pub attachment_checked_with_last_known_information: bool,
    pub from: ObjectZone,
    pub to: ObjectZone,
    pub destination_owner: ControllerScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellSelector {
    CreatureSpellWithFlyingYouCast,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpellCostReductionProgram {
    pub source: SourceRequirement,
    pub spells: SpellSelector,
    pub generic_reduction: u16,
    pub cannot_reduce_colored_requirements: bool,
    pub duration: EffectDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemporaryModifierEvent {
    AnotherCreatureYouControlWithFlyingEnters,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TemporarySelfModifierTriggerProgram {
    pub source: SourceRequirement,
    pub event: TemporaryModifierEvent,
    pub multiplicity: TriggerMultiplicity,
    pub subject: CreatureSelector,
    pub stats: StatChange,
    pub duration: EffectDuration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContinuousTriggerProgram {
    ContinuousCreatureModifier(ContinuousCreatureModifierProgram),
    TokenCreationTrigger(TokenCreationTriggerProgram),
    LifeGainTrigger(LifeGainTriggerProgram),
    AttachmentMoveTrigger(AttachmentMoveTriggerProgram),
    EquippedDeathReturn(EquippedDeathReturnProgram),
    SpellCostReduction(SpellCostReductionProgram),
    TemporarySelfModifierTrigger(TemporarySelfModifierTriggerProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledContinuousTrigger {
    pub ownership: FaceOwnership,
    pub program: ContinuousTriggerProgram,
}

pub(crate) fn compile_continuous_trigger_runtime(
    input: ContinuousTriggerCardInput<'_>,
) -> Option<Vec<CompiledContinuousTrigger>> {
    match input.layout {
        "normal" if input.faces.is_empty() => {
            let oracle_text = input.oracle_text?;
            let clauses = normalize_oracle_root(oracle_text)?;
            compile_normal_card(input.type_line, &clauses)
        }
        "modal_dfc" => compile_halvar_modal_faces(input),
        _ => None,
    }
}

fn compile_normal_card(
    type_line: &str,
    clauses: &[String],
) -> Option<Vec<CompiledContinuousTrigger>> {
    compile_alela(type_line, clauses)
        .or_else(|| compile_all_that_glitters(type_line, clauses))
        .or_else(|| compile_archon(type_line, clauses))
        .or_else(|| compile_daxos(type_line, clauses))
        .or_else(|| compile_empyrean_eagle(type_line, clauses))
        .or_else(|| compile_serras_guardian(type_line, clauses))
        .or_else(|| compile_spear_of_heliod(type_line, clauses))
        .or_else(|| compile_watcher(type_line, clauses))
        .or_else(|| compile_warden(type_line, clauses))
}

fn compile_alela(type_line: &str, clauses: &[String]) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Creature], &[])
        || clauses
            != [
                "flying, deathtouch, lifelink",
                "other creatures you control with flying get +1/+0",
                "whenever you cast an artifact or enchantment spell, create a 1/1 blue faerie creature token with flying",
            ]
    {
        return None;
    }
    let source = battlefield_source(0, &[CardType::Creature], &[]);
    Some(vec![
        exact_clause_program(
            0,
            1,
            ContinuousTriggerProgram::ContinuousCreatureModifier(
                ContinuousCreatureModifierProgram {
                    source: source.clone(),
                    subject: controlled_creatures(true, &[Keyword::Flying], &[], None),
                    stats: fixed_stats(1, 0),
                    granted_keywords: Vec::new(),
                    duration: EffectDuration::WhileSourceRemainsOnBattlefield,
                },
            ),
        ),
        exact_clause_program(
            0,
            2,
            ContinuousTriggerProgram::TokenCreationTrigger(TokenCreationTriggerProgram {
                source,
                event: TokenTriggerEvent::YouCastSpell {
                    any_of_card_types: vec![CardType::Artifact, CardType::Enchantment],
                    trigger_once_when_multiple_types_match: true,
                },
                multiplicity: TriggerMultiplicity::OncePerMatchingEvent,
                token_controller: ControllerScope::You,
                count: 1,
                token: CreatureTokenDefinition {
                    subtypes: vec![CardSubtype::Faerie],
                    colors: vec![TokenColor::Blue],
                    power: 1,
                    toughness: 1,
                    keywords: vec![Keyword::Flying],
                },
            }),
        ),
    ])
}

fn compile_all_that_glitters(
    type_line: &str,
    clauses: &[String],
) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Enchantment], &[CardSubtype::Aura])
        || clauses
            != [
                "enchant creature",
                "enchanted creature gets +1/+1 for each artifact and/or enchantment you control",
            ]
    {
        return None;
    }
    Some(vec![complete_face_program(
        0,
        clauses,
        ContinuousTriggerProgram::ContinuousCreatureModifier(ContinuousCreatureModifierProgram {
            source: battlefield_source(0, &[CardType::Enchantment], &[CardSubtype::Aura]),
            subject: CreatureSelector {
                relation: CreatureRelation::EnchantedCreature,
                controller: ControllerScope::Any,
                exclude_source: false,
                required_keywords: Vec::new(),
                required_subtypes: Vec::new(),
                attachment: None,
            },
            stats: StatChange {
                power: 1,
                toughness: 1,
                multiplier: StatMultiplier::PerMatchingPermanent(DynamicPermanentCount {
                    controller: ControllerScope::You,
                    any_of_card_types: vec![CardType::Artifact, CardType::Enchantment],
                    count_each_matching_object_once: true,
                }),
            },
            granted_keywords: Vec::new(),
            duration: EffectDuration::WhileAuraRemainsAttached,
        }),
    )])
}

fn compile_archon(type_line: &str, clauses: &[String]) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Creature], &[])
        || clauses
            != [
                "flying",
                "lifelink",
                "pegasus creatures you control have lifelink",
                "constellation - whenever an enchantment you control enters, create a 2/2 white pegasus creature token with flying",
            ]
    {
        return None;
    }
    let source = battlefield_source(0, &[CardType::Creature], &[]);
    Some(vec![
        exact_clause_program(
            0,
            2,
            ContinuousTriggerProgram::ContinuousCreatureModifier(
                ContinuousCreatureModifierProgram {
                    source: source.clone(),
                    subject: controlled_creatures(false, &[], &[CardSubtype::Pegasus], None),
                    stats: fixed_stats(0, 0),
                    granted_keywords: vec![Keyword::Lifelink],
                    duration: EffectDuration::WhileSourceRemainsOnBattlefield,
                },
            ),
        ),
        exact_clause_program(
            0,
            3,
            ContinuousTriggerProgram::TokenCreationTrigger(TokenCreationTriggerProgram {
                source,
                event: TokenTriggerEvent::EnchantmentYouControlEnters,
                multiplicity: TriggerMultiplicity::OncePerMatchingEvent,
                token_controller: ControllerScope::You,
                count: 1,
                token: CreatureTokenDefinition {
                    subtypes: vec![CardSubtype::Pegasus],
                    colors: vec![TokenColor::White],
                    power: 2,
                    toughness: 2,
                    keywords: vec![Keyword::Flying],
                },
            }),
        ),
    ])
}

fn compile_daxos(type_line: &str, clauses: &[String]) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Enchantment, CardType::Creature], &[])
        || clauses.len() != 2
        || !is_named_devotion_toughness_clause(&clauses[0])
        || clauses[1] != "whenever another creature you control enters or dies, you gain 1 life"
    {
        return None;
    }
    Some(vec![exact_clause_program(
        0,
        1,
        ContinuousTriggerProgram::LifeGainTrigger(LifeGainTriggerProgram {
            source: battlefield_source(0, &[CardType::Enchantment, CardType::Creature], &[]),
            events: vec![CreatureLifecycleEvent::Enters, CreatureLifecycleEvent::Dies],
            creature_controller: ControllerScope::You,
            exclude_source: true,
            death_controller_evidence: ControllerEvidence::LastKnownInformationForDeath,
            life_per_event: 1,
            multiplicity: TriggerMultiplicity::OncePerMatchingEvent,
        }),
    )])
}

fn compile_empyrean_eagle(
    type_line: &str,
    clauses: &[String],
) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Creature], &[])
        || clauses
            != [
                "flying",
                "other creatures you control with flying get +1/+1",
            ]
    {
        return None;
    }
    Some(vec![exact_clause_program(
        0,
        1,
        ContinuousTriggerProgram::ContinuousCreatureModifier(ContinuousCreatureModifierProgram {
            source: battlefield_source(0, &[CardType::Creature], &[]),
            subject: controlled_creatures(true, &[Keyword::Flying], &[], None),
            stats: fixed_stats(1, 1),
            granted_keywords: Vec::new(),
            duration: EffectDuration::WhileSourceRemainsOnBattlefield,
        }),
    )])
}

fn compile_serras_guardian(
    type_line: &str,
    clauses: &[String],
) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Creature], &[])
        || clauses
            != [
                "flying",
                "vigilance",
                "other creatures you control have vigilance",
            ]
    {
        return None;
    }
    Some(vec![exact_clause_program(
        0,
        2,
        ContinuousTriggerProgram::ContinuousCreatureModifier(ContinuousCreatureModifierProgram {
            source: battlefield_source(0, &[CardType::Creature], &[]),
            subject: controlled_creatures(true, &[], &[], None),
            stats: fixed_stats(0, 0),
            granted_keywords: vec![Keyword::Vigilance],
            duration: EffectDuration::WhileSourceRemainsOnBattlefield,
        }),
    )])
}

fn compile_spear_of_heliod(
    type_line: &str,
    clauses: &[String],
) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Enchantment, CardType::Artifact], &[])
        || clauses
            != [
                "creatures you control get +1/+1",
                "{1}{w}{w}, {t}: destroy target creature that dealt damage to you this turn",
            ]
    {
        return None;
    }
    Some(vec![exact_clause_program(
        0,
        0,
        ContinuousTriggerProgram::ContinuousCreatureModifier(ContinuousCreatureModifierProgram {
            source: battlefield_source(0, &[CardType::Enchantment, CardType::Artifact], &[]),
            subject: controlled_creatures(false, &[], &[], None),
            stats: fixed_stats(1, 1),
            granted_keywords: Vec::new(),
            duration: EffectDuration::WhileSourceRemainsOnBattlefield,
        }),
    )])
}

fn compile_watcher(type_line: &str, clauses: &[String]) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Creature], &[])
        || clauses
            != [
                "flying",
                "creature spells with flying you cast cost {1} less to cast",
                "whenever another creature you control with flying enters, this creature gets +1/+1 until end of turn",
            ]
    {
        return None;
    }
    let source = battlefield_source(0, &[CardType::Creature], &[]);
    Some(vec![
        exact_clause_program(
            0,
            1,
            ContinuousTriggerProgram::SpellCostReduction(flying_creature_cost_reduction(
                source.clone(),
            )),
        ),
        exact_clause_program(
            0,
            2,
            ContinuousTriggerProgram::TemporarySelfModifierTrigger(
                TemporarySelfModifierTriggerProgram {
                    source,
                    event: TemporaryModifierEvent::AnotherCreatureYouControlWithFlyingEnters,
                    multiplicity: TriggerMultiplicity::OncePerMatchingEvent,
                    subject: CreatureSelector {
                        relation: CreatureRelation::SourceCreature,
                        controller: ControllerScope::You,
                        exclude_source: false,
                        required_keywords: Vec::new(),
                        required_subtypes: Vec::new(),
                        attachment: None,
                    },
                    stats: fixed_stats(1, 1),
                    duration: EffectDuration::UntilEndOfTurn,
                },
            ),
        ),
    ])
}

fn compile_warden(type_line: &str, clauses: &[String]) -> Option<Vec<CompiledContinuousTrigger>> {
    if !source_type_matches(type_line, &[CardType::Creature], &[])
        || clauses
            != [
                "flying",
                "creature spells with flying you cast cost {1} less to cast",
            ]
    {
        return None;
    }
    Some(vec![exact_clause_program(
        0,
        1,
        ContinuousTriggerProgram::SpellCostReduction(flying_creature_cost_reduction(
            battlefield_source(0, &[CardType::Creature], &[]),
        )),
    )])
}

fn compile_halvar_modal_faces(
    input: ContinuousTriggerCardInput<'_>,
) -> Option<Vec<CompiledContinuousTrigger>> {
    let [front, back] = input.faces else {
        return None;
    };
    if let Some(root_oracle_text) = input.oracle_text {
        let exact_joined_faces = format!("{}\n//\n{}", front.oracle_text, back.oracle_text);
        if root_oracle_text != exact_joined_faces {
            return None;
        }
    }
    let root_faces = input
        .type_line
        .split("//")
        .map(str::trim)
        .collect::<Vec<_>>();
    if root_faces.len() != 2
        || !source_type_matches(root_faces[0], &[CardType::Creature], &[])
        || !source_type_matches(
            root_faces[1],
            &[CardType::Artifact],
            &[CardSubtype::Equipment],
        )
        || !source_type_matches(front.type_line, &[CardType::Creature], &[])
        || !source_type_matches(
            back.type_line,
            &[CardType::Artifact],
            &[CardSubtype::Equipment],
        )
    {
        return None;
    }
    let front_clauses = normalize_oracle_root(front.oracle_text)?;
    let back_clauses = normalize_oracle_root(back.oracle_text)?;
    if front_clauses
        != [
            "creatures you control that are enchanted or equipped have double strike",
            "at the beginning of each combat, you may attach target aura or equipment attached to a creature you control to target creature you control",
        ]
        || back_clauses
            != [
                "equipped creature gets +2/+0 and has vigilance",
                "whenever equipped creature dies, return it to its owner's hand",
                "equip {1}{w}",
            ]
    {
        return None;
    }

    let front_source = battlefield_source(0, &[CardType::Creature], &[]);
    let back_source = battlefield_source(1, &[CardType::Artifact], &[CardSubtype::Equipment]);
    Some(vec![
        exact_clause_program(
            0,
            0,
            ContinuousTriggerProgram::ContinuousCreatureModifier(
                ContinuousCreatureModifierProgram {
                    source: front_source.clone(),
                    subject: controlled_creatures(
                        false,
                        &[],
                        &[],
                        Some(AttachmentQualification::EnchantedOrEquipped),
                    ),
                    stats: fixed_stats(0, 0),
                    granted_keywords: vec![Keyword::DoubleStrike],
                    duration: EffectDuration::WhileSourceRemainsOnBattlefield,
                },
            ),
        ),
        exact_clause_program(
            0,
            1,
            ContinuousTriggerProgram::AttachmentMoveTrigger(AttachmentMoveTriggerProgram {
                source: front_source,
                timing: CombatTiming::BeginningOfEachCombat,
                optional: true,
                attachment_kind: AttachmentKind::AuraOrEquipment,
                attachment_controller: ControllerScope::Any,
                must_be_attached_to_creature_you_control: true,
                destination_creature_controller: ControllerScope::You,
                identity: AttachmentIdentity::TargetAttachmentPermanentInstance,
                recheck_attachment_legality: true,
            }),
        ),
        exact_clause_program(
            1,
            0,
            ContinuousTriggerProgram::ContinuousCreatureModifier(
                ContinuousCreatureModifierProgram {
                    source: back_source.clone(),
                    subject: CreatureSelector {
                        relation: CreatureRelation::EquippedCreature,
                        controller: ControllerScope::Any,
                        exclude_source: false,
                        required_keywords: Vec::new(),
                        required_subtypes: Vec::new(),
                        attachment: None,
                    },
                    stats: fixed_stats(2, 0),
                    granted_keywords: vec![Keyword::Vigilance],
                    duration: EffectDuration::WhileSourceRemainsOnBattlefield,
                },
            ),
        ),
        exact_clause_program(
            1,
            1,
            ContinuousTriggerProgram::EquippedDeathReturn(EquippedDeathReturnProgram {
                source: back_source,
                identity: LinkedDeathIdentity::EquippedCreatureCardThatDied,
                attachment_checked_with_last_known_information: true,
                from: ObjectZone::Graveyard,
                to: ObjectZone::Hand,
                destination_owner: ControllerScope::Owner,
            }),
        ),
    ])
}

fn flying_creature_cost_reduction(source: SourceRequirement) -> SpellCostReductionProgram {
    SpellCostReductionProgram {
        source,
        spells: SpellSelector::CreatureSpellWithFlyingYouCast,
        generic_reduction: 1,
        cannot_reduce_colored_requirements: true,
        duration: EffectDuration::WhileSourceRemainsOnBattlefield,
    }
}

fn controlled_creatures(
    exclude_source: bool,
    required_keywords: &[Keyword],
    required_subtypes: &[CardSubtype],
    attachment: Option<AttachmentQualification>,
) -> CreatureSelector {
    CreatureSelector {
        relation: CreatureRelation::ControlledCreatures,
        controller: ControllerScope::You,
        exclude_source,
        required_keywords: required_keywords.to_vec(),
        required_subtypes: required_subtypes.to_vec(),
        attachment,
    }
}

fn fixed_stats(power: i16, toughness: i16) -> StatChange {
    StatChange {
        power,
        toughness,
        multiplier: StatMultiplier::Fixed,
    }
}

fn battlefield_source(
    face_index: u16,
    card_types: &[CardType],
    required_subtypes: &[CardSubtype],
) -> SourceRequirement {
    SourceRequirement {
        zone: SourceZone::Battlefield,
        face_index,
        card_types: card_types.to_vec(),
        required_subtypes: required_subtypes.to_vec(),
    }
}

fn exact_clause_program(
    face_index: u16,
    clause_index: u16,
    program: ContinuousTriggerProgram,
) -> CompiledContinuousTrigger {
    CompiledContinuousTrigger {
        ownership: FaceOwnership {
            face_index,
            oracle: OracleOwnership::ExactClauseSet {
                clause_indices: vec![clause_index],
            },
        },
        program,
    }
}

fn complete_face_program(
    face_index: u16,
    clauses: &[String],
    program: ContinuousTriggerProgram,
) -> CompiledContinuousTrigger {
    CompiledContinuousTrigger {
        ownership: FaceOwnership {
            face_index,
            oracle: OracleOwnership::CompleteFaceRoot {
                clause_count: u16::try_from(clauses.len()).expect("reviewed roots fit in u16"),
            },
        },
        program,
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
        CardType::Artifact => "artifact",
        CardType::Creature => "creature",
        CardType::Enchantment => "enchantment",
    }
}

fn subtype_word(subtype: CardSubtype) -> &'static str {
    match subtype {
        CardSubtype::Aura => "aura",
        CardSubtype::Equipment => "equipment",
        CardSubtype::Faerie => "faerie",
        CardSubtype::Pegasus => "pegasus",
    }
}

fn has_type_line_word(type_line: &str, expected: &str) -> bool {
    type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn is_named_devotion_toughness_clause(clause: &str) -> bool {
    let suffix = "'s toughness is equal to your devotion to white";
    let Some(reference) = clause.strip_suffix(suffix) else {
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
        "a", "all", "an", "any", "each", "it", "source", "target", "that", "the", "them", "this",
        "those", "you", "your",
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
        let without_reminders = remove_reviewed_reminder_text(&collapsed);
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
        " (damage dealt by this creature also causes you to gain that much life.)",
        " (each {w} in the mana costs of permanents you control counts toward your devotion to white.)",
        " (this creature can't be blocked except by creatures with flying or reach.)",
        " (attacking doesn't cause this creature to tap.)",
    ];
    REMINDERS.iter().fold(value.to_string(), |text, reminder| {
        text.replace(reminder, "")
    })
}
