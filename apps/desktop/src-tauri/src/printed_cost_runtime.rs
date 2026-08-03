//! Exact parsing and transactional payment for printed mana costs.
//!
//! The parser is deliberately closed. A symbol that is not represented by a
//! typed variant is rejected instead of being treated as generic mana. Costs
//! with multiple faces remain separate, and all alternative payments require
//! an explicit caller choice.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::strict_engine::{
    ManaColor, ManaCost as StrictManaCost, ManaPaymentChoices as StrictManaPaymentChoices,
    ManaSymbol as StrictManaSymbol, ManaUnit, ManaUnitId, ObjectId, PhyrexianPayment,
};

pub(crate) const PRINTED_COST_RUNTIME_VERSION: &str = "printed-cost-runtime-0.1";
pub(crate) const PRINTED_COST_PAYMENT_BRIDGE_VERSION: &str = "printed-cost-payment-bridge-0.1";

/// One variable letter has one declared value for the complete cost. Repeated
/// appearances of that letter each contribute the declared value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum VariableManaSymbol {
    X,
    Y,
    Z,
}

impl VariableManaSymbol {
    fn token(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }
}

/// Symbols already represented by the strict kernel are retained without
/// translation. The remaining variants cover printed symbols whose values or
/// alternatives cannot be expressed by `strict_engine::ManaSymbol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrintedManaSymbol {
    Strict(StrictManaSymbol),
    LargeGeneric(u64),
    MonocoloredHybrid {
        generic: u16,
        color: ManaColor,
    },
    HybridPhyrexian {
        first: ManaColor,
        second: ManaColor,
    },
    Variable(VariableManaSymbol),
    HalfColored(ManaColor),
    HalfGeneric,
    /// One mana of any type produced by a legendary source.
    LegendarySource,
    /// A nonmana printed cost paid by giving up one remaining land play.
    LandDrop,
    Infinity,
}

impl PrintedManaSymbol {
    pub(crate) fn strict_symbol(&self) -> Option<&StrictManaSymbol> {
        match self {
            Self::Strict(symbol) => Some(symbol),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrintedManaCostFace {
    pub raw: String,
    /// An empty Scryfall cost is no mana cost, not a zero mana cost.
    pub has_mana_cost: bool,
    pub symbols: Vec<PrintedManaSymbol>,
}

impl PrintedManaCostFace {
    /// Returns a strict-kernel cost only when no information would be lost.
    pub(crate) fn strict_cost(&self) -> Option<StrictManaCost> {
        if !self.has_mana_cost {
            return None;
        }
        self.symbols
            .iter()
            .map(|symbol| symbol.strict_symbol().cloned())
            .collect::<Option<Vec<_>>>()
            .map(|symbols| StrictManaCost { symbols })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrintedManaCost {
    pub raw: String,
    pub faces: Vec<PrintedManaCostFace>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrintedCostParseError {
    ExpectedOpeningBrace {
        face_index: usize,
        byte_offset: usize,
    },
    MissingClosingBrace {
        face_index: usize,
        byte_offset: usize,
    },
    EmptySymbol {
        face_index: usize,
        byte_offset: usize,
    },
    NestedBrace {
        face_index: usize,
        byte_offset: usize,
    },
    UnsupportedSymbol {
        face_index: usize,
        symbol_index: usize,
        symbol: String,
    },
    NumericOverflow {
        face_index: usize,
        symbol_index: usize,
        symbol: String,
    },
}

impl fmt::Display for PrintedCostParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedOpeningBrace {
                face_index,
                byte_offset,
            } => write!(
                formatter,
                "expected an opening brace in face {face_index} at byte {byte_offset}"
            ),
            Self::MissingClosingBrace {
                face_index,
                byte_offset,
            } => write!(
                formatter,
                "missing a closing brace in face {face_index} after byte {byte_offset}"
            ),
            Self::EmptySymbol {
                face_index,
                byte_offset,
            } => write!(
                formatter,
                "empty mana symbol in face {face_index} at byte {byte_offset}"
            ),
            Self::NestedBrace {
                face_index,
                byte_offset,
            } => write!(
                formatter,
                "nested mana brace in face {face_index} at byte {byte_offset}"
            ),
            Self::UnsupportedSymbol {
                face_index,
                symbol_index,
                symbol,
            } => write!(
                formatter,
                "unsupported printed mana symbol {{{symbol}}} at face {face_index}, symbol {symbol_index}"
            ),
            Self::NumericOverflow {
                face_index,
                symbol_index,
                symbol,
            } => write!(
                formatter,
                "printed mana symbol {{{symbol}}} is too large at face {face_index}, symbol {symbol_index}"
            ),
        }
    }
}

impl std::error::Error for PrintedCostParseError {}

pub(crate) fn parse_printed_mana_cost(
    printed: &str,
) -> Result<PrintedManaCost, PrintedCostParseError> {
    let printed = printed.trim();
    if printed.is_empty() {
        return Ok(PrintedManaCost {
            raw: String::new(),
            faces: vec![PrintedManaCostFace {
                raw: String::new(),
                has_mana_cost: false,
                symbols: Vec::new(),
            }],
        });
    }

    let mut faces = Vec::new();
    for (face_index, raw_face) in printed.split("//").enumerate() {
        let raw_face = raw_face.trim();
        if raw_face.is_empty() {
            faces.push(PrintedManaCostFace {
                raw: String::new(),
                has_mana_cost: false,
                symbols: Vec::new(),
            });
            continue;
        }
        let tokens = scan_symbols(raw_face, face_index)?;
        let symbols = tokens
            .iter()
            .enumerate()
            .map(|(symbol_index, token)| parse_symbol(token, face_index, symbol_index))
            .collect::<Result<Vec<_>, _>>()?;
        faces.push(PrintedManaCostFace {
            raw: raw_face.to_owned(),
            has_mana_cost: true,
            symbols,
        });
    }

    Ok(PrintedManaCost {
        raw: printed.to_owned(),
        faces,
    })
}

pub(crate) fn printed_mana_cost_has_exact_payment_contract(cost: &PrintedManaCost) -> bool {
    !cost.faces.is_empty()
        && cost.faces.iter().all(|face| {
            if face.has_mana_cost {
                !face.raw.trim().is_empty()
            } else {
                face.raw.is_empty() && face.symbols.is_empty()
            }
        })
        && parse_printed_mana_cost(&cost.raw).is_ok_and(|recompiled| recompiled == *cost)
}

fn scan_symbols(face: &str, face_index: usize) -> Result<Vec<String>, PrintedCostParseError> {
    let mut symbols = Vec::new();
    let mut byte_offset = 0usize;
    while byte_offset < face.len() {
        let remaining = &face[byte_offset..];
        let Some(character) = remaining.chars().next() else {
            break;
        };
        if character.is_whitespace() {
            byte_offset += character.len_utf8();
            continue;
        }
        if character != '{' {
            return Err(PrintedCostParseError::ExpectedOpeningBrace {
                face_index,
                byte_offset,
            });
        }
        let symbol_start = byte_offset + character.len_utf8();
        let Some(relative_end) = face[symbol_start..].find('}') else {
            return Err(PrintedCostParseError::MissingClosingBrace {
                face_index,
                byte_offset,
            });
        };
        let symbol_end = symbol_start + relative_end;
        if face[symbol_start..symbol_end].contains('{') {
            return Err(PrintedCostParseError::NestedBrace {
                face_index,
                byte_offset,
            });
        }
        let symbol = face[symbol_start..symbol_end].trim();
        if symbol.is_empty() {
            return Err(PrintedCostParseError::EmptySymbol {
                face_index,
                byte_offset,
            });
        }
        symbols.push(symbol.to_uppercase());
        byte_offset = symbol_end + '}'.len_utf8();
    }
    Ok(symbols)
}

fn parse_symbol(
    symbol: &str,
    face_index: usize,
    symbol_index: usize,
) -> Result<PrintedManaSymbol, PrintedCostParseError> {
    if symbol.chars().all(|character| character.is_ascii_digit()) {
        let value = symbol
            .parse::<u64>()
            .map_err(|_| PrintedCostParseError::NumericOverflow {
                face_index,
                symbol_index,
                symbol: symbol.to_owned(),
            })?;
        return if let Ok(value) = u16::try_from(value) {
            Ok(PrintedManaSymbol::Strict(StrictManaSymbol::Generic(value)))
        } else {
            Ok(PrintedManaSymbol::LargeGeneric(value))
        };
    }

    if let Some(color) = color_from_token(symbol) {
        return Ok(if color == ManaColor::Colorless {
            PrintedManaSymbol::Strict(StrictManaSymbol::Colorless)
        } else {
            PrintedManaSymbol::Strict(StrictManaSymbol::Colored(color))
        });
    }

    match symbol {
        "S" => return Ok(PrintedManaSymbol::Strict(StrictManaSymbol::Snow)),
        "X" => {
            return Ok(PrintedManaSymbol::Strict(StrictManaSymbol::VariableX));
        }
        "Y" => return Ok(PrintedManaSymbol::Variable(VariableManaSymbol::Y)),
        "Z" => return Ok(PrintedManaSymbol::Variable(VariableManaSymbol::Z)),
        "L" => return Ok(PrintedManaSymbol::LegendarySource),
        "D" => return Ok(PrintedManaSymbol::LandDrop),
        "∞" | "INFINITY" => return Ok(PrintedManaSymbol::Infinity),
        "H" | "½" | "1/2" | "0.5" => return Ok(PrintedManaSymbol::HalfGeneric),
        _ => {}
    }

    if let Some(half_color) = symbol
        .strip_prefix('H')
        .or_else(|| symbol.strip_prefix('½'))
        .and_then(color_from_token)
    {
        return Ok(PrintedManaSymbol::HalfColored(half_color));
    }

    let alternatives = symbol.split('/').collect::<Vec<_>>();
    match alternatives.as_slice() {
        [generic, color] if generic.chars().all(|character| character.is_ascii_digit()) => {
            let generic =
                generic
                    .parse::<u16>()
                    .map_err(|_| PrintedCostParseError::NumericOverflow {
                        face_index,
                        symbol_index,
                        symbol: symbol.to_owned(),
                    })?;
            let Some(color) = color_from_token(color) else {
                return Err(PrintedCostParseError::UnsupportedSymbol {
                    face_index,
                    symbol_index,
                    symbol: symbol.to_owned(),
                });
            };
            Ok(PrintedManaSymbol::MonocoloredHybrid { generic, color })
        }
        [first, second] => {
            if first.eq_ignore_ascii_case("P") {
                let Some(color) = color_from_token(second) else {
                    return Err(PrintedCostParseError::UnsupportedSymbol {
                        face_index,
                        symbol_index,
                        symbol: symbol.to_owned(),
                    });
                };
                return Ok(PrintedManaSymbol::Strict(StrictManaSymbol::Phyrexian(
                    color,
                )));
            }
            if second.eq_ignore_ascii_case("P") {
                let Some(color) = color_from_token(first) else {
                    return Err(PrintedCostParseError::UnsupportedSymbol {
                        face_index,
                        symbol_index,
                        symbol: symbol.to_owned(),
                    });
                };
                return Ok(PrintedManaSymbol::Strict(StrictManaSymbol::Phyrexian(
                    color,
                )));
            }
            let (Some(first), Some(second)) = (color_from_token(first), color_from_token(second))
            else {
                return Err(PrintedCostParseError::UnsupportedSymbol {
                    face_index,
                    symbol_index,
                    symbol: symbol.to_owned(),
                });
            };
            Ok(PrintedManaSymbol::Strict(StrictManaSymbol::Hybrid(
                first, second,
            )))
        }
        [first, second, phyrexian] if phyrexian.eq_ignore_ascii_case("P") => {
            let (Some(first), Some(second)) = (color_from_token(first), color_from_token(second))
            else {
                return Err(PrintedCostParseError::UnsupportedSymbol {
                    face_index,
                    symbol_index,
                    symbol: symbol.to_owned(),
                });
            };
            Ok(PrintedManaSymbol::HybridPhyrexian { first, second })
        }
        _ => Err(PrintedCostParseError::UnsupportedSymbol {
            face_index,
            symbol_index,
            symbol: symbol.to_owned(),
        }),
    }
}

fn color_from_token(token: &str) -> Option<ManaColor> {
    match token {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        "C" => Some(ManaColor::Colorless),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SymbolPaymentChoice {
    Mana(ManaColor),
    Generic,
    Phyrexian(PhyrexianPayment),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PrintedManaPaymentChoices {
    pub variables: BTreeMap<VariableManaSymbol, u64>,
    /// Indexed by the symbol's position within the selected face.
    pub alternatives: BTreeMap<usize, SymbolPaymentChoice>,
}

impl PrintedManaPaymentChoices {
    /// Builds the strict kernel's choice record when this face and every
    /// declared value fit the kernel vocabulary. Hybrid branch choices remain
    /// validated by this runtime because the strict record has no hybrid slot.
    pub(crate) fn strict_choices_for(
        &self,
        face: &PrintedManaCostFace,
    ) -> Option<StrictManaPaymentChoices> {
        face.strict_cost()?;
        let has_x = face.symbols.iter().any(|symbol| {
            matches!(
                symbol,
                PrintedManaSymbol::Strict(StrictManaSymbol::VariableX)
            )
        });
        let x_value = if has_x {
            u16::try_from(*self.variables.get(&VariableManaSymbol::X)?).ok()?
        } else {
            0
        };
        if self
            .variables
            .keys()
            .any(|variable| *variable != VariableManaSymbol::X || !has_x)
        {
            return None;
        }

        let mut phyrexian = Vec::new();
        let mut expected_alternatives = BTreeSet::new();
        for (index, symbol) in face.symbols.iter().enumerate() {
            match symbol {
                PrintedManaSymbol::Strict(StrictManaSymbol::Hybrid(first, second)) => {
                    expected_alternatives.insert(index);
                    let SymbolPaymentChoice::Mana(color) =
                        self.alternatives.get(&index).copied()?
                    else {
                        return None;
                    };
                    if color != *first && color != *second {
                        return None;
                    }
                }
                PrintedManaSymbol::Strict(StrictManaSymbol::Phyrexian(color)) => {
                    expected_alternatives.insert(index);
                    let payment = match self.alternatives.get(&index).copied()? {
                        SymbolPaymentChoice::Phyrexian(payment) => payment,
                        SymbolPaymentChoice::Mana(chosen) if chosen == *color => {
                            PhyrexianPayment::Mana
                        }
                        _ => return None,
                    };
                    phyrexian.push(payment);
                }
                _ => {}
            }
        }
        if self
            .alternatives
            .keys()
            .any(|index| !expected_alternatives.contains(index))
        {
            return None;
        }
        Some(StrictManaPaymentChoices { x_value, phyrexian })
    }
}

/// One strict mana unit may carry two half-units in ordinary Magic or one
/// half-unit when a half-mana producing effect explicitly created it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AvailableManaUnit {
    pub unit: ManaUnit,
    pub half_units: u64,
    pub from_legendary_source: bool,
}

impl AvailableManaUnit {
    pub(crate) fn whole(unit: ManaUnit) -> Self {
        Self {
            unit,
            half_units: 2,
            from_legendary_source: false,
        }
    }

    pub(crate) fn half(unit: ManaUnit) -> Self {
        Self {
            unit,
            half_units: 1,
            from_legendary_source: false,
        }
    }

    pub(crate) fn from_legendary_source(mut self) -> Self {
        self.from_legendary_source = true;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PrintedManaPaymentResources {
    pub life: u64,
    pub mana: Vec<AvailableManaUnit>,
    pub land_drops_remaining: u32,
    pub is_active_player_turn: bool,
}

impl PrintedManaPaymentResources {
    pub(crate) fn from_strict_units(life: u64, mana: Vec<ManaUnit>) -> Self {
        Self {
            life,
            mana: mana.into_iter().map(AvailableManaUnit::whole).collect(),
            land_drops_remaining: 0,
            is_active_player_turn: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManaSpendReceipt {
    pub mana_unit_id: ManaUnitId,
    pub source: Option<ObjectId>,
    pub color: ManaColor,
    pub from_snow_source: bool,
    pub from_legendary_source: bool,
    pub half_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SymbolPaymentReceipt {
    pub symbol_index: usize,
    pub symbol: PrintedManaSymbol,
    pub choice: Option<SymbolPaymentChoice>,
    pub variable_value: Option<u64>,
    pub mana_spent: Vec<ManaSpendReceipt>,
    pub life_paid: u64,
    pub land_drops_spent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrintedManaPaymentReceipt {
    pub runtime_version: &'static str,
    pub face_index: usize,
    pub face_raw: String,
    pub symbols: Vec<SymbolPaymentReceipt>,
    pub total_life_paid: u64,
    pub total_land_drops_spent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrintedManaPaymentError {
    FaceOutOfRange {
        requested: usize,
        face_count: usize,
    },
    NoManaCost {
        face_index: usize,
    },
    DuplicateManaUnit(ManaUnitId),
    EmptyManaUnit(ManaUnitId),
    UnexpectedAlternativeChoice {
        symbol_index: usize,
    },
    MissingAlternativeChoice {
        symbol_index: usize,
    },
    InvalidAlternativeChoice {
        symbol_index: usize,
        choice: SymbolPaymentChoice,
    },
    MissingVariableValue(VariableManaSymbol),
    UnexpectedVariableValue(VariableManaSymbol),
    ManaAmountOverflow,
    InfiniteManaCannotBePaid {
        symbol_index: usize,
    },
    InsufficientMana,
    InsufficientLife {
        required: u64,
        available: u64,
    },
    LandDropPaymentOutsideActiveTurn {
        symbol_index: usize,
    },
    InsufficientLandDrops {
        required: u32,
        available: u32,
    },
}

impl fmt::Display for PrintedManaPaymentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FaceOutOfRange {
                requested,
                face_count,
            } => write!(
                formatter,
                "printed mana cost face {requested} is outside {face_count} faces"
            ),
            Self::NoManaCost { face_index } => {
                write!(
                    formatter,
                    "printed face {face_index} has no mana cost to pay"
                )
            }
            Self::DuplicateManaUnit(id) => {
                write!(formatter, "mana unit {} appears more than once", id.0)
            }
            Self::EmptyManaUnit(id) => {
                write!(formatter, "mana unit {} contains no spendable mana", id.0)
            }
            Self::UnexpectedAlternativeChoice { symbol_index } => {
                write!(
                    formatter,
                    "symbol {symbol_index} has an unexpected payment choice"
                )
            }
            Self::MissingAlternativeChoice { symbol_index } => {
                write!(
                    formatter,
                    "symbol {symbol_index} needs an explicit payment choice"
                )
            }
            Self::InvalidAlternativeChoice {
                symbol_index,
                choice,
            } => write!(
                formatter,
                "choice {choice:?} cannot pay printed symbol {symbol_index}"
            ),
            Self::MissingVariableValue(variable) => {
                write!(formatter, "{} has no declared value", variable.token())
            }
            Self::UnexpectedVariableValue(variable) => {
                write!(
                    formatter,
                    "{} has a value but is absent from the cost",
                    variable.token()
                )
            }
            Self::ManaAmountOverflow => formatter.write_str("mana amount overflowed"),
            Self::InfiniteManaCannotBePaid { symbol_index } => write!(
                formatter,
                "infinite mana at symbol {symbol_index} cannot be paid from a finite pool"
            ),
            Self::InsufficientMana => formatter.write_str("insufficient mana"),
            Self::InsufficientLife {
                required,
                available,
            } => write!(
                formatter,
                "insufficient life: payment needs {required}, but only {available} is available"
            ),
            Self::LandDropPaymentOutsideActiveTurn { symbol_index } => write!(
                formatter,
                "land drop symbol {symbol_index} can be paid only during the payer's turn"
            ),
            Self::InsufficientLandDrops {
                required,
                available,
            } => write!(
                formatter,
                "insufficient land drops: payment needs {required}, but only {available} remain"
            ),
        }
    }
}

impl std::error::Error for PrintedManaPaymentError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManaConstraint {
    Color(ManaColor),
    Snow,
    LegendarySource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConstrainedObligation {
    symbol_index: usize,
    constraint: ManaConstraint,
    half_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GenericObligation {
    symbol_index: usize,
    half_units: u64,
}

/// Pays exactly one selected face. Validation and assignment occur against a
/// staged copy, so any failure leaves both life and mana untouched.
pub(crate) fn pay_printed_mana_cost(
    cost: &PrintedManaCost,
    face_index: usize,
    choices: &PrintedManaPaymentChoices,
    resources: &mut PrintedManaPaymentResources,
) -> Result<PrintedManaPaymentReceipt, PrintedManaPaymentError> {
    let face = cost
        .faces
        .get(face_index)
        .ok_or(PrintedManaPaymentError::FaceOutOfRange {
            requested: face_index,
            face_count: cost.faces.len(),
        })?;
    if !face.has_mana_cost {
        return Err(PrintedManaPaymentError::NoManaCost { face_index });
    }
    validate_resources(resources)?;

    let mut constrained = Vec::new();
    let mut generic = Vec::new();
    let mut receipts = face
        .symbols
        .iter()
        .cloned()
        .enumerate()
        .map(|(symbol_index, symbol)| SymbolPaymentReceipt {
            symbol_index,
            symbol,
            choice: None,
            variable_value: None,
            mana_spent: Vec::new(),
            life_paid: 0,
            land_drops_spent: 0,
        })
        .collect::<Vec<_>>();
    let mut life_due = 0u64;
    let mut land_drops_due = 0u32;
    let mut expected_variables = BTreeSet::new();
    let mut expected_alternatives = BTreeSet::new();

    for (symbol_index, symbol) in face.symbols.iter().enumerate() {
        match symbol {
            PrintedManaSymbol::Strict(StrictManaSymbol::Generic(amount)) => {
                push_generic(&mut generic, symbol_index, u64::from(*amount))?;
            }
            PrintedManaSymbol::LargeGeneric(amount) => {
                push_generic(&mut generic, symbol_index, *amount)?;
            }
            PrintedManaSymbol::Strict(StrictManaSymbol::Colored(color)) => {
                constrained.push(ConstrainedObligation {
                    symbol_index,
                    constraint: ManaConstraint::Color(*color),
                    half_units: 2,
                });
            }
            PrintedManaSymbol::Strict(StrictManaSymbol::Colorless) => {
                constrained.push(ConstrainedObligation {
                    symbol_index,
                    constraint: ManaConstraint::Color(ManaColor::Colorless),
                    half_units: 2,
                });
            }
            PrintedManaSymbol::Strict(StrictManaSymbol::Snow) => {
                constrained.push(ConstrainedObligation {
                    symbol_index,
                    constraint: ManaConstraint::Snow,
                    half_units: 2,
                });
            }
            PrintedManaSymbol::Strict(StrictManaSymbol::VariableX) => {
                expected_variables.insert(VariableManaSymbol::X);
                let value = required_variable(choices, VariableManaSymbol::X)?;
                receipts[symbol_index].variable_value = Some(value);
                push_generic(&mut generic, symbol_index, value)?;
            }
            PrintedManaSymbol::Variable(variable) => {
                expected_variables.insert(*variable);
                let value = required_variable(choices, *variable)?;
                receipts[symbol_index].variable_value = Some(value);
                push_generic(&mut generic, symbol_index, value)?;
            }
            PrintedManaSymbol::Strict(StrictManaSymbol::Hybrid(first, second)) => {
                expected_alternatives.insert(symbol_index);
                let choice = required_choice(choices, symbol_index)?;
                receipts[symbol_index].choice = Some(choice);
                let SymbolPaymentChoice::Mana(color) = choice else {
                    return Err(PrintedManaPaymentError::InvalidAlternativeChoice {
                        symbol_index,
                        choice,
                    });
                };
                if color != *first && color != *second {
                    return Err(PrintedManaPaymentError::InvalidAlternativeChoice {
                        symbol_index,
                        choice,
                    });
                }
                constrained.push(ConstrainedObligation {
                    symbol_index,
                    constraint: ManaConstraint::Color(color),
                    half_units: 2,
                });
            }
            PrintedManaSymbol::MonocoloredHybrid {
                generic: amount,
                color,
            } => {
                expected_alternatives.insert(symbol_index);
                let choice = required_choice(choices, symbol_index)?;
                receipts[symbol_index].choice = Some(choice);
                match choice {
                    SymbolPaymentChoice::Mana(chosen) if chosen == *color => {
                        constrained.push(ConstrainedObligation {
                            symbol_index,
                            constraint: ManaConstraint::Color(*color),
                            half_units: 2,
                        });
                    }
                    SymbolPaymentChoice::Generic => {
                        push_generic(&mut generic, symbol_index, u64::from(*amount))?;
                    }
                    _ => {
                        return Err(PrintedManaPaymentError::InvalidAlternativeChoice {
                            symbol_index,
                            choice,
                        });
                    }
                }
            }
            PrintedManaSymbol::Strict(StrictManaSymbol::Phyrexian(color)) => {
                expected_alternatives.insert(symbol_index);
                let choice = required_choice(choices, symbol_index)?;
                receipts[symbol_index].choice = Some(choice);
                match choice {
                    SymbolPaymentChoice::Mana(chosen) if chosen == *color => {
                        constrained.push(ConstrainedObligation {
                            symbol_index,
                            constraint: ManaConstraint::Color(*color),
                            half_units: 2,
                        });
                    }
                    SymbolPaymentChoice::Phyrexian(PhyrexianPayment::Mana) => {
                        constrained.push(ConstrainedObligation {
                            symbol_index,
                            constraint: ManaConstraint::Color(*color),
                            half_units: 2,
                        });
                    }
                    SymbolPaymentChoice::Phyrexian(PhyrexianPayment::Life) => {
                        life_due = life_due
                            .checked_add(2)
                            .ok_or(PrintedManaPaymentError::ManaAmountOverflow)?;
                        receipts[symbol_index].life_paid = 2;
                    }
                    _ => {
                        return Err(PrintedManaPaymentError::InvalidAlternativeChoice {
                            symbol_index,
                            choice,
                        });
                    }
                }
            }
            PrintedManaSymbol::HybridPhyrexian { first, second } => {
                expected_alternatives.insert(symbol_index);
                let choice = required_choice(choices, symbol_index)?;
                receipts[symbol_index].choice = Some(choice);
                match choice {
                    SymbolPaymentChoice::Mana(color) if color == *first || color == *second => {
                        constrained.push(ConstrainedObligation {
                            symbol_index,
                            constraint: ManaConstraint::Color(color),
                            half_units: 2,
                        });
                    }
                    SymbolPaymentChoice::Phyrexian(PhyrexianPayment::Life) => {
                        life_due = life_due
                            .checked_add(2)
                            .ok_or(PrintedManaPaymentError::ManaAmountOverflow)?;
                        receipts[symbol_index].life_paid = 2;
                    }
                    _ => {
                        return Err(PrintedManaPaymentError::InvalidAlternativeChoice {
                            symbol_index,
                            choice,
                        });
                    }
                }
            }
            PrintedManaSymbol::HalfColored(color) => {
                constrained.push(ConstrainedObligation {
                    symbol_index,
                    constraint: ManaConstraint::Color(*color),
                    half_units: 1,
                });
            }
            PrintedManaSymbol::HalfGeneric => {
                generic.push(GenericObligation {
                    symbol_index,
                    half_units: 1,
                });
            }
            PrintedManaSymbol::LegendarySource => {
                constrained.push(ConstrainedObligation {
                    symbol_index,
                    constraint: ManaConstraint::LegendarySource,
                    half_units: 2,
                });
            }
            PrintedManaSymbol::LandDrop => {
                if !resources.is_active_player_turn {
                    return Err(PrintedManaPaymentError::LandDropPaymentOutsideActiveTurn {
                        symbol_index,
                    });
                }
                land_drops_due = land_drops_due
                    .checked_add(1)
                    .ok_or(PrintedManaPaymentError::ManaAmountOverflow)?;
                receipts[symbol_index].land_drops_spent = 1;
            }
            PrintedManaSymbol::Infinity => {
                return Err(PrintedManaPaymentError::InfiniteManaCannotBePaid { symbol_index });
            }
        }
    }

    for &symbol_index in choices.alternatives.keys() {
        if !expected_alternatives.contains(&symbol_index) {
            return Err(PrintedManaPaymentError::UnexpectedAlternativeChoice { symbol_index });
        }
    }
    for &variable in choices.variables.keys() {
        if !expected_variables.contains(&variable) {
            return Err(PrintedManaPaymentError::UnexpectedVariableValue(variable));
        }
    }
    if resources.life < life_due {
        return Err(PrintedManaPaymentError::InsufficientLife {
            required: life_due,
            available: resources.life,
        });
    }
    if resources.land_drops_remaining < land_drops_due {
        return Err(PrintedManaPaymentError::InsufficientLandDrops {
            required: land_drops_due,
            available: resources.land_drops_remaining,
        });
    }

    let mut remaining = resources
        .mana
        .iter()
        .map(|mana| mana.half_units)
        .collect::<Vec<_>>();
    assign_constrained(&resources.mana, &mut remaining, &constrained, &mut receipts)?;
    assign_generic(&resources.mana, &mut remaining, &generic, &mut receipts)?;

    let mut staged = resources.clone();
    staged.life -= life_due;
    staged.land_drops_remaining -= land_drops_due;
    for (mana, half_units) in staged.mana.iter_mut().zip(remaining) {
        mana.half_units = half_units;
    }
    staged.mana.retain(|mana| mana.half_units > 0);
    *resources = staged;

    Ok(PrintedManaPaymentReceipt {
        runtime_version: PRINTED_COST_RUNTIME_VERSION,
        face_index,
        face_raw: face.raw.clone(),
        symbols: receipts,
        total_life_paid: life_due,
        total_land_drops_spent: land_drops_due,
    })
}

fn validate_resources(
    resources: &PrintedManaPaymentResources,
) -> Result<(), PrintedManaPaymentError> {
    let mut ids = BTreeSet::new();
    for mana in &resources.mana {
        if mana.half_units == 0 {
            return Err(PrintedManaPaymentError::EmptyManaUnit(mana.unit.id));
        }
        if !ids.insert(mana.unit.id) {
            return Err(PrintedManaPaymentError::DuplicateManaUnit(mana.unit.id));
        }
    }
    Ok(())
}

fn required_choice(
    choices: &PrintedManaPaymentChoices,
    symbol_index: usize,
) -> Result<SymbolPaymentChoice, PrintedManaPaymentError> {
    choices
        .alternatives
        .get(&symbol_index)
        .copied()
        .ok_or(PrintedManaPaymentError::MissingAlternativeChoice { symbol_index })
}

fn required_variable(
    choices: &PrintedManaPaymentChoices,
    variable: VariableManaSymbol,
) -> Result<u64, PrintedManaPaymentError> {
    choices
        .variables
        .get(&variable)
        .copied()
        .ok_or(PrintedManaPaymentError::MissingVariableValue(variable))
}

fn push_generic(
    generic: &mut Vec<GenericObligation>,
    symbol_index: usize,
    whole_units: u64,
) -> Result<(), PrintedManaPaymentError> {
    let half_units = whole_units
        .checked_mul(2)
        .ok_or(PrintedManaPaymentError::ManaAmountOverflow)?;
    generic.push(GenericObligation {
        symbol_index,
        half_units,
    });
    Ok(())
}

fn unit_matches(unit: &AvailableManaUnit, constraint: ManaConstraint) -> bool {
    match constraint {
        ManaConstraint::Color(color) => unit.unit.color == color,
        ManaConstraint::Snow => unit.unit.from_snow_source,
        ManaConstraint::LegendarySource => unit.from_legendary_source,
    }
}

fn assign_constrained(
    mana: &[AvailableManaUnit],
    remaining: &mut [u64],
    obligations: &[ConstrainedObligation],
    receipts: &mut [SymbolPaymentReceipt],
) -> Result<(), PrintedManaPaymentError> {
    if obligations.is_empty() {
        return Ok(());
    }

    let source = 0usize;
    let mana_start = 1usize;
    let obligation_start = mana_start + mana.len();
    let sink = obligation_start + obligations.len();
    let mut network = FlowNetwork::new(sink + 1);

    for (mana_index, available) in remaining.iter().copied().enumerate() {
        network.add_edge(source, mana_start + mana_index, available);
    }
    let mut assignment_edges = Vec::new();
    for (obligation_index, obligation) in obligations.iter().enumerate() {
        let obligation_node = obligation_start + obligation_index;
        network.add_edge(obligation_node, sink, obligation.half_units);
        for (mana_index, unit) in mana.iter().enumerate() {
            if unit_matches(unit, obligation.constraint) {
                let edge_index = network.add_edge(
                    mana_start + mana_index,
                    obligation_node,
                    obligation.half_units,
                );
                assignment_edges.push((
                    mana_index,
                    obligation_index,
                    mana_start + mana_index,
                    edge_index,
                    obligation.half_units,
                ));
            }
        }
    }

    let total_due = obligations.iter().try_fold(0u64, |total, obligation| {
        total
            .checked_add(obligation.half_units)
            .ok_or(PrintedManaPaymentError::ManaAmountOverflow)
    })?;
    if network.max_flow(source, sink) != total_due {
        return Err(PrintedManaPaymentError::InsufficientMana);
    }

    for (mana_index, obligation_index, node, edge_index, capacity) in assignment_edges {
        let spent = capacity - network.edges[node][edge_index].capacity;
        if spent == 0 {
            continue;
        }
        remaining[mana_index] -= spent;
        add_spend(
            &mut receipts[obligations[obligation_index].symbol_index],
            &mana[mana_index],
            spent,
        );
    }
    Ok(())
}

fn assign_generic(
    mana: &[AvailableManaUnit],
    remaining: &mut [u64],
    obligations: &[GenericObligation],
    receipts: &mut [SymbolPaymentReceipt],
) -> Result<(), PrintedManaPaymentError> {
    for obligation in obligations {
        let mut due = obligation.half_units;
        for (mana_index, available) in remaining.iter_mut().enumerate() {
            if due == 0 {
                break;
            }
            let spent = (*available).min(due);
            *available -= spent;
            due -= spent;
            if spent > 0 {
                add_spend(
                    &mut receipts[obligation.symbol_index],
                    &mana[mana_index],
                    spent,
                );
            }
        }
        if due > 0 {
            return Err(PrintedManaPaymentError::InsufficientMana);
        }
    }
    Ok(())
}

fn add_spend(receipt: &mut SymbolPaymentReceipt, mana: &AvailableManaUnit, half_units: u64) {
    if let Some(existing) = receipt
        .mana_spent
        .iter_mut()
        .find(|spent| spent.mana_unit_id == mana.unit.id)
    {
        existing.half_units += half_units;
        return;
    }
    receipt.mana_spent.push(ManaSpendReceipt {
        mana_unit_id: mana.unit.id,
        source: mana.unit.source,
        color: mana.unit.color,
        from_snow_source: mana.unit.from_snow_source,
        from_legendary_source: mana.from_legendary_source,
        half_units,
    });
}

#[derive(Debug, Clone, Copy)]
struct FlowEdge {
    destination: usize,
    reverse: usize,
    capacity: u64,
}

#[derive(Debug)]
struct FlowNetwork {
    edges: Vec<Vec<FlowEdge>>,
}

impl FlowNetwork {
    fn new(node_count: usize) -> Self {
        Self {
            edges: vec![Vec::new(); node_count],
        }
    }

    fn add_edge(&mut self, source: usize, destination: usize, capacity: u64) -> usize {
        let forward_index = self.edges[source].len();
        let reverse_index = self.edges[destination].len();
        self.edges[source].push(FlowEdge {
            destination,
            reverse: reverse_index,
            capacity,
        });
        self.edges[destination].push(FlowEdge {
            destination: source,
            reverse: forward_index,
            capacity: 0,
        });
        forward_index
    }

    fn max_flow(&mut self, source: usize, sink: usize) -> u64 {
        let mut total = 0u64;
        loop {
            let mut level = vec![usize::MAX; self.edges.len()];
            level[source] = 0;
            let mut queue = VecDeque::from([source]);
            while let Some(node) = queue.pop_front() {
                for edge in &self.edges[node] {
                    if edge.capacity > 0 && level[edge.destination] == usize::MAX {
                        level[edge.destination] = level[node] + 1;
                        queue.push_back(edge.destination);
                    }
                }
            }
            if level[sink] == usize::MAX {
                return total;
            }

            let mut next_edge = vec![0usize; self.edges.len()];
            loop {
                let sent = self.send_flow(source, sink, u64::MAX, &level, &mut next_edge);
                if sent == 0 {
                    break;
                }
                total = total.saturating_add(sent);
            }
        }
    }

    fn send_flow(
        &mut self,
        node: usize,
        sink: usize,
        offered: u64,
        level: &[usize],
        next_edge: &mut [usize],
    ) -> u64 {
        if node == sink {
            return offered;
        }
        while next_edge[node] < self.edges[node].len() {
            let edge_index = next_edge[node];
            let edge = self.edges[node][edge_index];
            if edge.capacity > 0 && level[edge.destination] == level[node] + 1 {
                let sent = self.send_flow(
                    edge.destination,
                    sink,
                    offered.min(edge.capacity),
                    level,
                    next_edge,
                );
                if sent > 0 {
                    self.edges[node][edge_index].capacity -= sent;
                    self.edges[edge.destination][edge.reverse].capacity += sent;
                    return sent;
                }
            }
            next_edge[node] += 1;
        }
        0
    }
}
