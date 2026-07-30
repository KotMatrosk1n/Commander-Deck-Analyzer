//! Generic live inputs for printed characteristic values.
//!
//! Printed `*`, `?`, augment deltas, and nonnumeric loyalty values are not
//! constants. This module compiles the Oracle definition into a typed
//! expression whose leaves are public game-state queries, choices, or random
//! results. The expression remains exact while the underlying state changes.

use crate::characteristic_oracle_runtime::{
    CharacteristicColor, ExactRational, LoyaltyInitializationProcedure, PrintedStatProcedure,
};

pub(crate) const DYNAMIC_CHARACTERISTIC_RUNTIME_VERSION: &str = "dynamic-characteristic-runtime/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicCharacteristicSubject {
    Power,
    Toughness,
    Loyalty,
}

impl DynamicCharacteristicSubject {
    fn tag(self) -> &'static str {
        match self {
            Self::Power => "power",
            Self::Toughness => "toughness",
            Self::Loyalty => "loyalty",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicPlayerScope {
    Controller,
    TargetOpponent,
    // Retained in the player-scope vocabulary; direct opponent references currently use TargetOpponent.
    #[allow(dead_code)]
    ChosenOpponent,
    // Retained in the player-scope vocabulary; defending-player values currently use zone queries.
    #[allow(dead_code)]
    DefendingPlayer,
    OpponentWithMost,
    // Retained in the player-scope vocabulary; multiplayer aggregates currently use zone queries.
    #[allow(dead_code)]
    Opponents,
    // Retained in the player-scope vocabulary; table-wide aggregates currently use zone queries.
    #[allow(dead_code)]
    AllPlayers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicZoneScope {
    ControllerBattlefield,
    OpponentBattlefields,
    ChosenPlayerBattlefield,
    DefendingPlayerBattlefield,
    AllBattlefields,
    ControllerGraveyard,
    OpponentGraveyards,
    ChosenPlayerGraveyard,
    AllGraveyards,
    ControllerExile,
    ControllerGraveyardAndExile,
    LinkedExile,
    CraftExile,
    ControllerHand,
    OpponentHands,
    ChosenPlayerHand,
    AllHands,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicQueryAggregate {
    Count,
    DistinctNames,
    DistinctCardTypes,
    DistinctSubtypes,
    DistinctColors,
    DistinctBasicLandTypes,
    SumManaValue,
    GreatestManaValue,
    SumPower,
    SumToughness,
    // Retained in the query vocabulary; colored symbol totals currently use Devotion sources.
    #[allow(dead_code)]
    ManaSymbolCount(CharacteristicColor),
    // Retained in the query vocabulary; counter totals currently use PermanentCounters sources.
    #[allow(dead_code)]
    CounterCount,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DynamicOracleQuery {
    pub zone: DynamicZoneScope,
    pub aggregate: DynamicQueryAggregate,
    /// Canonical noun phrase retained for the generic object predicate engine.
    /// It never contains a card name and is interpreted against live objects.
    pub predicate: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicChoiceReason {
    PrintedSize,
    CastX,
    LifePaid,
    PermanentsSacrificed,
    // Retained in the choice vocabulary; numeric characteristic choices do not currently select types.
    #[allow(dead_code)]
    CreatureType,
    // Retained in the choice vocabulary; numeric characteristic choices do not currently select colors.
    #[allow(dead_code)]
    Color,
    // Retained in the choice vocabulary; numeric characteristic choices do not currently select players.
    #[allow(dead_code)]
    Player,
    DeckConstructionNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicRandomReason {
    PrintedSize,
    PrintedLoyalty,
    // Retained in the random-result vocabulary; coin outcomes currently use their mechanic runtime.
    #[allow(dead_code)]
    CoinResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicLinkedMetric {
    ManaValue,
    Power,
    Toughness,
    Colors,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicCopySource {
    ChosenDeckConstructionCard,
    OtherCommander,
    ChosenCommander,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicValueSource {
    Query(DynamicOracleQuery),
    LifeTotal(DynamicPlayerScope),
    // Retained for state resolution; hand-size phrases currently compile as zone queries.
    #[allow(dead_code)]
    HandSize(DynamicPlayerScope),
    Devotion(CharacteristicColor),
    PermanentCounters {
        kind: String,
        scope: String,
    },
    Choice {
        reason: DynamicChoiceReason,
        minimum: i16,
        maximum: Option<i16>,
        ordinal: u8,
    },
    ChoiceFromValues {
        reason: DynamicChoiceReason,
        values: Vec<ExactRational>,
    },
    RandomRange {
        reason: DynamicRandomReason,
        minimum: i16,
        maximum: i16,
        ordinal: u8,
    },
    Dice {
        reason: DynamicRandomReason,
        count: u8,
        sides: u8,
        modifier: i16,
        ordinal: u8,
    },
    Speed,
    PartySize,
    CardsDrawnThisTurn,
    StuckTogetherCount,
    TurnsTaken,
    CurrentHour,
    UserHeightFeet,
    UserShoeSize,
    GreatestNotedManaValue,
    LinkedValue(DynamicLinkedMetric),
    CopyCharacteristic {
        source: DynamicCopySource,
        subject: DynamicCharacteristicSubject,
    },
    AugmentHostCharacteristic(DynamicCharacteristicSubject),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DynamicValueExpression {
    Constant(ExactRational),
    Source(DynamicValueSource),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(i16, Box<Self>),
    HalfRoundedUp(Box<Self>),
    Square(Box<Self>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DynamicRuntimeValue {
    pub minimum: Option<ExactRational>,
    pub maximum: Option<ExactRational>,
}

impl DynamicRuntimeValue {
    pub(crate) fn exact(value: ExactRational) -> Self {
        Self {
            minimum: Some(value),
            maximum: Some(value),
        }
    }

    pub(crate) fn bounded(minimum: ExactRational, maximum: ExactRational) -> Option<Self> {
        rational_less_than_or_equal(minimum, maximum).then_some(Self {
            minimum: Some(minimum),
            maximum: Some(maximum),
        })
    }

    pub(crate) fn nonnegative_unbounded() -> Self {
        Self {
            minimum: Some(ExactRational::from_integer(0)),
            maximum: None,
        }
    }

    pub(crate) fn exact_value(self) -> Option<ExactRational> {
        (self.minimum == self.maximum)
            .then_some(self.minimum)
            .flatten()
    }

    pub(crate) fn conservative_integer(self) -> Option<i32> {
        let minimum = self.minimum?;
        (minimum.denominator == 1)
            .then(|| i32::try_from(minimum.numerator).ok())
            .flatten()
    }
}

impl DynamicValueExpression {
    fn canonical_payload(&self) -> String {
        match self {
            Self::Constant(value) => {
                format!("constant={}/{}", value.numerator, value.denominator)
            }
            Self::Source(source) => format!("source={source:?}"),
            Self::Add(left, right) => format!(
                "add=({})+({})",
                left.canonical_payload(),
                right.canonical_payload()
            ),
            Self::Subtract(left, right) => format!(
                "subtract=({})-({})",
                left.canonical_payload(),
                right.canonical_payload()
            ),
            Self::Multiply(multiplier, value) => {
                format!("multiply={multiplier}*({})", value.canonical_payload())
            }
            Self::HalfRoundedUp(value) => {
                format!("half-rounded-up=({})", value.canonical_payload())
            }
            Self::Square(value) => format!("square=({})", value.canonical_payload()),
        }
    }

    fn evaluate<R: DynamicCharacteristicState>(&self, state: &R) -> Option<DynamicRuntimeValue> {
        match self {
            Self::Constant(value) => Some(DynamicRuntimeValue::exact(*value)),
            Self::Source(source) => state.resolve_dynamic_characteristic_source(source),
            Self::Add(left, right) => {
                add_runtime_values(left.evaluate(state)?, right.evaluate(state)?)
            }
            Self::Subtract(left, right) => {
                subtract_runtime_values(left.evaluate(state)?, right.evaluate(state)?)
            }
            Self::Multiply(multiplier, value) => {
                multiply_runtime_value(i64::from(*multiplier), value.evaluate(state)?)
            }
            Self::HalfRoundedUp(value) => half_rounded_up_runtime(value.evaluate(state)?),
            Self::Square(value) => square_runtime_value(value.evaluate(state)?),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DynamicCharacteristicProcedure {
    pub subject: DynamicCharacteristicSubject,
    pub expression: DynamicValueExpression,
    pub oracle_definition: String,
}

impl DynamicCharacteristicProcedure {
    pub(crate) fn evaluate<R: DynamicCharacteristicState>(
        &self,
        state: &R,
    ) -> Option<DynamicRuntimeValue> {
        self.expression.evaluate(state)
    }

    pub(crate) fn canonical_evidence_payload(&self) -> String {
        format!(
            "dynamic-{}:{}:oracle={}",
            self.subject.tag(),
            self.expression.canonical_payload(),
            self.oracle_definition
        )
    }
}

pub(crate) trait DynamicCharacteristicState {
    fn resolve_dynamic_characteristic_source(
        &self,
        source: &DynamicValueSource,
    ) -> Option<DynamicRuntimeValue>;
}

pub(crate) fn compile_dynamic_printed_stat_procedure(
    layout: &str,
    oracle_text: &str,
    printed: PrintedStatProcedure,
    subject: DynamicCharacteristicSubject,
) -> Option<DynamicCharacteristicProcedure> {
    if !matches!(
        subject,
        DynamicCharacteristicSubject::Power | DynamicCharacteristicSubject::Toughness
    ) {
        return None;
    }
    if let PrintedStatProcedure::AugmentDelta(delta) = printed {
        return Some(DynamicCharacteristicProcedure {
            subject,
            expression: DynamicValueExpression::Add(
                Box::new(DynamicValueExpression::Source(
                    DynamicValueSource::AugmentHostCharacteristic(subject),
                )),
                Box::new(DynamicValueExpression::Constant(delta)),
            ),
            oracle_definition: format!("augment:{}", layout.trim().to_ascii_lowercase()),
        });
    }
    printed.required_variable()?;

    let normalized = normalize_oracle(oracle_text);
    let expression = compile_special_printed_stat(&normalized, subject)
        .or_else(|| compile_oracle_equal_to_expression(&normalized, subject))?;
    Some(DynamicCharacteristicProcedure {
        subject,
        expression,
        oracle_definition: relevant_oracle_definition(&normalized, subject),
    })
}

pub(crate) fn compile_dynamic_loyalty_procedure(
    oracle_text: &str,
    printed: LoyaltyInitializationProcedure,
) -> Option<DynamicCharacteristicProcedure> {
    let normalized = normalize_oracle(oracle_text);
    let expression = match printed {
        LoyaltyInitializationProcedure::Fixed(_) => return None,
        LoyaltyInitializationProcedure::PaidX => {
            DynamicValueExpression::Source(DynamicValueSource::Choice {
                reason: DynamicChoiceReason::CastX,
                minimum: 0,
                maximum: None,
                ordinal: 1,
            })
        }
        LoyaltyInitializationProcedure::Dice {
            count,
            sides,
            modifier,
        } => DynamicValueExpression::Source(DynamicValueSource::Dice {
            reason: DynamicRandomReason::PrintedLoyalty,
            count,
            sides,
            modifier,
            ordinal: 1,
        }),
        LoyaltyInitializationProcedure::OracleDefined => {
            let rhs = extract_after_any(
                &normalized,
                &[
                    "enters with a number of loyalty counters on it equal to ",
                    "number of loyalty counters on it is equal to ",
                    "number of loyalty counters on this permanent is equal to ",
                    "number of loyalty counters on ",
                ],
            )?;
            let rhs = if let Some((_, value)) = rhs.split_once(" is equal to ") {
                value
            } else {
                rhs
            };
            compile_value_phrase(rhs, &normalized, DynamicCharacteristicSubject::Loyalty)?
        }
    };
    Some(DynamicCharacteristicProcedure {
        subject: DynamicCharacteristicSubject::Loyalty,
        expression,
        oracle_definition: relevant_oracle_definition(
            &normalized,
            DynamicCharacteristicSubject::Loyalty,
        ),
    })
}

fn compile_special_printed_stat(
    oracle: &str,
    subject: DynamicCharacteristicSubject,
) -> Option<DynamicValueExpression> {
    if oracle.contains("copy of your other commander") {
        return Some(DynamicValueExpression::Source(
            DynamicValueSource::CopyCharacteristic {
                source: DynamicCopySource::OtherCommander,
                subject,
            },
        ));
    }
    if oracle.contains("copy of any of your commanders") {
        return Some(DynamicValueExpression::Source(
            DynamicValueSource::CopyCharacteristic {
                source: DynamicCopySource::ChosenCommander,
                subject,
            },
        ));
    }
    if oracle.contains("name a nephilim card originally printed")
        && oracle.contains("is a copy of that card")
    {
        return Some(DynamicValueExpression::Source(
            DynamicValueSource::CopyCharacteristic {
                source: DynamicCopySource::ChosenDeckConstructionCard,
                subject,
            },
        ));
    }

    let pairs = printed_size_pairs(oracle);
    if !pairs.is_empty()
        && (oracle.contains("becomes your choice")
            || oracle.contains("enters as a ")
            || oracle.contains("size ")
            || oracle.contains("stage 1 ")
            || oracle.contains("has base power and toughness"))
    {
        let values = pairs
            .into_iter()
            .map(|(power, toughness)| {
                if subject == DynamicCharacteristicSubject::Power {
                    power
                } else {
                    toughness
                }
            })
            .collect::<Vec<_>>();
        return Some(DynamicValueExpression::Source(
            DynamicValueSource::ChoiceFromValues {
                reason: DynamicChoiceReason::PrintedSize,
                values,
            },
        ));
    }

    if oracle.contains("roll a six-sided die twice")
        && oracle.contains("base power")
        && oracle.contains("base toughness")
    {
        return Some(DynamicValueExpression::Source(DynamicValueSource::Dice {
            reason: DynamicRandomReason::PrintedSize,
            count: 1,
            sides: 6,
            modifier: 0,
            ordinal: if subject == DynamicCharacteristicSubject::Power {
                1
            } else {
                2
            },
        }));
    }
    if oracle.contains("choose two numbers from 3 to 7 at random") {
        return Some(DynamicValueExpression::Source(
            DynamicValueSource::RandomRange {
                reason: DynamicRandomReason::PrintedSize,
                minimum: 3,
                maximum: 7,
                ordinal: if subject == DynamicCharacteristicSubject::Power {
                    1
                } else {
                    2
                },
            },
        ));
    }
    if oracle.contains("choose a number between 0 and 7") {
        let choice = DynamicValueExpression::Source(DynamicValueSource::Choice {
            reason: DynamicChoiceReason::PrintedSize,
            minimum: 0,
            maximum: Some(7),
            ordinal: 1,
        });
        return Some(if subject == DynamicCharacteristicSubject::Power {
            choice
        } else {
            DynamicValueExpression::Subtract(
                Box::new(DynamicValueExpression::Constant(
                    ExactRational::from_integer(7),
                )),
                Box::new(choice),
            )
        });
    }
    if oracle.contains("during deckbuilding, choose a number from one to nine") {
        return Some(DynamicValueExpression::Source(DynamicValueSource::Choice {
            reason: DynamicChoiceReason::DeckConstructionNumber,
            minimum: 1,
            maximum: Some(9),
            ordinal: 1,
        }));
    }
    None
}

fn compile_oracle_equal_to_expression(
    oracle: &str,
    subject: DynamicCharacteristicSubject,
) -> Option<DynamicValueExpression> {
    let rhs = extract_printed_stat_rhs(oracle, subject)?;
    let trimmed_rhs = trim_value_phrase(rhs);
    let expression = if subject == DynamicCharacteristicSubject::Toughness
        && trimmed_rhs.starts_with("that number plus ")
    {
        let power =
            compile_oracle_equal_to_expression(oracle, DynamicCharacteristicSubject::Power)?;
        let offset = parse_leading_integer(trimmed_rhs.strip_prefix("that number plus ")?)?;
        DynamicValueExpression::Add(
            Box::new(power),
            Box::new(DynamicValueExpression::Constant(
                ExactRational::from_integer(i64::from(offset)),
            )),
        )
    } else {
        compile_value_phrase(trimmed_rhs, oracle, subject)?
    };
    Some(expression)
}

fn extract_printed_stat_rhs(oracle: &str, subject: DynamicCharacteristicSubject) -> Option<&str> {
    const COMBINED: [&str; 3] = [
        "power and toughness are each equal to ",
        "power and toughness each equal to ",
        "power and toughness equal to ",
    ];
    let earliest = |markers: &[&str]| {
        markers
            .iter()
            .filter_map(|marker| oracle.find(marker).map(|index| (index, marker.len())))
            .min_by_key(|(index, _)| *index)
    };
    match subject {
        DynamicCharacteristicSubject::Power => {
            let mut markers = vec!["power is equal to "];
            markers.extend(COMBINED);
            let (index, marker_length) = earliest(&markers)?;
            Some(&oracle[index + marker_length..])
        }
        DynamicCharacteristicSubject::Toughness => {
            let combined = earliest(&COMBINED);
            let explicit = oracle
                .find("toughness is equal to ")
                .map(|index| (index, "toughness is equal to ".len()));
            let selected = match (combined, explicit) {
                (Some(combined), Some(explicit)) => {
                    let combined_sentence_end = oracle[combined.0..]
                        .find(". ")
                        .map(|offset| combined.0 + offset)
                        .unwrap_or(oracle.len());
                    if explicit.0 < combined_sentence_end {
                        explicit
                    } else {
                        combined
                    }
                }
                (Some(value), None) | (None, Some(value)) => value,
                (None, None) => return None,
            };
            Some(&oracle[selected.0 + selected.1..])
        }
        DynamicCharacteristicSubject::Loyalty => None,
    }
}

fn compile_value_phrase(
    source: &str,
    oracle: &str,
    subject: DynamicCharacteristicSubject,
) -> Option<DynamicValueExpression> {
    let phrase = trim_value_phrase(source);
    if phrase.is_empty() {
        return None;
    }
    if let Some(rest) = phrase.strip_prefix("half ") {
        let rest = rest.strip_suffix(", rounded up").unwrap_or(rest);
        return Some(DynamicValueExpression::HalfRoundedUp(Box::new(
            compile_value_phrase(rest, oracle, subject)?,
        )));
    }
    if let Some(rest) = phrase.strip_prefix("twice ") {
        return Some(DynamicValueExpression::Multiply(
            2,
            Box::new(compile_value_phrase(rest, oracle, subject)?),
        ));
    }
    if let Some(rest) = phrase.strip_prefix("the square of ") {
        return Some(DynamicValueExpression::Square(Box::new(
            compile_value_phrase(rest, oracle, subject)?,
        )));
    }
    if let Some((left, right)) = split_formula_operator(phrase, " plus ") {
        return Some(DynamicValueExpression::Add(
            Box::new(compile_value_phrase(left, oracle, subject)?),
            Box::new(compile_value_phrase(right, oracle, subject)?),
        ));
    }
    if let Some((left, right)) = split_formula_operator(phrase, " minus ") {
        return Some(DynamicValueExpression::Subtract(
            Box::new(compile_value_phrase(left, oracle, subject)?),
            Box::new(compile_value_phrase(right, oracle, subject)?),
        ));
    }
    if let Some(value) = parse_leading_integer(phrase)
        && value.to_string() == phrase
    {
        return Some(DynamicValueExpression::Constant(
            ExactRational::from_integer(i64::from(value)),
        ));
    }

    match phrase {
        "your life total" => Some(DynamicValueExpression::Source(
            DynamicValueSource::LifeTotal(DynamicPlayerScope::Controller),
        )),
        "the life total of target opponent" => Some(DynamicValueExpression::Source(
            DynamicValueSource::LifeTotal(DynamicPlayerScope::TargetOpponent),
        )),
        "the life total of an opponent with the most life"
        | "the highest life total among your opponents"
        | "the highest life total among players" => Some(DynamicValueExpression::Source(
            DynamicValueSource::LifeTotal(DynamicPlayerScope::OpponentWithMost),
        )),
        "your devotion to white" => devotion(CharacteristicColor::White),
        "your devotion to blue" => devotion(CharacteristicColor::Blue),
        "your devotion to black" => devotion(CharacteristicColor::Black),
        "your devotion to red" => devotion(CharacteristicColor::Red),
        "your devotion to green" => devotion(CharacteristicColor::Green),
        "your speed" => Some(DynamicValueExpression::Source(DynamicValueSource::Speed)),
        "the number of creatures in your party" => Some(DynamicValueExpression::Source(
            DynamicValueSource::PartySize,
        )),
        "the number of cards you've drawn this turn" => Some(DynamicValueExpression::Source(
            DynamicValueSource::CardsDrawnThisTurn,
        )),
        "the number of cards named s.n.o.t. stuck together to form it" => Some(
            DynamicValueExpression::Source(DynamicValueSource::StuckTogetherCount),
        ),
        "the number of turns you've taken this game" => Some(DynamicValueExpression::Source(
            DynamicValueSource::TurnsTaken,
        )),
        "the current hour, using the twelve-hour system" => Some(DynamicValueExpression::Source(
            DynamicValueSource::CurrentHour,
        )),
        "your height in feet" => Some(DynamicValueExpression::Source(
            DynamicValueSource::UserHeightFeet,
        )),
        "your american shoe size" => Some(DynamicValueExpression::Source(
            DynamicValueSource::UserShoeSize,
        )),
        "the greatest number noted for it this turn" => Some(DynamicValueExpression::Source(
            DynamicValueSource::GreatestNotedManaValue,
        )),
        "the exiled card's mana value" => Some(DynamicValueExpression::Source(
            DynamicValueSource::LinkedValue(DynamicLinkedMetric::ManaValue),
        )),
        "the total power of the exiled cards"
        | "the total power of the exiled cards used to craft it" => {
            Some(DynamicValueExpression::Source(
                DynamicValueSource::LinkedValue(DynamicLinkedMetric::Power),
            ))
        }
        "their total toughness" | "the total toughness of the exiled cards" => {
            Some(DynamicValueExpression::Source(
                DynamicValueSource::LinkedValue(DynamicLinkedMetric::Toughness),
            ))
        }
        "the number of colors among the exiled cards used to craft it" => {
            Some(DynamicValueExpression::Source(
                DynamicValueSource::LinkedValue(DynamicLinkedMetric::Colors),
            ))
        }
        "the life paid as it entered" => {
            Some(DynamicValueExpression::Source(DynamicValueSource::Choice {
                reason: DynamicChoiceReason::LifePaid,
                minimum: 0,
                maximum: None,
                ordinal: 1,
            }))
        }
        "the number of forests sacrificed as it entered" => {
            Some(DynamicValueExpression::Source(DynamicValueSource::Choice {
                reason: DynamicChoiceReason::PermanentsSacrificed,
                minimum: 0,
                maximum: None,
                ordinal: 1,
            }))
        }
        "the last chosen number" => {
            Some(DynamicValueExpression::Source(DynamicValueSource::Choice {
                reason: DynamicChoiceReason::PrintedSize,
                minimum: 0,
                maximum: Some(7),
                ordinal: 1,
            }))
        }
        _ => compile_count_or_aggregate_phrase(phrase, oracle, subject)
            .map(DynamicValueSource::Query)
            .map(DynamicValueExpression::Source)
            .or_else(|| compile_counter_phrase(phrase).map(DynamicValueExpression::Source)),
    }
}

fn devotion(color: CharacteristicColor) -> Option<DynamicValueExpression> {
    Some(DynamicValueExpression::Source(
        DynamicValueSource::Devotion(color),
    ))
}

fn compile_counter_phrase(phrase: &str) -> Option<DynamicValueSource> {
    let rest = phrase.strip_prefix("the number of ")?;
    let (kind, scope) = rest.split_once(" counters ")?;
    Some(DynamicValueSource::PermanentCounters {
        kind: kind.trim().to_string(),
        scope: scope.trim().to_string(),
    })
}

fn compile_count_or_aggregate_phrase(
    phrase: &str,
    _oracle: &str,
    _subject: DynamicCharacteristicSubject,
) -> Option<DynamicOracleQuery> {
    let (aggregate, noun_phrase) =
        if let Some(rest) = phrase.strip_prefix("the number of card types among ") {
            (DynamicQueryAggregate::DistinctCardTypes, rest)
        } else if let Some(rest) =
            phrase.strip_prefix("the number of different subtypes other than creature types among ")
        {
            (DynamicQueryAggregate::DistinctSubtypes, rest)
        } else if let Some(rest) = phrase.strip_prefix("the number of colors among ") {
            (DynamicQueryAggregate::DistinctColors, rest)
        } else if let Some(rest) = phrase.strip_prefix("the number of basic land types among ") {
            (DynamicQueryAggregate::DistinctBasicLandTypes, rest)
        } else if let Some(rest) = phrase.strip_prefix("the number of differently named ") {
            (DynamicQueryAggregate::DistinctNames, rest)
        } else if let Some(rest) = phrase.strip_prefix("the number of ") {
            (DynamicQueryAggregate::Count, rest)
        } else if let Some(rest) = phrase.strip_prefix("number of ") {
            (DynamicQueryAggregate::Count, rest)
        } else if let Some(rest) = phrase.strip_prefix("the total number of ") {
            (DynamicQueryAggregate::Count, rest)
        } else if let Some(rest) = phrase.strip_prefix("the total mana value of ") {
            (DynamicQueryAggregate::SumManaValue, rest)
        } else if let Some(rest) = phrase.strip_prefix("the greatest mana value among ") {
            (DynamicQueryAggregate::GreatestManaValue, rest)
        } else if let Some(rest) = phrase.strip_prefix("the total power of ") {
            (DynamicQueryAggregate::SumPower, rest)
        } else {
            let rest = phrase.strip_prefix("the total toughness of ")?;
            (DynamicQueryAggregate::SumToughness, rest)
        };
    let (zone, predicate) = classify_query_zone(noun_phrase)?;
    Some(DynamicOracleQuery {
        zone,
        aggregate,
        predicate: predicate.trim().to_string(),
    })
}

fn classify_query_zone(source: &str) -> Option<(DynamicZoneScope, String)> {
    let phrase = source.trim();
    let patterns = [
        (
            " you own in exile and in your graveyard",
            DynamicZoneScope::ControllerGraveyardAndExile,
        ),
        (
            " in your graveyard and in exile",
            DynamicZoneScope::ControllerGraveyardAndExile,
        ),
        (" in all players' hands", DynamicZoneScope::AllHands),
        (
            " in the hand of the opponent with the most cards in hand",
            DynamicZoneScope::OpponentHands,
        ),
        (
            " in the chosen player's hand",
            DynamicZoneScope::ChosenPlayerHand,
        ),
        (" in your hand", DynamicZoneScope::ControllerHand),
        (
            " in your opponents' graveyards",
            DynamicZoneScope::OpponentGraveyards,
        ),
        (
            " in the chosen player's graveyard",
            DynamicZoneScope::ChosenPlayerGraveyard,
        ),
        (
            " in target opponent's graveyard",
            DynamicZoneScope::ChosenPlayerGraveyard,
        ),
        (" in all graveyards", DynamicZoneScope::AllGraveyards),
        (" in your graveyard", DynamicZoneScope::ControllerGraveyard),
        (" you own in exile", DynamicZoneScope::ControllerExile),
        (" exiled with it", DynamicZoneScope::LinkedExile),
        (
            " among the exiled cards used to craft it",
            DynamicZoneScope::CraftExile,
        ),
        (" used to craft it", DynamicZoneScope::CraftExile),
        (
            " your opponents control",
            DynamicZoneScope::OpponentBattlefields,
        ),
        (
            " the chosen player controls",
            DynamicZoneScope::ChosenPlayerBattlefield,
        ),
        (
            " defending player controls",
            DynamicZoneScope::DefendingPlayerBattlefield,
        ),
        (" you control", DynamicZoneScope::ControllerBattlefield),
        (" on the battlefield", DynamicZoneScope::AllBattlefields),
    ];
    for (suffix, zone) in patterns {
        if let Some(predicate) = phrase.strip_suffix(suffix) {
            return Some((zone, predicate.to_string()));
        }
    }
    let infix_patterns = [
        (
            " you control that are ",
            DynamicZoneScope::ControllerBattlefield,
        ),
        (
            " you control with ",
            DynamicZoneScope::ControllerBattlefield,
        ),
        (
            " on the battlefield with ",
            DynamicZoneScope::AllBattlefields,
        ),
        (
            " in your graveyard with ",
            DynamicZoneScope::ControllerGraveyard,
        ),
    ];
    for (infix, zone) in infix_patterns {
        if let Some((before, after)) = phrase.split_once(infix) {
            return Some((zone, format!("{before} that are {after}")));
        }
    }
    if let Some(predicate) = phrase.strip_suffix(" in your graveyards") {
        return Some((DynamicZoneScope::AllGraveyards, predicate.to_string()));
    }
    None
}

fn extract_after_any<'a>(source: &'a str, markers: &[&str]) -> Option<&'a str> {
    markers.iter().find_map(|marker| {
        source
            .find(marker)
            .map(|index| &source[index + marker.len()..])
    })
}

fn trim_value_phrase(source: &str) -> &str {
    let end = [
        first_sentence_boundary(source),
        source.find('\n'),
        source.find(" and its power is equal to "),
        source.find(" and its toughness is equal to "),
        source.find(" and toughness is equal to "),
        source.find(" during turns other than yours"),
    ]
    .into_iter()
    .flatten()
    .min()
    .unwrap_or(source.len());
    source[..end]
        .trim()
        .trim_end_matches('.')
        .trim_matches(|character| matches!(character, ',' | ';' | ':'))
}

fn first_sentence_boundary(source: &str) -> Option<usize> {
    source.match_indices(". ").find_map(|(index, _)| {
        let token_start = source[..index]
            .rfind(char::is_whitespace)
            .map_or(0, |offset| offset + 1);
        let token = &source[token_start..=index];
        (token.matches('.').count() < 2).then_some(index)
    })
}

fn split_formula_operator<'a>(source: &'a str, operator: &str) -> Option<(&'a str, &'a str)> {
    let (left, right) = source.split_once(operator)?;
    (!left.trim().is_empty() && !right.trim().is_empty()).then_some((left.trim(), right.trim()))
}

fn parse_leading_integer(source: &str) -> Option<i16> {
    let token = source.split_whitespace().next()?;
    token.parse::<i16>().ok()
}

fn normalize_oracle(source: &str) -> String {
    source
        .replace(['\r', '\n'], " ")
        .replace('\u{2019}', "'")
        .replace('\u{2212}', "-")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn relevant_oracle_definition(oracle: &str, subject: DynamicCharacteristicSubject) -> String {
    let needle = subject.tag();
    oracle
        .split('.')
        .find(|sentence| sentence.contains(needle))
        .unwrap_or(oracle)
        .trim()
        .to_string()
}

fn printed_size_pairs(oracle: &str) -> Vec<(ExactRational, ExactRational)> {
    let bytes = oracle.as_bytes();
    let mut pairs = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let power_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'/' {
            continue;
        }
        let power = oracle[power_start..index].parse::<i64>().ok();
        index += 1;
        let toughness_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if toughness_start == index {
            continue;
        }
        let toughness = oracle[toughness_start..index].parse::<i64>().ok();
        if let (Some(power), Some(toughness)) = (power, toughness) {
            let pair = (
                ExactRational::from_integer(power),
                ExactRational::from_integer(toughness),
            );
            if !pairs.contains(&pair) {
                pairs.push(pair);
            }
        }
    }
    pairs
}

fn add_rational(left: ExactRational, right: ExactRational) -> Option<ExactRational> {
    let numerator = left
        .numerator
        .checked_mul(i64::from(right.denominator))?
        .checked_add(right.numerator.checked_mul(i64::from(left.denominator))?)?;
    ExactRational::new(numerator, left.denominator.checked_mul(right.denominator)?)
}

fn subtract_rational(left: ExactRational, right: ExactRational) -> Option<ExactRational> {
    add_rational(
        left,
        ExactRational::new(right.numerator.checked_neg()?, right.denominator)?,
    )
}

fn multiply_rational(left: ExactRational, right: ExactRational) -> Option<ExactRational> {
    ExactRational::new(
        left.numerator.checked_mul(right.numerator)?,
        left.denominator.checked_mul(right.denominator)?,
    )
}

fn half_rounded_up(value: ExactRational) -> Option<ExactRational> {
    if value.numerator < 0 {
        return None;
    }
    let denominator = u64::from(value.denominator).checked_mul(2)?;
    let numerator = u64::try_from(value.numerator).ok()?;
    let rounded = numerator.checked_add(denominator.checked_sub(1)?)? / denominator;
    Some(ExactRational::from_integer(i64::try_from(rounded).ok()?))
}

fn rational_less_than_or_equal(left: ExactRational, right: ExactRational) -> bool {
    let left_cross = i128::from(left.numerator) * i128::from(right.denominator);
    let right_cross = i128::from(right.numerator) * i128::from(left.denominator);
    left_cross <= right_cross
}

fn add_runtime_values(
    left: DynamicRuntimeValue,
    right: DynamicRuntimeValue,
) -> Option<DynamicRuntimeValue> {
    Some(DynamicRuntimeValue {
        minimum: match (left.minimum, right.minimum) {
            (Some(left), Some(right)) => add_rational(left, right),
            _ => None,
        },
        maximum: match (left.maximum, right.maximum) {
            (Some(left), Some(right)) => add_rational(left, right),
            _ => None,
        },
    })
}

fn subtract_runtime_values(
    left: DynamicRuntimeValue,
    right: DynamicRuntimeValue,
) -> Option<DynamicRuntimeValue> {
    Some(DynamicRuntimeValue {
        minimum: match (left.minimum, right.maximum) {
            (Some(left), Some(right)) => subtract_rational(left, right),
            _ => None,
        },
        maximum: match (left.maximum, right.minimum) {
            (Some(left), Some(right)) => subtract_rational(left, right),
            _ => None,
        },
    })
}

fn multiply_runtime_value(
    multiplier: i64,
    value: DynamicRuntimeValue,
) -> Option<DynamicRuntimeValue> {
    let multiplier_value = ExactRational::from_integer(multiplier);
    if multiplier >= 0 {
        Some(DynamicRuntimeValue {
            minimum: value
                .minimum
                .and_then(|value| multiply_rational(multiplier_value, value)),
            maximum: value
                .maximum
                .and_then(|value| multiply_rational(multiplier_value, value)),
        })
    } else {
        Some(DynamicRuntimeValue {
            minimum: value
                .maximum
                .and_then(|value| multiply_rational(multiplier_value, value)),
            maximum: value
                .minimum
                .and_then(|value| multiply_rational(multiplier_value, value)),
        })
    }
}

fn half_rounded_up_runtime(value: DynamicRuntimeValue) -> Option<DynamicRuntimeValue> {
    Some(DynamicRuntimeValue {
        minimum: value.minimum.and_then(half_rounded_up),
        maximum: value.maximum.and_then(half_rounded_up),
    })
}

fn square_runtime_value(value: DynamicRuntimeValue) -> Option<DynamicRuntimeValue> {
    let zero = ExactRational::from_integer(0);
    let square = |value| multiply_rational(value, value);
    let minimum = match (value.minimum, value.maximum) {
        (Some(minimum), Some(maximum))
            if rational_less_than_or_equal(minimum, zero)
                && rational_less_than_or_equal(zero, maximum) =>
        {
            Some(zero)
        }
        (Some(minimum), Some(maximum)) => {
            let left = square(minimum)?;
            let right = square(maximum)?;
            Some(if rational_less_than_or_equal(left, right) {
                left
            } else {
                right
            })
        }
        (Some(minimum), None) if rational_less_than_or_equal(zero, minimum) => square(minimum),
        _ => None,
    };
    let maximum = match (value.minimum, value.maximum) {
        (Some(minimum), Some(maximum)) => {
            let left = square(minimum)?;
            let right = square(maximum)?;
            Some(if rational_less_than_or_equal(left, right) {
                right
            } else {
                left
            })
        }
        _ => None,
    };
    Some(DynamicRuntimeValue { minimum, maximum })
}
