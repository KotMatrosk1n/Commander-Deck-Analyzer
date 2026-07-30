//! Content keyed programs for continuous access to the top of a library.
//!
//! The runtime keeps private inspection, public revelation, casting, land
//! plays, and additional land-play capacity separate. It does not bypass
//! normal timing, cost, or land-play limits.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const LIBRARY_ACCESS_COMPILER_VERSION: &str = "library-access-compiler-0.1";
pub const LIBRARY_ACCESS_RUNTIME_VERSION: &str = "library-access-runtime-0.1";

pub type PlayerId = u8;
pub type ObjectId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LibraryCardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Planeswalker,
    Sorcery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryCard {
    pub id: ObjectId,
    pub owner: PlayerId,
    pub card_types: BTreeSet<LibraryCardType>,
}

impl LibraryCard {
    fn is_land(&self) -> bool {
        self.card_types.contains(&LibraryCardType::Land)
    }

    fn is_spell_card(&self) -> bool {
        !self.is_land()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryViewer {
    Controller,
    AllPlayers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryDisclosure {
    PrivateInspection,
    PublicReveal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryAccessScope {
    Controller,
    AllPlayers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryAccessSourceZone {
    Battlefield,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTopAction {
    Cast,
    PlayLand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTimingRule {
    Normal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryCostRule {
    PrintedAndAdditionalCosts,
    NotApplicableToLandPlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryCardFilter {
    AnySpell,
    CardTypes(BTreeSet<LibraryCardType>),
    Land,
}

impl LibraryCardFilter {
    fn matches(&self, card: &LibraryCard) -> bool {
        match self {
            Self::AnySpell => card.is_spell_card(),
            Self::CardTypes(types) => {
                card.is_spell_card()
                    && card
                        .card_types
                        .iter()
                        .any(|card_type| types.contains(card_type))
            }
            Self::Land => card.is_land(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryTopPermission {
    pub action: LibraryTopAction,
    pub filter: LibraryCardFilter,
    pub timing: LibraryTimingRule,
    pub cost: LibraryCostRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryAccessProgramKind {
    InspectTop {
        viewer: LibraryViewer,
        disclosure: LibraryDisclosure,
    },
    TopPermissions(Vec<LibraryTopPermission>),
    AdditionalLandPlays {
        affected: LibraryAccessScope,
        amount: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryAccessProgram {
    exact_source: String,
    semantic_digest: String,
    active_zone: LibraryAccessSourceZone,
    kind: LibraryAccessProgramKind,
}

impl LibraryAccessProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &LibraryAccessProgramKind {
        &self.kind
    }

    pub fn active_zone(&self) -> LibraryAccessSourceZone {
        self.active_zone
    }
}

pub fn compile_library_access_program(source: &str) -> Option<LibraryAccessProgram> {
    let exact_source = source.trim();
    if exact_source != source || exact_source.is_empty() {
        return None;
    }
    let kind = match exact_source {
        "You may look at the top card of your library any time." => {
            LibraryAccessProgramKind::InspectTop {
                viewer: LibraryViewer::Controller,
                disclosure: LibraryDisclosure::PrivateInspection,
            }
        }
        "Play with the top card of your library revealed." => {
            LibraryAccessProgramKind::InspectTop {
                viewer: LibraryViewer::AllPlayers,
                disclosure: LibraryDisclosure::PublicReveal,
            }
        }
        "You may play an additional land on each of your turns." => {
            LibraryAccessProgramKind::AdditionalLandPlays {
                affected: LibraryAccessScope::Controller,
                amount: 1,
            }
        }
        "Each player may play an additional land on each of their turns." => {
            LibraryAccessProgramKind::AdditionalLandPlays {
                affected: LibraryAccessScope::AllPlayers,
                amount: 1,
            }
        }
        "You may cast creature spells from the top of your library." => {
            LibraryAccessProgramKind::TopPermissions(vec![cast_permission([
                LibraryCardType::Creature,
            ])])
        }
        "You may cast instant and sorcery spells from the top of your library." => {
            LibraryAccessProgramKind::TopPermissions(vec![cast_permission([
                LibraryCardType::Instant,
                LibraryCardType::Sorcery,
            ])])
        }
        "You may play lands from the top of your library." => {
            LibraryAccessProgramKind::TopPermissions(vec![land_permission()])
        }
        "You may play lands and cast spells from the top of your library." => {
            LibraryAccessProgramKind::TopPermissions(vec![
                land_permission(),
                LibraryTopPermission {
                    action: LibraryTopAction::Cast,
                    filter: LibraryCardFilter::AnySpell,
                    timing: LibraryTimingRule::Normal,
                    cost: LibraryCostRule::PrintedAndAdditionalCosts,
                },
            ])
        }
        "You may play lands and cast creature spells from the top of your library." => {
            LibraryAccessProgramKind::TopPermissions(vec![
                land_permission(),
                cast_permission([LibraryCardType::Creature]),
            ])
        }
        _ => return None,
    };
    let semantic_digest = library_access_semantic_digest(exact_source, &kind);
    Some(LibraryAccessProgram {
        exact_source: exact_source.to_owned(),
        semantic_digest,
        active_zone: LibraryAccessSourceZone::Battlefield,
        kind,
    })
}

fn cast_permission<const N: usize>(types: [LibraryCardType; N]) -> LibraryTopPermission {
    LibraryTopPermission {
        action: LibraryTopAction::Cast,
        filter: LibraryCardFilter::CardTypes(types.into_iter().collect()),
        timing: LibraryTimingRule::Normal,
        cost: LibraryCostRule::PrintedAndAdditionalCosts,
    }
}

fn land_permission() -> LibraryTopPermission {
    LibraryTopPermission {
        action: LibraryTopAction::PlayLand,
        filter: LibraryCardFilter::Land,
        timing: LibraryTimingRule::Normal,
        cost: LibraryCostRule::NotApplicableToLandPlay,
    }
}

fn library_access_semantic_digest(source: &str, kind: &LibraryAccessProgramKind) -> String {
    let mut hasher = Sha256::new();
    let components = [
        "library-access-content/v1".to_owned(),
        LIBRARY_ACCESS_COMPILER_VERSION.to_owned(),
        LIBRARY_ACCESS_RUNTIME_VERSION.to_owned(),
        source.to_owned(),
        format!("active-zone:{:?}", LibraryAccessSourceZone::Battlefield),
        canonical_program(kind),
    ];
    for component in components {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn canonical_program(kind: &LibraryAccessProgramKind) -> String {
    match kind {
        LibraryAccessProgramKind::InspectTop { viewer, disclosure } => {
            format!("inspect-top/v1/{viewer:?}/{disclosure:?}")
        }
        LibraryAccessProgramKind::AdditionalLandPlays { affected, amount } => {
            format!("additional-land-plays/v1/{affected:?}/{amount}")
        }
        LibraryAccessProgramKind::TopPermissions(permissions) => {
            let permissions = permissions
                .iter()
                .map(|permission| {
                    let filter = match &permission.filter {
                        LibraryCardFilter::AnySpell => "any-spell".to_owned(),
                        LibraryCardFilter::Land => "land".to_owned(),
                        LibraryCardFilter::CardTypes(types) => {
                            format!(
                                "types:{}",
                                types
                                    .iter()
                                    .map(|card_type| format!("{card_type:?}"))
                                    .collect::<Vec<_>>()
                                    .join(",")
                            )
                        }
                    };
                    format!(
                        "{:?}/{filter}/{:?}/{:?}",
                        permission.action, permission.timing, permission.cost
                    )
                })
                .collect::<Vec<_>>()
                .join("|");
            format!("top-permissions/v1/{permissions}")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveLibraryAccessProgram {
    pub source: ObjectId,
    pub controller: PlayerId,
    pub program: LibraryAccessProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryAccessAuthorization {
    pub actor: PlayerId,
    pub card: ObjectId,
    pub action: LibraryTopAction,
    pub grants: Vec<LibraryAccessGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryAccessGrant {
    pub source_program: ObjectId,
    pub timing: LibraryTimingRule,
    pub cost: LibraryCostRule,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryAccessState {
    pub libraries: BTreeMap<PlayerId, Vec<ObjectId>>,
    pub cards: BTreeMap<ObjectId, LibraryCard>,
    pub active_programs: BTreeMap<ObjectId, ActiveLibraryAccessProgram>,
    pub current_turn: Option<PlayerId>,
    pub land_plays_used: BTreeMap<PlayerId, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryAccessError {
    DuplicateSource(ObjectId),
    InactiveSource {
        source: ObjectId,
        expected: LibraryAccessSourceZone,
        actual: LibraryAccessSourceZone,
    },
    MissingCard(ObjectId),
    CardOwnerMismatch {
        card: ObjectId,
        library_owner: PlayerId,
        card_owner: PlayerId,
    },
    DuplicateLibraryCard(ObjectId),
    NotTopCard(ObjectId),
    NotPlayersTurn(PlayerId),
    NoPermission {
        player: PlayerId,
        card: ObjectId,
        action: LibraryTopAction,
    },
    LandPlayLimitReached {
        player: PlayerId,
        used: u32,
        limit: u32,
    },
}

impl fmt::Display for LibraryAccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LibraryAccessError {}

impl LibraryAccessState {
    pub fn set_library(
        &mut self,
        owner: PlayerId,
        ordered_top_first: Vec<ObjectId>,
    ) -> Result<(), LibraryAccessError> {
        let mut seen = BTreeSet::new();
        for card_id in &ordered_top_first {
            if !seen.insert(*card_id) {
                return Err(LibraryAccessError::DuplicateLibraryCard(*card_id));
            }
            let card = self
                .cards
                .get(card_id)
                .ok_or(LibraryAccessError::MissingCard(*card_id))?;
            if card.owner != owner {
                return Err(LibraryAccessError::CardOwnerMismatch {
                    card: *card_id,
                    library_owner: owner,
                    card_owner: card.owner,
                });
            }
        }
        self.libraries.insert(owner, ordered_top_first);
        Ok(())
    }

    pub fn activate(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        source_zone: LibraryAccessSourceZone,
        program: LibraryAccessProgram,
    ) -> Result<(), LibraryAccessError> {
        if self.active_programs.contains_key(&source) {
            return Err(LibraryAccessError::DuplicateSource(source));
        }
        if source_zone != program.active_zone() {
            return Err(LibraryAccessError::InactiveSource {
                source,
                expected: program.active_zone(),
                actual: source_zone,
            });
        }
        self.active_programs.insert(
            source,
            ActiveLibraryAccessProgram {
                source,
                controller,
                program,
            },
        );
        Ok(())
    }

    pub fn deactivate(&mut self, source: ObjectId) {
        self.active_programs.remove(&source);
    }

    pub fn begin_turn(&mut self, player: PlayerId) {
        self.current_turn = Some(player);
        self.land_plays_used.insert(player, 0);
    }

    pub fn top_card(&self, owner: PlayerId) -> Option<ObjectId> {
        self.libraries
            .get(&owner)
            .and_then(|library| library.first())
            .copied()
    }

    pub fn player_may_inspect_top(&self, viewer: PlayerId, owner: PlayerId) -> bool {
        self.active_programs.values().any(|active| {
            active.controller == owner
                && matches!(
                    active.program.kind(),
                    LibraryAccessProgramKind::InspectTop {
                        viewer: LibraryViewer::Controller,
                        disclosure: LibraryDisclosure::PrivateInspection,
                    }
                )
                && viewer == owner
        }) || self.top_is_publicly_revealed(owner)
    }

    pub fn top_is_publicly_revealed(&self, owner: PlayerId) -> bool {
        self.active_programs.values().any(|active| {
            active.controller == owner
                && matches!(
                    active.program.kind(),
                    LibraryAccessProgramKind::InspectTop {
                        viewer: LibraryViewer::AllPlayers,
                        disclosure: LibraryDisclosure::PublicReveal,
                    }
                )
        })
    }

    pub fn land_play_limit(&self, player: PlayerId) -> u32 {
        if self.current_turn != Some(player) {
            return 0;
        }
        1u32.saturating_add(
            self.active_programs
                .values()
                .filter_map(|active| match active.program.kind() {
                    LibraryAccessProgramKind::AdditionalLandPlays {
                        affected: LibraryAccessScope::Controller,
                        amount,
                    } if active.controller == player => Some(*amount),
                    LibraryAccessProgramKind::AdditionalLandPlays {
                        affected: LibraryAccessScope::AllPlayers,
                        amount,
                    } => Some(*amount),
                    _ => None,
                })
                .fold(0u32, u32::saturating_add),
        )
    }

    pub fn authorize_top_action(
        &self,
        actor: PlayerId,
        card: ObjectId,
        action: LibraryTopAction,
    ) -> Result<LibraryAccessAuthorization, LibraryAccessError> {
        if self.top_card(actor) != Some(card) {
            return Err(LibraryAccessError::NotTopCard(card));
        }
        let card_state = self
            .cards
            .get(&card)
            .ok_or(LibraryAccessError::MissingCard(card))?;
        let mut grants = Vec::new();
        for active in self
            .active_programs
            .values()
            .filter(|active| active.controller == actor)
        {
            let LibraryAccessProgramKind::TopPermissions(permissions) = active.program.kind()
            else {
                continue;
            };
            for permission in permissions.iter().filter(|permission| {
                permission.action == action && permission.filter.matches(card_state)
            }) {
                grants.push(LibraryAccessGrant {
                    source_program: active.source,
                    timing: permission.timing,
                    cost: permission.cost,
                });
            }
        }
        if grants.is_empty() {
            return Err(LibraryAccessError::NoPermission {
                player: actor,
                card,
                action,
            });
        }
        if action == LibraryTopAction::PlayLand {
            if self.current_turn != Some(actor) {
                return Err(LibraryAccessError::NotPlayersTurn(actor));
            }
            let used = self.land_plays_used.get(&actor).copied().unwrap_or(0);
            let limit = self.land_play_limit(actor);
            if used >= limit {
                return Err(LibraryAccessError::LandPlayLimitReached {
                    player: actor,
                    used,
                    limit,
                });
            }
        }
        grants.sort_by_key(|grant| grant.source_program);
        Ok(LibraryAccessAuthorization {
            actor,
            card,
            action,
            grants,
        })
    }

    pub fn record_land_play_from_top(
        &mut self,
        actor: PlayerId,
        card: ObjectId,
    ) -> Result<LibraryAccessAuthorization, LibraryAccessError> {
        let authorization = self.authorize_top_action(actor, card, LibraryTopAction::PlayLand)?;
        *self.land_plays_used.entry(actor).or_default() += 1;
        Ok(authorization)
    }
}
