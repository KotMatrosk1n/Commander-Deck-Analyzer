//! Typed programs for complete parenthetical Oracle lines.
//!
//! These lines are not ordinary spell or ability instructions. They describe
//! shared rules procedures, face relationships, intrinsic characteristics, or
//! notation. Keeping them as typed programs prevents explanatory text from
//! being mistaken for an executable effect while still retaining every rule
//! that the line communicates.

#![allow(dead_code)]

use sha2::{Digest, Sha256};

pub const STANDALONE_ORACLE_ANNOTATION_COMPILER_VERSION: &str =
    "standalone-oracle-annotation-compiler-0.1";
pub const STANDALONE_ORACLE_ANNOTATION_RUNTIME_VERSION: &str =
    "standalone-oracle-annotation-runtime-0.1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandaloneOracleAnnotation {
    exact_source: String,
    semantic_digest: String,
    kind: StandaloneOracleAnnotationKind,
}

impl StandaloneOracleAnnotation {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &StandaloneOracleAnnotationKind {
        &self.kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandaloneOracleAnnotationKind {
    SagaLifecycle(SagaLifecycleAnnotation),
    RoomLifecycle(RoomLifecycleAnnotation),
    ClassLevelLifecycle(ClassLevelAnnotation),
    BattleLifecycle(BattleLifecycleAnnotation),
    OngoingSchemeLifecycle(OngoingSchemeAnnotation),
    ConspiracySetup(ConspiracySetupAnnotation),
    ManaNotation(ManaNotationAnnotation),
    IntrinsicManaAbility(IntrinsicManaAbilityAnnotation),
    LegendarySpellRestriction(LegendarySpellRestrictionAnnotation),
    TransformOrigin {
        front_face_name: String,
    },
    MeldPartner {
        partner_name: String,
    },
    MissingManaCostRestriction(MissingManaCostWording),
    CounterMarker {
        counter: MarkerCounter,
    },
    PoisonLossThreshold {
        counters: u8,
    },
    CreatureSizeClassification {
        small_maximum_total: u16,
        medium_minimum_total: u16,
        medium_maximum_total: u16,
        large_minimum_total: u16,
    },
    LandCreatureCharacteristics(LandCreatureAnnotation),
    FlashLikePermission {
        broader_card_types: bool,
    },
    EnumeratedExternalProcedure {
        current_count: u16,
        die_sides: u16,
    },
    InfinitePower,
    DoubleFacedSubstitute,
    BackFaceCastRestriction,
    LandDropAlternativeCost {
        mana: String,
        color: AnnotationColor,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SagaLifecycleAnnotation {
    pub lore_counters_on_entry: u8,
    pub lore_counters_after_controller_draw_step: u8,
    pub final_chapter: Option<u16>,
    pub sacrifice_after_final_chapter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomLifecycleAnnotation {
    pub either_half_may_be_cast: bool,
    pub cast_door_unlocks_on_battlefield: bool,
    pub unlock_is_sorcery_special_action: bool,
    pub unlock_cost_is_locked_door_mana_cost: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassLevelAnnotation {
    pub next_level_only: bool,
    pub timing_is_sorcery: bool,
    pub installs_level_ability: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleLifecycleAnnotation {
    Siege {
        protector_chosen_from_opponents: bool,
        controller_and_other_players_may_attack: bool,
        defeated_destination_is_exile: bool,
        defeated_back_face_is_cast: bool,
    },
    ControlPointReturn {
        any_opponent_may_attack: bool,
        defeated_destination_is_exile: bool,
        return_under_defeating_players_control: bool,
        defense_counters_on_return: Option<u16>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OngoingSchemeAnnotation {
    pub remains_face_up: bool,
    pub until_abandoned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConspiracySetupAnnotation {
    pub starts_face_up_in_command_zone: bool,
    pub excluded_from_minimum_deck_size: bool,
    pub secret_mission: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaNotationAnnotation {
    Represents {
        symbol: AnnotationManaSymbol,
        meaning: ManaSymbolMeaning,
    },
    PaymentAlternatives {
        symbol: AnnotationManaSymbol,
        alternatives: Vec<ManaPaymentAlternative>,
        printed_mana_value: Option<u16>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaSymbolMeaning {
    ColorlessMana,
    ManaFromSnowSource,
    ManaFromLegendarySource,
    LandDrop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaPaymentAlternative {
    Mana(AnnotationManaSymbol),
    AnyMana(u16),
    Life(u16),
    ManaFromSnowSource(u16),
    ManaFromLegendarySource(u16),
    GiveUpLandDrops(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationManaSymbol {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    Snow,
    Legendary,
    LandDrop,
    Hybrid(Box<AnnotationManaSymbol>, Box<AnnotationManaSymbol>),
    GenericHybrid {
        generic: u16,
        color: Box<AnnotationManaSymbol>,
    },
    Phyrexian(Box<AnnotationManaSymbol>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrinsicManaAbilityAnnotation {
    pub taps_source: bool,
    pub choices: Vec<AnnotationManaSymbol>,
    pub produces_one_mana: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendarySpellKind {
    Instant,
    Sorcery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegendarySpellRestrictionAnnotation {
    pub spell_kind: LegendarySpellKind,
    pub requires_controlled_legendary_creature_or_planeswalker: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingManaCostWording {
    CannotBePaid,
    CannotBePlayed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerCounter {
    Acorn,
    Experience,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandCreatureAnnotation {
    pub is_not_a_spell: bool,
    pub affected_by_summoning_sickness: bool,
    pub color: Option<AnnotationColor>,
    pub intrinsic_mana_ability: IntrinsicManaAbilityAnnotation,
}

pub fn compile_standalone_oracle_annotation(source: &str) -> Option<StandaloneOracleAnnotation> {
    let exact_source = source.trim();
    let inner = exact_source.strip_prefix('(')?.strip_suffix(')')?;
    if inner.trim().is_empty() {
        return None;
    }

    let kind = parse_saga_lifecycle(inner)
        .or_else(|| parse_room_lifecycle(inner))
        .or_else(|| parse_class_level_lifecycle(inner))
        .or_else(|| parse_battle_lifecycle(inner))
        .or_else(|| parse_scheme_lifecycle(inner))
        .or_else(|| parse_conspiracy_setup(inner))
        .or_else(|| parse_mana_notation(inner))
        .or_else(|| parse_intrinsic_mana_ability(inner))
        .or_else(|| parse_legendary_spell_restriction(inner))
        .or_else(|| parse_transform_origin(inner))
        .or_else(|| parse_meld_partner(inner))
        .or_else(|| parse_missing_mana_cost(inner))
        .or_else(|| parse_counter_marker(inner))
        .or_else(|| parse_poison_loss_threshold(inner))
        .or_else(|| parse_creature_size_classification(inner))
        .or_else(|| parse_land_creature_characteristics(inner))
        .or_else(|| parse_exact_miscellaneous_annotation(inner))?;

    let semantic_digest = annotation_semantic_digest(exact_source, &kind);
    Some(StandaloneOracleAnnotation {
        exact_source: exact_source.to_owned(),
        semantic_digest,
        kind,
    })
}

fn parse_saga_lifecycle(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    let lower = inner.to_ascii_lowercase();
    let base = [
        "as this saga enters and after your draw step, add a lore counter.",
        "as this saga enters and after your draw step add a lore counter.",
    ]
    .into_iter()
    .find(|candidate| lower.starts_with(candidate))?;
    let remainder = inner[base.len()..].trim();
    let final_chapter = if remainder.is_empty() {
        None
    } else {
        let roman = remainder
            .strip_prefix("Sacrifice after ")
            .or_else(|| remainder.strip_prefix("sacrifice after "))?
            .strip_suffix('.')?;
        Some(parse_roman_chapter(roman)?)
    };
    Some(StandaloneOracleAnnotationKind::SagaLifecycle(
        SagaLifecycleAnnotation {
            lore_counters_on_entry: 1,
            lore_counters_after_controller_draw_step: 1,
            final_chapter,
            sacrifice_after_final_chapter: final_chapter.is_some(),
        },
    ))
}

fn parse_room_lifecycle(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    (inner
        == "You may cast either half. That door unlocks on the battlefield. As a sorcery, you may pay the mana cost of a locked door to unlock it.")
        .then_some(StandaloneOracleAnnotationKind::RoomLifecycle(
            RoomLifecycleAnnotation {
                either_half_may_be_cast: true,
                cast_door_unlocks_on_battlefield: true,
                unlock_is_sorcery_special_action: true,
                unlock_cost_is_locked_door_mana_cost: true,
            },
        ))
}

fn parse_class_level_lifecycle(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    (inner == "Gain the next level as a sorcery to add its ability.").then_some(
        StandaloneOracleAnnotationKind::ClassLevelLifecycle(ClassLevelAnnotation {
            next_level_only: true,
            timing_is_sorcery: true,
            installs_level_ability: true,
        }),
    )
}

fn parse_battle_lifecycle(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    if inner
        == "As a Siege enters, choose an opponent to protect it. You and others can attack it. When it's defeated, exile it, then cast it transformed."
    {
        return Some(StandaloneOracleAnnotationKind::BattleLifecycle(
            BattleLifecycleAnnotation::Siege {
                protector_chosen_from_opponents: true,
                controller_and_other_players_may_attack: true,
                defeated_destination_is_exile: true,
                defeated_back_face_is_cast: true,
            },
        ));
    }
    if inner
        == "An opponent may attack this battle. When an opponent defeats it, exile it, then return it to the battlefield under their control."
    {
        return Some(StandaloneOracleAnnotationKind::BattleLifecycle(
            BattleLifecycleAnnotation::ControlPointReturn {
                any_opponent_may_attack: true,
                defeated_destination_is_exile: true,
                return_under_defeating_players_control: true,
                defense_counters_on_return: None,
            },
        ));
    }
    if inner
        == "Any opponent may attack this battle. When an opponent defeats this, exile it, then put it onto the battlefield under that player's control with 4 defense counters. This is different from sieges!"
    {
        return Some(StandaloneOracleAnnotationKind::BattleLifecycle(
            BattleLifecycleAnnotation::ControlPointReturn {
                any_opponent_may_attack: true,
                defeated_destination_is_exile: true,
                return_under_defeating_players_control: true,
                defense_counters_on_return: Some(4),
            },
        ));
    }
    None
}

fn parse_scheme_lifecycle(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    match inner {
        "An ongoing scheme remains face up until it's abandoned." => Some(
            StandaloneOracleAnnotationKind::OngoingSchemeLifecycle(OngoingSchemeAnnotation {
                remains_face_up: true,
                until_abandoned: true,
            }),
        ),
        "An ongoing scheme remains face up." => Some(
            StandaloneOracleAnnotationKind::OngoingSchemeLifecycle(OngoingSchemeAnnotation {
                remains_face_up: true,
                until_abandoned: false,
            }),
        ),
        _ => None,
    }
}

fn parse_conspiracy_setup(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    match inner {
        "Start the game with this conspiracy face up in the command zone." => Some(
            StandaloneOracleAnnotationKind::ConspiracySetup(ConspiracySetupAnnotation {
                starts_face_up_in_command_zone: true,
                excluded_from_minimum_deck_size: false,
                secret_mission: false,
            }),
        ),
        "Start the game with this Conspiracy face up in the command zone. It doesn't count toward your minimum deck size." => {
            Some(StandaloneOracleAnnotationKind::ConspiracySetup(
                ConspiracySetupAnnotation {
                    starts_face_up_in_command_zone: true,
                    excluded_from_minimum_deck_size: true,
                    secret_mission: false,
                },
            ))
        }
        "Start the game with this conspiracy face up in the command zone. Before the game, secretly choose one of the following. During your end step, if you meet the condition, you may reveal your choice and turn this card face down. When you do, collect the reward." => {
            Some(StandaloneOracleAnnotationKind::ConspiracySetup(
                ConspiracySetupAnnotation {
                    starts_face_up_in_command_zone: true,
                    excluded_from_minimum_deck_size: false,
                    secret_mission: true,
                },
            ))
        }
        _ => None,
    }
}

fn parse_mana_notation(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    if inner == "{C} represents colorless mana." {
        return Some(StandaloneOracleAnnotationKind::ManaNotation(
            ManaNotationAnnotation::Represents {
                symbol: AnnotationManaSymbol::Colorless,
                meaning: ManaSymbolMeaning::ColorlessMana,
            },
        ));
    }
    if inner == "{S} can be paid with one mana from a snow source." {
        return Some(StandaloneOracleAnnotationKind::ManaNotation(
            ManaNotationAnnotation::PaymentAlternatives {
                symbol: AnnotationManaSymbol::Snow,
                alternatives: vec![ManaPaymentAlternative::ManaFromSnowSource(1)],
                printed_mana_value: None,
            },
        ));
    }
    if inner == "{L} can be paid with one mana from a legendary source." {
        return Some(StandaloneOracleAnnotationKind::ManaNotation(
            ManaNotationAnnotation::PaymentAlternatives {
                symbol: AnnotationManaSymbol::Legendary,
                alternatives: vec![ManaPaymentAlternative::ManaFromLegendarySource(1)],
                printed_mana_value: None,
            },
        ));
    }
    if inner
        == "{D} is a land drop. You may give up one potential land drop this turn to pay for {D}."
    {
        return Some(StandaloneOracleAnnotationKind::ManaNotation(
            ManaNotationAnnotation::PaymentAlternatives {
                symbol: AnnotationManaSymbol::LandDrop,
                alternatives: vec![ManaPaymentAlternative::GiveUpLandDrops(1)],
                printed_mana_value: None,
            },
        ));
    }

    let payment = inner.strip_suffix('.')?;
    if let Some((symbol_text, alternatives_text)) = payment
        .strip_prefix('{')
        .and_then(|text| text.split_once("} can be paid with either "))
    {
        let symbol = parse_mana_symbol_body(symbol_text)?;
        let (first, second) = alternatives_text.split_once(" or ")?;
        let alternatives = vec![
            parse_payment_alternative(first)?,
            parse_payment_alternative(second)?,
        ];
        if !payment_alternatives_match_symbol(&symbol, &alternatives) {
            return None;
        }
        return Some(StandaloneOracleAnnotationKind::ManaNotation(
            ManaNotationAnnotation::PaymentAlternatives {
                symbol,
                alternatives,
                printed_mana_value: None,
            },
        ));
    }

    let (payment_text, mana_value_text) = payment.split_once(". This card's mana value is ")?;
    let mana_value = mana_value_text.parse::<u16>().ok()?;
    let (symbol_text, alternatives_text) = payment_text
        .strip_prefix('{')
        .and_then(|text| text.split_once("} can be paid with "))?;
    let symbol = parse_mana_symbol_body(symbol_text)?;
    let (first, second) = alternatives_text.split_once(" or with ")?;
    let first = first.strip_prefix("any ")?.strip_suffix(" mana")?;
    let first = english_or_numeric_u16(first)?;
    let alternatives = vec![
        ManaPaymentAlternative::AnyMana(first),
        parse_payment_alternative(second)?,
    ];
    if !payment_alternatives_match_symbol(&symbol, &alternatives) {
        return None;
    }
    Some(StandaloneOracleAnnotationKind::ManaNotation(
        ManaNotationAnnotation::PaymentAlternatives {
            symbol,
            alternatives,
            printed_mana_value: Some(mana_value),
        },
    ))
}

fn payment_alternatives_match_symbol(
    symbol: &AnnotationManaSymbol,
    alternatives: &[ManaPaymentAlternative],
) -> bool {
    match (symbol, alternatives) {
        (
            AnnotationManaSymbol::Phyrexian(color),
            [
                ManaPaymentAlternative::Mana(mana),
                ManaPaymentAlternative::Life(2),
            ],
        ) => mana == color.as_ref(),
        (
            AnnotationManaSymbol::Hybrid(first, second),
            [
                ManaPaymentAlternative::Mana(first_mana),
                ManaPaymentAlternative::Mana(second_mana),
            ],
        ) => first_mana == first.as_ref() && second_mana == second.as_ref(),
        (
            AnnotationManaSymbol::GenericHybrid { generic, color },
            [
                ManaPaymentAlternative::AnyMana(amount),
                ManaPaymentAlternative::Mana(mana),
            ],
        ) => amount == generic && mana == color.as_ref(),
        _ => false,
    }
}

fn parse_payment_alternative(text: &str) -> Option<ManaPaymentAlternative> {
    let text = text.trim();
    if let Some(amount) = text.strip_suffix(" life") {
        return Some(ManaPaymentAlternative::Life(english_or_numeric_u16(
            amount,
        )?));
    }
    let symbol = text.strip_prefix('{')?.strip_suffix('}')?;
    Some(ManaPaymentAlternative::Mana(parse_mana_symbol_body(
        symbol,
    )?))
}

fn parse_mana_symbol_body(body: &str) -> Option<AnnotationManaSymbol> {
    let atom = |value| match value {
        "W" => Some(AnnotationManaSymbol::White),
        "U" => Some(AnnotationManaSymbol::Blue),
        "B" => Some(AnnotationManaSymbol::Black),
        "R" => Some(AnnotationManaSymbol::Red),
        "G" => Some(AnnotationManaSymbol::Green),
        "C" => Some(AnnotationManaSymbol::Colorless),
        "S" => Some(AnnotationManaSymbol::Snow),
        "L" => Some(AnnotationManaSymbol::Legendary),
        "D" => Some(AnnotationManaSymbol::LandDrop),
        _ => None,
    };
    if let Some(symbol) = atom(body) {
        return Some(symbol);
    }
    let (first, second) = body.split_once('/')?;
    if second == "P" {
        return Some(AnnotationManaSymbol::Phyrexian(Box::new(atom(first)?)));
    }
    if let Ok(generic) = first.parse::<u16>() {
        return Some(AnnotationManaSymbol::GenericHybrid {
            generic,
            color: Box::new(atom(second)?),
        });
    }
    Some(AnnotationManaSymbol::Hybrid(
        Box::new(atom(first)?),
        Box::new(atom(second)?),
    ))
}

fn parse_intrinsic_mana_ability(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    let body = inner.strip_prefix("{T}: Add ")?.strip_suffix('.')?;
    let choices = split_mana_choices(body)?
        .into_iter()
        .map(|symbol| {
            let body = symbol.strip_prefix('{')?.strip_suffix('}')?;
            parse_mana_symbol_body(body)
        })
        .collect::<Option<Vec<_>>>()?;
    (!choices.is_empty()).then_some(StandaloneOracleAnnotationKind::IntrinsicManaAbility(
        IntrinsicManaAbilityAnnotation {
            taps_source: true,
            choices,
            produces_one_mana: true,
        },
    ))
}

fn split_mana_choices(body: &str) -> Option<Vec<String>> {
    let normalized = body.replace(", or ", ", ").replace(" or ", ", ");
    let choices = normalized
        .split(", ")
        .map(str::trim)
        .filter(|choice| !choice.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    (!choices.is_empty()).then_some(choices)
}

fn parse_legendary_spell_restriction(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    let spell_kind = match inner {
        "You may cast a legendary instant only if you control a legendary creature or planeswalker." => {
            LegendarySpellKind::Instant
        }
        "You may cast a legendary sorcery only if you control a legendary creature or planeswalker." => {
            LegendarySpellKind::Sorcery
        }
        _ => return None,
    };
    Some(StandaloneOracleAnnotationKind::LegendarySpellRestriction(
        LegendarySpellRestrictionAnnotation {
            spell_kind,
            requires_controlled_legendary_creature_or_planeswalker: true,
        },
    ))
}

fn parse_transform_origin(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    let front_face_name = inner
        .strip_prefix("Transforms from ")?
        .strip_suffix('.')?
        .trim();
    (!front_face_name.is_empty()).then_some(StandaloneOracleAnnotationKind::TransformOrigin {
        front_face_name: front_face_name.to_owned(),
    })
}

fn parse_meld_partner(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    let partner_name = inner.strip_prefix("Melds with ")?.strip_suffix('.')?.trim();
    (!partner_name.is_empty()).then_some(StandaloneOracleAnnotationKind::MeldPartner {
        partner_name: partner_name.to_owned(),
    })
}

fn parse_missing_mana_cost(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    match inner {
        "Nonexistent mana costs can't be paid." => {
            Some(StandaloneOracleAnnotationKind::MissingManaCostRestriction(
                MissingManaCostWording::CannotBePaid,
            ))
        }
        "Spells without mana costs can't be played" => {
            Some(StandaloneOracleAnnotationKind::MissingManaCostRestriction(
                MissingManaCostWording::CannotBePlayed,
            ))
        }
        _ => None,
    }
}

fn parse_counter_marker(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    match inner {
        "Place your acorn counters in this area." => {
            Some(StandaloneOracleAnnotationKind::CounterMarker {
                counter: MarkerCounter::Acorn,
            })
        }
        "Place your experience counters here." => {
            Some(StandaloneOracleAnnotationKind::CounterMarker {
                counter: MarkerCounter::Experience,
            })
        }
        _ => None,
    }
}

fn parse_poison_loss_threshold(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    (inner == "A player with ten or more poison counters loses the game.")
        .then_some(StandaloneOracleAnnotationKind::PoisonLossThreshold { counters: 10 })
}

fn parse_creature_size_classification(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    (inner
        == "A small creature has total power and toughness 4 or less, medium is 5\u{2014}8, and large is 9 or more.")
        .then_some(StandaloneOracleAnnotationKind::CreatureSizeClassification {
            small_maximum_total: 4,
            medium_minimum_total: 5,
            medium_maximum_total: 8,
            large_minimum_total: 9,
        })
}

fn parse_land_creature_characteristics(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    let (prefix, ability_text) = inner.split_once(" and it has \"")?;
    let ability_text = ability_text.strip_suffix('"')?;
    let ability = match parse_intrinsic_mana_ability(ability_text)? {
        StandaloneOracleAnnotationKind::IntrinsicManaAbility(ability) => ability,
        _ => return None,
    };
    let lower = prefix.to_ascii_lowercase();
    if !lower.contains("isn't a spell") || !lower.contains("affected by summoning sickness") {
        return None;
    }
    let color = [
        ("it's white", AnnotationColor::White),
        ("it's blue", AnnotationColor::Blue),
        ("it's black", AnnotationColor::Black),
        ("it's red", AnnotationColor::Red),
        ("it's green", AnnotationColor::Green),
    ]
    .into_iter()
    .find_map(|(needle, color)| lower.contains(needle).then_some(color));
    Some(StandaloneOracleAnnotationKind::LandCreatureCharacteristics(
        LandCreatureAnnotation {
            is_not_a_spell: true,
            affected_by_summoning_sickness: true,
            color,
            intrinsic_mana_ability: ability,
        },
    ))
}

fn parse_exact_miscellaneous_annotation(inner: &str) -> Option<StandaloneOracleAnnotationKind> {
    match inner {
        "It's like flash but more card types." => {
            Some(StandaloneOracleAnnotationKind::FlashLikePermission {
                broader_card_types: true,
            })
        }
        "There are currently thirteen. Perhaps look up the list and roll a D20?" => Some(
            StandaloneOracleAnnotationKind::EnumeratedExternalProcedure {
                current_count: 13,
                die_sides: 20,
            },
        ),
        "This creature has INFINITE POWER." => Some(StandaloneOracleAnnotationKind::InfinitePower),
        "You can use this card to represent a double-faced card." => {
            Some(StandaloneOracleAnnotationKind::DoubleFacedSubstitute)
        }
        "You can't cast this face unless it's been transformed by the front face." => {
            Some(StandaloneOracleAnnotationKind::BackFaceCastRestriction)
        }
        "You have to pay {2}{G} to play this land as your land drop. It's green." => {
            Some(StandaloneOracleAnnotationKind::LandDropAlternativeCost {
                mana: "{2}{G}".to_owned(),
                color: AnnotationColor::Green,
            })
        }
        _ => None,
    }
}

fn annotation_semantic_digest(exact_source: &str, kind: &StandaloneOracleAnnotationKind) -> String {
    let mut components = vec![
        "standalone-oracle-annotation-semantic-content/v1".to_owned(),
        STANDALONE_ORACLE_ANNOTATION_COMPILER_VERSION.to_owned(),
        STANDALONE_ORACLE_ANNOTATION_RUNTIME_VERSION.to_owned(),
        exact_source.to_owned(),
    ];
    components.extend(annotation_semantic_components(kind));
    let mut hasher = Sha256::new();
    for component in components {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn annotation_semantic_components(kind: &StandaloneOracleAnnotationKind) -> Vec<String> {
    match kind {
        StandaloneOracleAnnotationKind::SagaLifecycle(program) => vec![
            "saga-lifecycle/v1".to_owned(),
            program.lore_counters_on_entry.to_string(),
            program.lore_counters_after_controller_draw_step.to_string(),
            program
                .final_chapter
                .map(|chapter| chapter.to_string())
                .unwrap_or_else(|| "highest-printed".to_owned()),
            program.sacrifice_after_final_chapter.to_string(),
        ],
        StandaloneOracleAnnotationKind::RoomLifecycle(_) => {
            vec!["room-lifecycle/v1".to_owned()]
        }
        StandaloneOracleAnnotationKind::ClassLevelLifecycle(_) => {
            vec!["class-level-lifecycle/v1".to_owned()]
        }
        StandaloneOracleAnnotationKind::BattleLifecycle(program) => vec![match program {
            BattleLifecycleAnnotation::Siege { .. } => "battle-siege-lifecycle/v1".to_owned(),
            BattleLifecycleAnnotation::ControlPointReturn {
                defense_counters_on_return,
                ..
            } => format!(
                "battle-control-point-return/v1/defense:{}",
                defense_counters_on_return
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unchanged".to_owned())
            ),
        }],
        StandaloneOracleAnnotationKind::OngoingSchemeLifecycle(program) => vec![format!(
            "ongoing-scheme/v1/until-abandoned:{}",
            program.until_abandoned
        )],
        StandaloneOracleAnnotationKind::ConspiracySetup(program) => vec![format!(
            "conspiracy-setup/v1/minimum-excluded:{}/secret-mission:{}",
            program.excluded_from_minimum_deck_size, program.secret_mission
        )],
        StandaloneOracleAnnotationKind::ManaNotation(program) => {
            vec![format!("mana-notation/v1/{program:?}")]
        }
        StandaloneOracleAnnotationKind::IntrinsicManaAbility(program) => {
            vec![format!("intrinsic-mana-ability/v1/{program:?}")]
        }
        StandaloneOracleAnnotationKind::LegendarySpellRestriction(program) => {
            vec![format!("legendary-spell-restriction/v1/{program:?}")]
        }
        StandaloneOracleAnnotationKind::TransformOrigin { front_face_name } => {
            vec!["transform-origin/v1".to_owned(), front_face_name.to_owned()]
        }
        StandaloneOracleAnnotationKind::MeldPartner { partner_name } => {
            vec!["meld-partner/v1".to_owned(), partner_name.to_owned()]
        }
        StandaloneOracleAnnotationKind::MissingManaCostRestriction(wording) => {
            vec![format!("missing-mana-cost/v1/{wording:?}")]
        }
        StandaloneOracleAnnotationKind::CounterMarker { counter } => {
            vec![format!("counter-marker/v1/{counter:?}")]
        }
        StandaloneOracleAnnotationKind::PoisonLossThreshold { counters } => {
            vec![format!("poison-loss-threshold/v1/{counters}")]
        }
        StandaloneOracleAnnotationKind::CreatureSizeClassification {
            small_maximum_total,
            medium_minimum_total,
            medium_maximum_total,
            large_minimum_total,
        } => vec![format!(
            "creature-size/v1/{small_maximum_total}/{medium_minimum_total}/{medium_maximum_total}/{large_minimum_total}"
        )],
        StandaloneOracleAnnotationKind::LandCreatureCharacteristics(program) => {
            vec![format!("land-creature-characteristics/v1/{program:?}")]
        }
        StandaloneOracleAnnotationKind::FlashLikePermission { broader_card_types } => {
            vec![format!("flash-like-permission/v1/{broader_card_types}")]
        }
        StandaloneOracleAnnotationKind::EnumeratedExternalProcedure {
            current_count,
            die_sides,
        } => vec![format!(
            "enumerated-external-procedure/v1/{current_count}/{die_sides}"
        )],
        StandaloneOracleAnnotationKind::InfinitePower => {
            vec!["infinite-power/v1".to_owned()]
        }
        StandaloneOracleAnnotationKind::DoubleFacedSubstitute => {
            vec!["double-faced-substitute/v1".to_owned()]
        }
        StandaloneOracleAnnotationKind::BackFaceCastRestriction => {
            vec!["back-face-cast-restriction/v1".to_owned()]
        }
        StandaloneOracleAnnotationKind::LandDropAlternativeCost { mana, color } => vec![
            "land-drop-alternative-cost/v1".to_owned(),
            mana.to_owned(),
            format!("{color:?}"),
        ],
    }
}

fn english_or_numeric_u16(text: &str) -> Option<u16> {
    match text.trim() {
        "one" => Some(1),
        "two" => Some(2),
        other => other.parse().ok(),
    }
}

fn parse_roman_chapter(text: &str) -> Option<u16> {
    match text {
        "I" => Some(1),
        "II" => Some(2),
        "III" => Some(3),
        "IV" => Some(4),
        "V" => Some(5),
        "VI" => Some(6),
        _ => None,
    }
}
