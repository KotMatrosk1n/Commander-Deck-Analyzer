//! Typed payment for special resource symbols.
//!
//! This module is intentionally independent from the production execution
//! bridge. It validates a complete payment against a staged resource state and
//! commits only after every symbol succeeds.

use std::collections::BTreeMap;
use std::fmt;

pub const SPECIAL_RESOURCE_RUNTIME_VERSION: &str = "special-resource-payment-0.1";

pub type PlayerId = u8;
pub type ObjectId = u64;
pub type ManaUnitId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PhyrexianColor {
    White,
    Blue,
    Black,
    Red,
    Green,
}

impl PhyrexianColor {
    fn mana_color(self) -> ManaColor {
        match self {
            Self::White => ManaColor::White,
            Self::Blue => ManaColor::Blue,
            Self::Black => ManaColor::Black,
            Self::Red => ManaColor::Red,
            Self::Green => ManaColor::Green,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManaSourceProvenance {
    SnowSource { object: ObjectId },
    NonSnowSource { object: ObjectId },
}

impl ManaSourceProvenance {
    fn is_snow_source(self) -> bool {
        matches!(self, Self::SnowSource { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    pub source: ManaSourceProvenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialCostSymbol {
    Energy,
    Ticket,
    Snow,
    Phyrexian(PhyrexianColor),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialResourceCost {
    exact_oracle: String,
    symbols: Vec<SpecialCostSymbol>,
}

impl SpecialResourceCost {
    pub fn from_compiled_symbols(
        exact_oracle: impl Into<String>,
        symbols: Vec<SpecialCostSymbol>,
    ) -> Result<Self, SpecialCostParseError> {
        let exact_oracle = exact_oracle.into();
        if exact_oracle.is_empty() || symbols.is_empty() {
            return Err(SpecialCostParseError::EmptyCost);
        }
        Ok(Self {
            exact_oracle,
            symbols,
        })
    }

    pub fn parse_exact(exact_oracle: &str) -> Result<Self, SpecialCostParseError> {
        if exact_oracle.is_empty() {
            return Err(SpecialCostParseError::EmptyCost);
        }

        const TOKENS: &[(&str, SpecialCostSymbol)] = &[
            ("{TK}", SpecialCostSymbol::Ticket),
            ("{E}", SpecialCostSymbol::Energy),
            ("{S}", SpecialCostSymbol::Snow),
            ("{W/P}", SpecialCostSymbol::Phyrexian(PhyrexianColor::White)),
            ("{U/P}", SpecialCostSymbol::Phyrexian(PhyrexianColor::Blue)),
            ("{B/P}", SpecialCostSymbol::Phyrexian(PhyrexianColor::Black)),
            ("{R/P}", SpecialCostSymbol::Phyrexian(PhyrexianColor::Red)),
            ("{G/P}", SpecialCostSymbol::Phyrexian(PhyrexianColor::Green)),
        ];

        let mut symbols = Vec::new();
        let mut offset = 0;
        while offset < exact_oracle.len() {
            let remaining = &exact_oracle[offset..];
            if let Some((token, symbol)) = TOKENS
                .iter()
                .find(|(token, _)| remaining.starts_with(*token))
            {
                symbols.push(*symbol);
                offset += token.len();
                continue;
            }

            if let Some(symbol_end) = remaining.find('}') {
                let symbol = &remaining[..=symbol_end];
                return Err(SpecialCostParseError::UnsupportedSymbol {
                    offset,
                    symbol: symbol.to_owned(),
                });
            }
            return Err(SpecialCostParseError::MalformedCost { offset });
        }

        Self::from_compiled_symbols(exact_oracle, symbols)
    }

    pub fn exact_oracle(&self) -> &str {
        &self.exact_oracle
    }

    pub fn symbols(&self) -> &[SpecialCostSymbol] {
        &self.symbols
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialCostParseError {
    EmptyCost,
    MalformedCost { offset: usize },
    UnsupportedSymbol { offset: usize, symbol: String },
}

impl fmt::Display for SpecialCostParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCost => write!(formatter, "the special resource cost is empty"),
            Self::MalformedCost { offset } => {
                write!(formatter, "the cost is malformed at byte {offset}")
            }
            Self::UnsupportedSymbol { offset, symbol } => {
                write!(
                    formatter,
                    "unsupported special resource symbol {symbol} at byte {offset}"
                )
            }
        }
    }
}

impl std::error::Error for SpecialCostParseError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentChoice {
    Energy,
    Ticket,
    Mana(ManaUnitId),
    Life(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExplicitSymbolPayment {
    pub symbol_index: usize,
    pub choice: PaymentChoice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSpecialResources {
    pub player: PlayerId,
    pub life_total: i32,
    pub energy: u32,
    pub tickets: u32,
    pub mana_pool: Vec<ManaUnit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBalances {
    pub life_total: i32,
    pub energy: u32,
    pub tickets: u32,
    pub mana_units: usize,
}

impl From<&PlayerSpecialResources> for ResourceBalances {
    fn from(resources: &PlayerSpecialResources) -> Self {
        Self {
            life_total: resources.life_total,
            energy: resources.energy,
            tickets: resources.tickets,
            mana_units: resources.mana_pool.len(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedSymbolPayment {
    Energy,
    Ticket,
    SnowMana {
        mana: ManaUnit,
    },
    PhyrexianMana {
        color: PhyrexianColor,
        mana: ManaUnit,
    },
    PhyrexianLife {
        color: PhyrexianColor,
        amount: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolPaymentReceipt {
    pub symbol_index: usize,
    pub symbol: SpecialCostSymbol,
    pub payment: ResolvedSymbolPayment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialResourcePaymentReceipt {
    pub runtime_version: &'static str,
    pub player: PlayerId,
    pub exact_cost: String,
    pub payments: Vec<SymbolPaymentReceipt>,
    pub before: ResourceBalances,
    pub after: ResourceBalances,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialPaymentError {
    PlayerMismatch {
        requested: PlayerId,
        resource_owner: PlayerId,
    },
    DuplicateManaUnit {
        mana_unit: ManaUnitId,
    },
    UnexpectedChoice {
        symbol_index: usize,
    },
    DuplicateChoice {
        symbol_index: usize,
    },
    MissingChoice {
        symbol_index: usize,
    },
    WrongChoiceKind {
        symbol_index: usize,
        symbol: SpecialCostSymbol,
        choice: PaymentChoice,
    },
    InvalidPhyrexianLifeAmount {
        symbol_index: usize,
        amount: u32,
    },
    InsufficientEnergy {
        required: u32,
        available: u32,
    },
    InsufficientTickets {
        required: u32,
        available: u32,
    },
    InsufficientLife {
        required: u32,
        available: i32,
    },
    UnknownManaUnit {
        symbol_index: usize,
        mana_unit: ManaUnitId,
    },
    ManaUnitAlreadySelected {
        mana_unit: ManaUnitId,
        first_symbol_index: usize,
        second_symbol_index: usize,
    },
    ManaNotFromSnowSource {
        symbol_index: usize,
        mana_unit: ManaUnitId,
    },
    PhyrexianManaColorMismatch {
        symbol_index: usize,
        required: PhyrexianColor,
        actual: ManaColor,
    },
}

impl fmt::Display for SpecialPaymentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SpecialPaymentError {}

pub fn pay_special_resource_cost(
    player: PlayerId,
    cost: &SpecialResourceCost,
    explicit_payments: &[ExplicitSymbolPayment],
    resources: &mut PlayerSpecialResources,
) -> Result<SpecialResourcePaymentReceipt, SpecialPaymentError> {
    if resources.player != player {
        return Err(SpecialPaymentError::PlayerMismatch {
            requested: player,
            resource_owner: resources.player,
        });
    }
    validate_mana_pool(resources)?;
    let choices = bind_explicit_choices(cost, explicit_payments)?;
    validate_balances(cost, &choices, resources)?;
    validate_mana_selections(cost, &choices, resources)?;

    let before = ResourceBalances::from(&*resources);
    let mut staged = resources.clone();
    let mut payments = Vec::with_capacity(cost.symbols.len());

    for (symbol_index, (symbol, choice)) in cost
        .symbols
        .iter()
        .copied()
        .zip(choices.iter().copied())
        .enumerate()
    {
        let payment = match (symbol, choice) {
            (SpecialCostSymbol::Energy, PaymentChoice::Energy) => {
                staged.energy -= 1;
                ResolvedSymbolPayment::Energy
            }
            (SpecialCostSymbol::Ticket, PaymentChoice::Ticket) => {
                staged.tickets -= 1;
                ResolvedSymbolPayment::Ticket
            }
            (SpecialCostSymbol::Snow, PaymentChoice::Mana(mana_unit)) => {
                let mana = remove_mana_unit(&mut staged.mana_pool, mana_unit);
                ResolvedSymbolPayment::SnowMana { mana }
            }
            (SpecialCostSymbol::Phyrexian(color), PaymentChoice::Mana(mana_unit)) => {
                let mana = remove_mana_unit(&mut staged.mana_pool, mana_unit);
                ResolvedSymbolPayment::PhyrexianMana { color, mana }
            }
            (SpecialCostSymbol::Phyrexian(color), PaymentChoice::Life(amount)) => {
                staged.life_total -= amount as i32;
                ResolvedSymbolPayment::PhyrexianLife { color, amount }
            }
            _ => unreachable!("all payment choices are validated before staging"),
        };
        payments.push(SymbolPaymentReceipt {
            symbol_index,
            symbol,
            payment,
        });
    }

    let after = ResourceBalances::from(&staged);
    *resources = staged;
    Ok(SpecialResourcePaymentReceipt {
        runtime_version: SPECIAL_RESOURCE_RUNTIME_VERSION,
        player,
        exact_cost: cost.exact_oracle.clone(),
        payments,
        before,
        after,
    })
}

fn validate_mana_pool(resources: &PlayerSpecialResources) -> Result<(), SpecialPaymentError> {
    let mut seen = BTreeMap::<ManaUnitId, ()>::new();
    for mana in &resources.mana_pool {
        if seen.insert(mana.id, ()).is_some() {
            return Err(SpecialPaymentError::DuplicateManaUnit { mana_unit: mana.id });
        }
    }
    Ok(())
}

fn bind_explicit_choices(
    cost: &SpecialResourceCost,
    explicit_payments: &[ExplicitSymbolPayment],
) -> Result<Vec<PaymentChoice>, SpecialPaymentError> {
    let mut choices = vec![None; cost.symbols.len()];
    for explicit in explicit_payments {
        let Some(slot) = choices.get_mut(explicit.symbol_index) else {
            return Err(SpecialPaymentError::UnexpectedChoice {
                symbol_index: explicit.symbol_index,
            });
        };
        if slot.replace(explicit.choice).is_some() {
            return Err(SpecialPaymentError::DuplicateChoice {
                symbol_index: explicit.symbol_index,
            });
        }
    }

    choices
        .into_iter()
        .enumerate()
        .map(|(symbol_index, choice)| {
            choice.ok_or(SpecialPaymentError::MissingChoice { symbol_index })
        })
        .collect()
}

fn validate_balances(
    cost: &SpecialResourceCost,
    choices: &[PaymentChoice],
    resources: &PlayerSpecialResources,
) -> Result<(), SpecialPaymentError> {
    let mut required_energy = 0u32;
    let mut required_tickets = 0u32;
    let mut required_life = 0u32;

    for (symbol_index, (symbol, choice)) in cost
        .symbols
        .iter()
        .copied()
        .zip(choices.iter().copied())
        .enumerate()
    {
        match (symbol, choice) {
            (SpecialCostSymbol::Energy, PaymentChoice::Energy) => {
                required_energy += 1;
            }
            (SpecialCostSymbol::Ticket, PaymentChoice::Ticket) => {
                required_tickets += 1;
            }
            (SpecialCostSymbol::Snow, PaymentChoice::Mana(_))
            | (SpecialCostSymbol::Phyrexian(_), PaymentChoice::Mana(_)) => {}
            (SpecialCostSymbol::Phyrexian(_), PaymentChoice::Life(2)) => {
                required_life += 2;
            }
            (SpecialCostSymbol::Phyrexian(_), PaymentChoice::Life(amount)) => {
                return Err(SpecialPaymentError::InvalidPhyrexianLifeAmount {
                    symbol_index,
                    amount,
                });
            }
            _ => {
                return Err(SpecialPaymentError::WrongChoiceKind {
                    symbol_index,
                    symbol,
                    choice,
                });
            }
        }
    }

    if resources.energy < required_energy {
        return Err(SpecialPaymentError::InsufficientEnergy {
            required: required_energy,
            available: resources.energy,
        });
    }
    if resources.tickets < required_tickets {
        return Err(SpecialPaymentError::InsufficientTickets {
            required: required_tickets,
            available: resources.tickets,
        });
    }
    if resources.life_total < required_life as i32 {
        return Err(SpecialPaymentError::InsufficientLife {
            required: required_life,
            available: resources.life_total,
        });
    }
    Ok(())
}

fn validate_mana_selections(
    cost: &SpecialResourceCost,
    choices: &[PaymentChoice],
    resources: &PlayerSpecialResources,
) -> Result<(), SpecialPaymentError> {
    let mana_by_id = resources
        .mana_pool
        .iter()
        .map(|mana| (mana.id, *mana))
        .collect::<BTreeMap<_, _>>();
    let mut first_selection = BTreeMap::<ManaUnitId, usize>::new();

    for (symbol_index, (symbol, choice)) in cost
        .symbols
        .iter()
        .copied()
        .zip(choices.iter().copied())
        .enumerate()
    {
        let PaymentChoice::Mana(mana_unit) = choice else {
            continue;
        };
        if let Some(first_symbol_index) = first_selection.insert(mana_unit, symbol_index) {
            return Err(SpecialPaymentError::ManaUnitAlreadySelected {
                mana_unit,
                first_symbol_index,
                second_symbol_index: symbol_index,
            });
        }
        let mana = mana_by_id
            .get(&mana_unit)
            .ok_or(SpecialPaymentError::UnknownManaUnit {
                symbol_index,
                mana_unit,
            })?;
        match symbol {
            SpecialCostSymbol::Snow if !mana.source.is_snow_source() => {
                return Err(SpecialPaymentError::ManaNotFromSnowSource {
                    symbol_index,
                    mana_unit,
                });
            }
            SpecialCostSymbol::Phyrexian(required) if mana.color != required.mana_color() => {
                return Err(SpecialPaymentError::PhyrexianManaColorMismatch {
                    symbol_index,
                    required,
                    actual: mana.color,
                });
            }
            SpecialCostSymbol::Snow | SpecialCostSymbol::Phyrexian(_) => {}
            SpecialCostSymbol::Energy | SpecialCostSymbol::Ticket => {
                unreachable!("mana choices for player resources are rejected before this pass")
            }
        }
    }
    Ok(())
}

fn remove_mana_unit(mana_pool: &mut Vec<ManaUnit>, mana_unit: ManaUnitId) -> ManaUnit {
    let position = mana_pool
        .iter()
        .position(|mana| mana.id == mana_unit)
        .expect("validated mana unit remains present in staged state");
    mana_pool.remove(position)
}
