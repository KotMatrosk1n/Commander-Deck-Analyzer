//! Exact parsing for printed mana costs.
//!
//! The parser is deliberately closed. A symbol that is not represented by a
//! typed variant is rejected instead of being treated as generic mana. Costs
//! with multiple faces remain separate.

use std::fmt;

use crate::strict_engine::{ManaColor, ManaSymbol as StrictManaSymbol};

pub(crate) const PRINTED_COST_RUNTIME_VERSION: &str = "printed-cost-runtime-0.1";
pub(crate) const PRINTED_COST_PAYMENT_BRIDGE_VERSION: &str = "printed-cost-payment-bridge-0.1";

/// One variable letter has one declared value for the complete cost. Repeated
/// appearances of that letter each contribute the declared value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum VariableManaSymbol {
    Y,
    Z,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrintedManaCostFace {
    pub raw: String,
    /// An empty Scryfall cost is no mana cost, not a zero mana cost.
    pub has_mana_cost: bool,
    pub symbols: Vec<PrintedManaSymbol>,
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
