/// Active screen in the gt-tui application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveScreen {
    Dashboard,
    EventFeed,
    Convoys,
    Agents,
    Mail,
    Beads,
    Docs,
}

impl ActiveScreen {
    pub const ALL: &[Self] = &[
        Self::Dashboard,
        Self::EventFeed,
        Self::Convoys,
        Self::Agents,
        Self::Mail,
        Self::Beads,
        Self::Docs,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::EventFeed => "Events",
            Self::Convoys => "Convoys",
            Self::Agents => "Agents",
            Self::Mail => "Mail",
            Self::Beads => "Beads",
            Self::Docs => "Docs",
        }
    }

    pub fn f_key(self) -> u8 {
        match self {
            Self::Dashboard => 1,
            Self::EventFeed => 2,
            Self::Convoys => 3,
            Self::Agents => 4,
            Self::Mail => 5,
            Self::Beads => 6,
            Self::Docs => 7,
        }
    }

    pub fn from_f_key(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::Dashboard),
            2 => Some(Self::EventFeed),
            3 => Some(Self::Convoys),
            4 => Some(Self::Agents),
            5 => Some(Self::Mail),
            6 => Some(Self::Beads),
            7 => Some(Self::Docs),
            _ => None,
        }
    }

    pub fn from_number_key(ch: char) -> Option<Self> {
        match ch {
            '1' => Some(Self::Dashboard),
            '2' => Some(Self::EventFeed),
            '3' => Some(Self::Convoys),
            '4' => Some(Self::Agents),
            '5' => Some(Self::Mail),
            '6' => Some(Self::Beads),
            '7' => Some(Self::Docs),
            _ => None,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Dashboard => Self::EventFeed,
            Self::EventFeed => Self::Convoys,
            Self::Convoys => Self::Agents,
            Self::Agents => Self::Mail,
            Self::Mail => Self::Beads,
            Self::Beads => Self::Docs,
            Self::Docs => Self::Dashboard,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Dashboard => Self::Docs,
            Self::EventFeed => Self::Dashboard,
            Self::Convoys => Self::EventFeed,
            Self::Agents => Self::Convoys,
            Self::Mail => Self::Agents,
            Self::Beads => Self::Mail,
            Self::Docs => Self::Beads,
        }
    }
}
