//! Typed, fail-closed parsing for bounded Oracle mana expressions.
//!
//! This module describes source semantics only. A successfully parsed value is
//! not an execution receipt and does not grant any runtime capability.

use std::fmt;

pub const BOUNDED_ORACLE_MANA_EXPRESSION_VERSION: &str = "bounded-oracle-mana-expression-0.2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaSymbol {
    Generic(u32),
    Color(ManaColor),
    VariableX,
    Snow,
    Hybrid(ManaColor, ManaColor),
    GenericHybrid { generic: u32, color: ManaColor },
    Phyrexian(ManaColor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuantityCalculation {
    Value(CalculatedValue),
    Sum(Vec<QuantityCalculation>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalculatedValue {
    Constant(u32),
    SourcePower,
    SacrificedCreatureManaValue,
    SacrificedPermanentManaValue,
    Count(CountedObjects),
    ManaSpentToCastReferencedSpell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountedObjects {
    pub controller: CountController,
    pub zone: Option<CountZone>,
    pub card_type: Option<CountedCardType>,
    pub subtype: Option<CountedSubtype>,
    pub named_subtype: Option<String>,
    pub keyword: Option<CountedKeyword>,
    pub shares_creature_type_with: Option<CountReference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountController {
    You,
    TargetOpponent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountZone {
    Battlefield,
    Hand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountedCardType {
    Creature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountedSubtype {
    Shrine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountedKeyword {
    Defender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountReference {
    ReferencedObject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaQuantity {
    Fixed(u32),
    X {
        defined_as: Option<Box<QuantityCalculation>>,
    },
    Calculated(Box<QuantityCalculation>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaColorDomain {
    Colors,
    ManaTypes,
    Explicit(Vec<ManaColor>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedManaTypes {
    ChosenColor,
    ChosenColors,
    CommanderColorIdentity,
    ExiledCardsColors,
    SacrificedLandCouldProduce,
    ControlledLandsCouldProduce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivedManaSourceScope {
    LandsYouControl,
    LandsOpponentControls,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaComposition {
    Exact(Vec<ManaColor>),
    OneOf(Vec<Vec<ManaColor>>),
    /// Printed alternatives whose branches may use different composition
    /// families, such as a fixed color or a previously chosen color.
    Alternatives(Vec<ManaComposition>),
    AnyOneColor,
    AnyCombination(ManaColorDomain),
    DifferentColors(ManaColorDomain),
    Derived(DerivedManaTypes),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellSpendFilter {
    pub card_type: Option<SpendCardType>,
    pub from: Option<SpendZone>,
    pub minimum_mana_value: Option<u32>,
    pub mana_cost_contains_x: bool,
    pub chosen_creature_type: bool,
    pub monocolored_of_produced_color: bool,
    pub cannot_be_countered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendCardType {
    Artifact,
    Creature,
    Planeswalker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendZone {
    Graveyard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpendPurpose {
    CastSpell(SpellSpendFilter),
    ActivateAbility,
    ActivateArtifactAbility,
    ActivateCreatureAbility,
    ActivateLandSourceAbility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpendRestriction {
    Only(SpendPurpose),
    AnyOf(Vec<SpendPurpose>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionBoundary {
    EndOfTurn,
    EndOfCombat,
    NextMainPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManaRetention {
    Normal,
    Retain {
        through: RetentionBoundary,
        across_steps: bool,
        across_phases: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaProductionExpression {
    pub version: &'static str,
    pub quantity: ManaQuantity,
    pub composition: ManaComposition,
    pub derived_source_scope: Option<DerivedManaSourceScope>,
    pub spend_restriction: Option<SpendRestriction>,
    pub retention: ManaRetention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost {
    pub symbols: Vec<ManaSymbol>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceCostComponent {
    Mana(ManaCost),
    Energy(u32),
    Tickets(u32),
    TapSource,
    UntapSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCostExpression {
    pub version: &'static str,
    pub components: Vec<ResourceCostComponent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedResourceSymbolKind {
    FractionalMana,
    PlatformSpecific,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaExpressionError {
    EmptyInput,
    ExpectedManaProduction,
    InvalidQuantity {
        text: String,
    },
    UnsupportedCalculation {
        text: String,
    },
    UnsupportedComposition {
        text: String,
    },
    UnsupportedSpendRestriction {
        text: String,
    },
    UnsupportedRetention {
        text: String,
    },
    MalformedResourceSymbol {
        offset: usize,
    },
    UnsupportedResourceSymbol {
        symbol: String,
        kind: UnsupportedResourceSymbolKind,
    },
    ResourceAmountOverflow {
        symbol: String,
    },
    UnexpectedTrailingInput {
        text: String,
    },
}

impl fmt::Display for ManaExpressionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(formatter, "the mana expression is empty"),
            Self::ExpectedManaProduction => {
                write!(
                    formatter,
                    "the expression does not begin with a complete Add instruction"
                )
            }
            Self::InvalidQuantity { text } => {
                write!(formatter, "unsupported mana quantity: {text}")
            }
            Self::UnsupportedCalculation { text } => {
                write!(formatter, "unsupported mana quantity calculation: {text}")
            }
            Self::UnsupportedComposition { text } => {
                write!(formatter, "unsupported mana composition: {text}")
            }
            Self::UnsupportedSpendRestriction { text } => {
                write!(formatter, "unsupported mana spend restriction: {text}")
            }
            Self::UnsupportedRetention { text } => {
                write!(formatter, "unsupported mana retention clause: {text}")
            }
            Self::MalformedResourceSymbol { offset } => {
                write!(
                    formatter,
                    "malformed resource symbol at byte offset {offset}"
                )
            }
            Self::UnsupportedResourceSymbol { symbol, kind } => {
                write!(
                    formatter,
                    "unsupported {kind:?} resource symbol: {{{symbol}}}"
                )
            }
            Self::ResourceAmountOverflow { symbol } => {
                write!(formatter, "resource amount is too large: {{{symbol}}}")
            }
            Self::UnexpectedTrailingInput { text } => {
                write!(formatter, "unexpected trailing input: {text}")
            }
        }
    }
}

impl std::error::Error for ManaExpressionError {}

pub fn parse_mana_production_expression(
    input: &str,
) -> Result<ManaProductionExpression, ManaExpressionError> {
    let normalized = normalize(input);
    if normalized.is_empty() {
        return Err(ManaExpressionError::EmptyInput);
    }
    let sentences = split_complete_sentences(&normalized)?;
    let Some(primary) = sentences.first() else {
        return Err(ManaExpressionError::EmptyInput);
    };
    let lower_primary = primary.to_ascii_lowercase();
    let Some(production_text) = lower_primary.strip_prefix("add ") else {
        return Err(ManaExpressionError::ExpectedManaProduction);
    };
    let (production_text, x_definition) =
        if let Some((expression, definition)) = production_text.split_once(", where x is ") {
            (
                expression.trim(),
                Some(parse_calculation(definition.trim())?),
            )
        } else {
            (production_text.trim(), None)
        };
    let (quantity, composition) = parse_production_body(production_text, x_definition)?;
    let derived_source_scope = match composition {
        ManaComposition::Derived(DerivedManaTypes::ControlledLandsCouldProduce)
            if production_text.contains("a land an opponent controls could produce") =>
        {
            Some(DerivedManaSourceScope::LandsOpponentControls)
        }
        ManaComposition::Derived(DerivedManaTypes::ControlledLandsCouldProduce) => {
            Some(DerivedManaSourceScope::LandsYouControl)
        }
        _ => None,
    };

    let mut spend_restriction = None;
    let mut retention = ManaRetention::Normal;
    for sentence in sentences.iter().skip(1) {
        let lower = sentence.to_ascii_lowercase();
        if lower.starts_with("spend this mana only to ") {
            if spend_restriction.is_some() {
                return Err(ManaExpressionError::UnexpectedTrailingInput {
                    text: sentence.to_string(),
                });
            }
            spend_restriction = Some(parse_spend_restriction(&lower)?);
        } else if lower.starts_with("until ") {
            if retention != ManaRetention::Normal {
                return Err(ManaExpressionError::UnexpectedTrailingInput {
                    text: sentence.to_string(),
                });
            }
            retention = parse_retention(&lower)?;
        } else {
            return Err(ManaExpressionError::UnexpectedTrailingInput {
                text: sentence.to_string(),
            });
        }
    }

    Ok(ManaProductionExpression {
        version: BOUNDED_ORACLE_MANA_EXPRESSION_VERSION,
        quantity,
        composition,
        derived_source_scope,
        spend_restriction,
        retention,
    })
}

pub fn parse_mana_spend_restriction_clause(
    input: &str,
) -> Result<SpendRestriction, ManaExpressionError> {
    let normalized = normalize(input).to_ascii_lowercase();
    parse_spend_restriction(normalized.trim_end_matches('.'))
}

pub fn parse_mana_retention_clause(input: &str) -> Result<ManaRetention, ManaExpressionError> {
    let normalized = normalize(input).to_ascii_lowercase();
    parse_retention(normalized.trim_end_matches('.'))
}

pub fn parse_resource_cost_expression(
    input: &str,
) -> Result<ResourceCostExpression, ManaExpressionError> {
    let normalized = normalize(input);
    if normalized.is_empty() {
        return Err(ManaExpressionError::EmptyInput);
    }

    let mut components = Vec::new();
    let mut pending_mana = Vec::new();
    let mut cursor = 0usize;
    let bytes = normalized.as_bytes();
    let mut expect_symbol = true;
    while cursor < bytes.len() {
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor == bytes.len() {
            break;
        }
        if bytes[cursor] == b',' {
            if expect_symbol {
                return Err(ManaExpressionError::UnexpectedTrailingInput {
                    text: normalized[cursor..].to_string(),
                });
            }
            flush_mana(&mut components, &mut pending_mana);
            cursor += 1;
            expect_symbol = true;
            continue;
        }
        if bytes[cursor] != b'{' {
            return Err(ManaExpressionError::UnexpectedTrailingInput {
                text: normalized[cursor..].to_string(),
            });
        }
        let Some(relative_end) = normalized[cursor + 1..].find('}') else {
            return Err(ManaExpressionError::MalformedResourceSymbol { offset: cursor });
        };
        let end = cursor + 1 + relative_end;
        let symbol = &normalized[cursor + 1..end];
        if symbol.is_empty() {
            return Err(ManaExpressionError::MalformedResourceSymbol { offset: cursor });
        }
        match parse_resource_symbol(symbol)? {
            ParsedResourceSymbol::Mana(mana) => pending_mana.push(mana),
            ParsedResourceSymbol::Energy => {
                flush_mana(&mut components, &mut pending_mana);
                increment_resource(&mut components, ResourceKind::Energy)?;
            }
            ParsedResourceSymbol::Ticket => {
                flush_mana(&mut components, &mut pending_mana);
                increment_resource(&mut components, ResourceKind::Ticket)?;
            }
            ParsedResourceSymbol::Tap => {
                flush_mana(&mut components, &mut pending_mana);
                components.push(ResourceCostComponent::TapSource);
            }
            ParsedResourceSymbol::Untap => {
                flush_mana(&mut components, &mut pending_mana);
                components.push(ResourceCostComponent::UntapSource);
            }
        }
        cursor = end + 1;
        expect_symbol = false;
    }
    if expect_symbol {
        return Err(ManaExpressionError::UnexpectedTrailingInput { text: normalized });
    }
    flush_mana(&mut components, &mut pending_mana);

    Ok(ResourceCostExpression {
        version: BOUNDED_ORACLE_MANA_EXPRESSION_VERSION,
        components,
    })
}

fn parse_production_body(
    text: &str,
    x_definition: Option<QuantityCalculation>,
) -> Result<(ManaQuantity, ManaComposition), ManaExpressionError> {
    if let Some((symbol_text, calculation_text)) = text.split_once(" for each ") {
        if x_definition.is_some() {
            return Err(ManaExpressionError::InvalidQuantity {
                text: text.to_string(),
            });
        }
        let colors = parse_exact_production_symbols(symbol_text.trim())?;
        return Ok((
            ManaQuantity::Calculated(Box::new(parse_for_each_calculation(
                calculation_text.trim(),
            )?)),
            ManaComposition::Exact(colors),
        ));
    }

    if text.starts_with('{') {
        if x_definition.is_some() {
            return Err(ManaExpressionError::InvalidQuantity {
                text: text.to_string(),
            });
        }
        let choices = if text.contains(" or ") {
            text.split(" or ")
                .map(parse_exact_production_symbols)
                .collect::<Result<Vec<_>, _>>()?
        } else if text.contains(',') || text.contains(" and ") {
            vec![parse_conjoined_exact_production_symbols(text)?]
        } else {
            vec![parse_exact_production_symbols(text)?]
        };
        let Some(first) = choices.first() else {
            return Err(ManaExpressionError::InvalidQuantity {
                text: text.to_string(),
            });
        };
        let amount = u32::try_from(first.len()).map_err(|_| {
            ManaExpressionError::ResourceAmountOverflow {
                symbol: text.to_string(),
            }
        })?;
        if amount == 0 || choices.iter().any(|choice| choice.len() != first.len()) {
            return Err(ManaExpressionError::InvalidQuantity {
                text: text.to_string(),
            });
        }
        let composition = if choices.len() == 1 {
            ManaComposition::Exact(choices.into_iter().next().unwrap_or_default())
        } else {
            ManaComposition::OneOf(choices)
        };
        return Ok((ManaQuantity::Fixed(amount), composition));
    }

    if let Some(rest) = text.strip_prefix("an amount of ")
        && let Some((symbol, calculation)) = rest.split_once(" equal to ")
    {
        let colors = parse_exact_production_symbols(symbol.trim())?;
        if colors.len() != 1 {
            return Err(ManaExpressionError::UnsupportedComposition {
                text: symbol.to_string(),
            });
        }
        return Ok((
            ManaQuantity::Calculated(Box::new(parse_calculation(calculation)?)),
            ManaComposition::Exact(colors),
        ));
    }

    if let Some((quantity_text, symbol_text)) = text.split_once(' ')
        && let Some(quantity) = parse_fixed_quantity(quantity_text)
        && symbol_text.starts_with('{')
    {
        if quantity == 0 {
            return Err(ManaExpressionError::InvalidQuantity {
                text: quantity_text.to_string(),
            });
        }
        let colors = parse_exact_production_symbols(symbol_text)?;
        if colors.len() != 1 {
            return Err(ManaExpressionError::UnsupportedComposition {
                text: symbol_text.to_string(),
            });
        }
        return Ok((
            ManaQuantity::Fixed(quantity),
            ManaComposition::Exact(colors),
        ));
    }

    let Some((quantity_text, composition_text)) = text.split_once(" mana") else {
        return Err(ManaExpressionError::InvalidQuantity {
            text: text.to_string(),
        });
    };
    let quantity = if quantity_text == "x" {
        ManaQuantity::X {
            defined_as: x_definition.map(Box::new),
        }
    } else {
        if x_definition.is_some() {
            return Err(ManaExpressionError::InvalidQuantity {
                text: text.to_string(),
            });
        }
        ManaQuantity::Fixed(parse_fixed_quantity(quantity_text).ok_or_else(|| {
            ManaExpressionError::InvalidQuantity {
                text: quantity_text.to_string(),
            }
        })?)
    };
    let composition = parse_composition(composition_text.trim())?;
    Ok((quantity, composition))
}

fn parse_composition(text: &str) -> Result<ManaComposition, ManaExpressionError> {
    match text {
        "of any color" | "of any one color" => Ok(ManaComposition::AnyOneColor),
        "in any combination of colors" => {
            Ok(ManaComposition::AnyCombination(ManaColorDomain::Colors))
        }
        "in any combination of mana types" => {
            Ok(ManaComposition::AnyCombination(ManaColorDomain::ManaTypes))
        }
        "of different colors" => Ok(ManaComposition::DifferentColors(ManaColorDomain::Colors)),
        "of the chosen color" => Ok(ManaComposition::Derived(DerivedManaTypes::ChosenColor)),
        "of any of the chosen colors" => {
            Ok(ManaComposition::Derived(DerivedManaTypes::ChosenColors))
        }
        "of any color in your commander's color identity" => Ok(ManaComposition::Derived(
            DerivedManaTypes::CommanderColorIdentity,
        )),
        "of any of the exiled cards' colors" => Ok(ManaComposition::Derived(
            DerivedManaTypes::ExiledCardsColors,
        )),
        "of any type the sacrificed land could produce" => Ok(ManaComposition::Derived(
            DerivedManaTypes::SacrificedLandCouldProduce,
        )),
        "of any type that a land you control could produce" => Ok(ManaComposition::Derived(
            DerivedManaTypes::ControlledLandsCouldProduce,
        )),
        "of any color that a land an opponent controls could produce"
        | "of any type that a land an opponent controls could produce" => Ok(
            ManaComposition::Derived(DerivedManaTypes::ControlledLandsCouldProduce),
        ),
        _ => {
            if let Some(list) = text.strip_prefix("in any combination of ") {
                let colors = parse_explicit_color_list(list)?;
                return Ok(ManaComposition::AnyCombination(ManaColorDomain::Explicit(
                    colors,
                )));
            }
            if text.starts_with("of {") {
                let colors =
                    parse_exact_production_symbols(text.strip_prefix("of ").unwrap_or_default())?;
                if colors.len() == 1 {
                    return Ok(ManaComposition::Exact(colors));
                }
            }
            Err(ManaExpressionError::UnsupportedComposition {
                text: text.to_string(),
            })
        }
    }
}

fn parse_explicit_color_list(text: &str) -> Result<Vec<ManaColor>, ManaExpressionError> {
    let parts = if let Some((leading, final_color)) = text.rsplit_once(", and/or ") {
        let mut parts = leading.split(", ").collect::<Vec<_>>();
        if parts.len() < 2
            || parts.iter().any(|part| part.is_empty())
            || final_color.is_empty()
            || final_color.contains(',')
            || final_color.contains(" and/or ")
        {
            return Err(ManaExpressionError::UnsupportedComposition {
                text: text.to_string(),
            });
        }
        parts.push(final_color);
        parts
    } else if let Some((first, second)) = text.split_once(" and/or ") {
        if first.is_empty()
            || second.is_empty()
            || first.contains(',')
            || second.contains(',')
            || second.contains(" and/or ")
        {
            return Err(ManaExpressionError::UnsupportedComposition {
                text: text.to_string(),
            });
        }
        vec![first, second]
    } else {
        return Err(ManaExpressionError::UnsupportedComposition {
            text: text.to_string(),
        });
    };
    let mut colors = Vec::new();
    for part in parts {
        let parsed = parse_exact_production_symbols(part)?;
        if parsed.len() != 1 {
            return Err(ManaExpressionError::UnsupportedComposition {
                text: text.to_string(),
            });
        }
        let color = parsed[0];
        if colors.contains(&color) {
            return Err(ManaExpressionError::UnsupportedComposition {
                text: text.to_string(),
            });
        }
        colors.push(color);
    }
    if colors.len() < 2 {
        return Err(ManaExpressionError::UnsupportedComposition {
            text: text.to_string(),
        });
    }
    Ok(colors)
}

fn parse_conjoined_exact_production_symbols(
    text: &str,
) -> Result<Vec<ManaColor>, ManaExpressionError> {
    let normalized = text.replace(", and ", ", ").replace(" and ", ", ");
    let parts = normalized.split(", ").collect::<Vec<_>>();
    if parts.len() < 2 || parts.iter().any(|part| part.trim().is_empty()) {
        return Err(ManaExpressionError::UnsupportedComposition {
            text: text.to_string(),
        });
    }
    let mut colors = Vec::new();
    for part in parts {
        colors.extend(parse_exact_production_symbols(part.trim())?);
    }
    if colors.is_empty() {
        return Err(ManaExpressionError::UnsupportedComposition {
            text: text.to_string(),
        });
    }
    Ok(colors)
}

fn parse_exact_production_symbols(text: &str) -> Result<Vec<ManaColor>, ManaExpressionError> {
    let mut colors = Vec::new();
    let mut cursor = 0usize;
    let bytes = text.as_bytes();
    while cursor < bytes.len() {
        if bytes[cursor] != b'{' {
            return Err(ManaExpressionError::UnsupportedComposition {
                text: text.to_string(),
            });
        }
        let Some(relative_end) = text[cursor + 1..].find('}') else {
            return Err(ManaExpressionError::MalformedResourceSymbol { offset: cursor });
        };
        let end = cursor + 1 + relative_end;
        let symbol = &text[cursor + 1..end];
        let color = parse_color(symbol).ok_or_else(|| {
            classify_unsupported_symbol(symbol).unwrap_or_else(|| {
                ManaExpressionError::UnsupportedComposition {
                    text: text.to_string(),
                }
            })
        })?;
        colors.push(color);
        cursor = end + 1;
    }
    if colors.is_empty() {
        return Err(ManaExpressionError::UnsupportedComposition {
            text: text.to_string(),
        });
    }
    Ok(colors)
}

fn parse_for_each_calculation(text: &str) -> Result<QuantityCalculation, ManaExpressionError> {
    let objects = match text {
        "card in target opponent's hand" => CountedObjects {
            controller: CountController::TargetOpponent,
            zone: Some(CountZone::Hand),
            card_type: None,
            subtype: None,
            named_subtype: None,
            keyword: None,
            shares_creature_type_with: None,
        },
        "berserker you control" => CountedObjects {
            controller: CountController::You,
            zone: Some(CountZone::Battlefield),
            card_type: Some(CountedCardType::Creature),
            subtype: None,
            named_subtype: Some("Berserker".to_string()),
            keyword: None,
            shares_creature_type_with: None,
        },
        _ => {
            return Err(ManaExpressionError::UnsupportedCalculation {
                text: text.to_string(),
            });
        }
    };
    Ok(QuantityCalculation::Value(CalculatedValue::Count(objects)))
}

fn parse_calculation(text: &str) -> Result<QuantityCalculation, ManaExpressionError> {
    if let Some((left, right)) = text.split_once(" plus ") {
        let terms = vec![
            parse_calculation(left.trim())?,
            parse_calculation(right.trim())?,
        ];
        return Ok(QuantityCalculation::Sum(terms));
    }
    let value = match text.trim() {
        "this object's power" => CalculatedValue::SourcePower,
        "the sacrificed creature's mana value" => CalculatedValue::SacrificedCreatureManaValue,
        "the sacrificed permanent's mana value" => CalculatedValue::SacrificedPermanentManaValue,
        "the number of creatures you control with defender" => {
            CalculatedValue::Count(CountedObjects {
                controller: CountController::You,
                zone: Some(CountZone::Battlefield),
                card_type: Some(CountedCardType::Creature),
                subtype: None,
                named_subtype: None,
                keyword: Some(CountedKeyword::Defender),
                shares_creature_type_with: None,
            })
        }
        "the number of shrines you control" => CalculatedValue::Count(CountedObjects {
            controller: CountController::You,
            zone: Some(CountZone::Battlefield),
            card_type: None,
            subtype: Some(CountedSubtype::Shrine),
            named_subtype: None,
            keyword: None,
            shares_creature_type_with: None,
        }),
        "the number of creatures you control that share a creature type with it" => {
            CalculatedValue::Count(CountedObjects {
                controller: CountController::You,
                zone: Some(CountZone::Battlefield),
                card_type: Some(CountedCardType::Creature),
                subtype: None,
                named_subtype: None,
                keyword: None,
                shares_creature_type_with: Some(CountReference::ReferencedObject),
            })
        }
        "the amount of mana spent to cast that spell" => {
            CalculatedValue::ManaSpentToCastReferencedSpell
        }
        other => {
            if let Some(fixed) = parse_fixed_quantity(other) {
                return Ok(QuantityCalculation::Value(CalculatedValue::Constant(fixed)));
            }
            return Err(ManaExpressionError::UnsupportedCalculation {
                text: text.to_string(),
            });
        }
    };
    Ok(QuantityCalculation::Value(value))
}

fn parse_spend_restriction(text: &str) -> Result<SpendRestriction, ManaExpressionError> {
    let body = text
        .strip_prefix("spend this mana only to ")
        .unwrap_or_default();
    if let Some((minimum_text, x_text)) = body
        .strip_prefix("cast creature spells with mana value ")
        .and_then(|body| {
            body.split_once(" or greater or creature spells with {x} in their mana costs")
        })
    {
        let minimum = minimum_text.parse::<u32>().map_err(|_| {
            ManaExpressionError::UnsupportedSpendRestriction {
                text: text.to_string(),
            }
        })?;
        if !x_text.is_empty() {
            return Err(ManaExpressionError::UnsupportedSpendRestriction {
                text: text.to_string(),
            });
        }
        return Ok(SpendRestriction::AnyOf(vec![
            SpendPurpose::CastSpell(SpellSpendFilter {
                card_type: Some(SpendCardType::Creature),
                from: None,
                minimum_mana_value: Some(minimum),
                mana_cost_contains_x: false,
                chosen_creature_type: false,
                monocolored_of_produced_color: false,
                cannot_be_countered: false,
            }),
            SpendPurpose::CastSpell(SpellSpendFilter {
                card_type: Some(SpendCardType::Creature),
                from: None,
                minimum_mana_value: None,
                mana_cost_contains_x: true,
                chosen_creature_type: false,
                monocolored_of_produced_color: false,
                cannot_be_countered: false,
            }),
        ]));
    }
    let purpose = match body {
        "cast a creature spell" | "cast creature spells" => {
            SpendPurpose::CastSpell(SpellSpendFilter {
                card_type: Some(SpendCardType::Creature),
                from: None,
                minimum_mana_value: None,
                mana_cost_contains_x: false,
                chosen_creature_type: false,
                monocolored_of_produced_color: false,
                cannot_be_countered: false,
            })
        }
        "cast a planeswalker spell" | "cast planeswalker spells" => {
            SpendPurpose::CastSpell(SpellSpendFilter {
                card_type: Some(SpendCardType::Planeswalker),
                from: None,
                minimum_mana_value: None,
                mana_cost_contains_x: false,
                chosen_creature_type: false,
                monocolored_of_produced_color: false,
                cannot_be_countered: false,
            })
        }
        "cast spells from your graveyard" => SpendPurpose::CastSpell(SpellSpendFilter {
            card_type: None,
            from: Some(SpendZone::Graveyard),
            minimum_mana_value: None,
            mana_cost_contains_x: false,
            chosen_creature_type: false,
            monocolored_of_produced_color: false,
            cannot_be_countered: false,
        }),
        "cast a creature spell of the chosen type, and that spell can't be countered" => {
            SpendPurpose::CastSpell(SpellSpendFilter {
                card_type: Some(SpendCardType::Creature),
                from: None,
                minimum_mana_value: None,
                mana_cost_contains_x: false,
                chosen_creature_type: true,
                monocolored_of_produced_color: false,
                cannot_be_countered: true,
            })
        }
        "cast monocolored spells of that color" => SpendPurpose::CastSpell(SpellSpendFilter {
            card_type: None,
            from: None,
            minimum_mana_value: None,
            mana_cost_contains_x: false,
            chosen_creature_type: false,
            monocolored_of_produced_color: true,
            cannot_be_countered: false,
        }),
        "activate abilities" => SpendPurpose::ActivateAbility,
        "activate abilities of artifacts" => SpendPurpose::ActivateArtifactAbility,
        "activate abilities of creatures" => SpendPurpose::ActivateCreatureAbility,
        "activate abilities of land sources" => SpendPurpose::ActivateLandSourceAbility,
        "cast artifact spells or activate abilities of artifacts" => {
            return Ok(SpendRestriction::AnyOf(vec![
                SpendPurpose::CastSpell(SpellSpendFilter {
                    card_type: Some(SpendCardType::Artifact),
                    from: None,
                    minimum_mana_value: None,
                    mana_cost_contains_x: false,
                    chosen_creature_type: false,
                    monocolored_of_produced_color: false,
                    cannot_be_countered: false,
                }),
                SpendPurpose::ActivateArtifactAbility,
            ]));
        }
        "cast creature spells or activate abilities of creatures" => {
            return Ok(SpendRestriction::AnyOf(vec![
                SpendPurpose::CastSpell(SpellSpendFilter {
                    card_type: Some(SpendCardType::Creature),
                    from: None,
                    minimum_mana_value: None,
                    mana_cost_contains_x: false,
                    chosen_creature_type: false,
                    monocolored_of_produced_color: false,
                    cannot_be_countered: false,
                }),
                SpendPurpose::ActivateCreatureAbility,
            ]));
        }
        _ => {
            return Err(ManaExpressionError::UnsupportedSpendRestriction {
                text: text.to_string(),
            });
        }
    };
    Ok(SpendRestriction::Only(purpose))
}

fn parse_retention(text: &str) -> Result<ManaRetention, ManaExpressionError> {
    for (prefix, boundary) in [
        ("until end of turn, ", RetentionBoundary::EndOfTurn),
        ("until end of combat, ", RetentionBoundary::EndOfCombat),
        (
            "until your next main phase, ",
            RetentionBoundary::NextMainPhase,
        ),
    ] {
        let Some(body) = text.strip_prefix(prefix) else {
            continue;
        };
        return match body {
            "you don't lose this mana as steps and phases end" => Ok(ManaRetention::Retain {
                through: boundary,
                across_steps: true,
                across_phases: true,
            }),
            "you don't lose this mana as steps end" => Ok(ManaRetention::Retain {
                through: boundary,
                across_steps: true,
                across_phases: false,
            }),
            _ => Err(ManaExpressionError::UnsupportedRetention {
                text: text.to_string(),
            }),
        };
    }
    Err(ManaExpressionError::UnsupportedRetention {
        text: text.to_string(),
    })
}

fn split_complete_sentences(text: &str) -> Result<Vec<&str>, ManaExpressionError> {
    let mut sentences = Vec::new();
    let mut start = 0usize;
    for (index, character) in text.char_indices() {
        if character != '.' {
            continue;
        }
        let sentence = text[start..index].trim();
        if sentence.is_empty() {
            return Err(ManaExpressionError::UnexpectedTrailingInput {
                text: text[start..].to_string(),
            });
        }
        sentences.push(sentence);
        start = index + 1;
    }
    let remainder = text[start..].trim();
    if !remainder.is_empty() {
        sentences.push(remainder);
    }
    if sentences.is_empty() {
        return Err(ManaExpressionError::EmptyInput);
    }
    Ok(sentences)
}

fn parse_fixed_quantity(text: &str) -> Option<u32> {
    if let Ok(value) = text.parse::<u32>() {
        return Some(value);
    }
    match text {
        "a" | "an" | "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedResourceSymbol {
    Mana(ManaSymbol),
    Energy,
    Ticket,
    Tap,
    Untap,
}

fn parse_resource_symbol(symbol: &str) -> Result<ParsedResourceSymbol, ManaExpressionError> {
    let upper = symbol.to_ascii_uppercase();
    match upper.as_str() {
        "E" => return Ok(ParsedResourceSymbol::Energy),
        "TK" => return Ok(ParsedResourceSymbol::Ticket),
        "T" => return Ok(ParsedResourceSymbol::Tap),
        "Q" => return Ok(ParsedResourceSymbol::Untap),
        "X" => return Ok(ParsedResourceSymbol::Mana(ManaSymbol::VariableX)),
        "S" => return Ok(ParsedResourceSymbol::Mana(ManaSymbol::Snow)),
        _ => {}
    }
    if let Some(color) = parse_color(&upper) {
        return Ok(ParsedResourceSymbol::Mana(ManaSymbol::Color(color)));
    }
    if upper.chars().all(|character| character.is_ascii_digit()) {
        let generic =
            upper
                .parse::<u32>()
                .map_err(|_| ManaExpressionError::ResourceAmountOverflow {
                    symbol: symbol.to_string(),
                })?;
        return Ok(ParsedResourceSymbol::Mana(ManaSymbol::Generic(generic)));
    }
    if let Some((left, right)) = upper.split_once('/') {
        if right == "P"
            && let Some(color) = parse_colored_mana(left)
        {
            return Ok(ParsedResourceSymbol::Mana(ManaSymbol::Phyrexian(color)));
        }
        if let (Some(left), Some(right)) = (parse_colored_mana(left), parse_colored_mana(right)) {
            return Ok(ParsedResourceSymbol::Mana(ManaSymbol::Hybrid(left, right)));
        }
        if let (Ok(generic), Some(color)) = (left.parse::<u32>(), parse_colored_mana(right)) {
            return Ok(ParsedResourceSymbol::Mana(ManaSymbol::GenericHybrid {
                generic,
                color,
            }));
        }
    }
    Err(classify_unsupported_symbol(symbol).unwrap_or_else(|| {
        ManaExpressionError::UnsupportedResourceSymbol {
            symbol: symbol.to_string(),
            kind: UnsupportedResourceSymbolKind::Unknown,
        }
    }))
}

fn classify_unsupported_symbol(symbol: &str) -> Option<ManaExpressionError> {
    let upper = symbol.to_ascii_uppercase();
    let kind = if matches!(upper.as_str(), "½" | "HW" | "HU" | "HB" | "HR" | "HG") {
        UnsupportedResourceSymbolKind::FractionalMana
    } else if matches!(upper.as_str(), "D" | "Z" | "A") {
        UnsupportedResourceSymbolKind::PlatformSpecific
    } else {
        return None;
    };
    Some(ManaExpressionError::UnsupportedResourceSymbol {
        symbol: symbol.to_string(),
        kind,
    })
}

fn parse_color(symbol: &str) -> Option<ManaColor> {
    match symbol.to_ascii_uppercase().as_str() {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        "C" => Some(ManaColor::Colorless),
        _ => None,
    }
}

fn parse_colored_mana(symbol: &str) -> Option<ManaColor> {
    parse_color(symbol).filter(|color| *color != ManaColor::Colorless)
}

fn normalize(input: &str) -> String {
    input
        .replace('\u{2019}', "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn flush_mana(components: &mut Vec<ResourceCostComponent>, pending: &mut Vec<ManaSymbol>) {
    if !pending.is_empty() {
        components.push(ResourceCostComponent::Mana(ManaCost {
            symbols: std::mem::take(pending),
        }));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceKind {
    Energy,
    Ticket,
}

fn increment_resource(
    components: &mut Vec<ResourceCostComponent>,
    kind: ResourceKind,
) -> Result<(), ManaExpressionError> {
    match (kind, components.last_mut()) {
        (ResourceKind::Energy, Some(ResourceCostComponent::Energy(amount)))
        | (ResourceKind::Ticket, Some(ResourceCostComponent::Tickets(amount))) => {
            *amount = amount.checked_add(1).ok_or_else(|| {
                ManaExpressionError::ResourceAmountOverflow {
                    symbol: match kind {
                        ResourceKind::Energy => "E",
                        ResourceKind::Ticket => "TK",
                    }
                    .to_string(),
                }
            })?;
        }
        (ResourceKind::Energy, _) => components.push(ResourceCostComponent::Energy(1)),
        (ResourceKind::Ticket, _) => components.push(ResourceCostComponent::Tickets(1)),
    }
    Ok(())
}
