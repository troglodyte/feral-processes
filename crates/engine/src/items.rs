use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemId {
    CoreFragment,
    PowerCell,
    IceBreaker,
}

impl ItemId {
    pub fn display_name(self) -> &'static str {
        match self {
            ItemId::CoreFragment => "Core Fragment",
            ItemId::PowerCell => "Power Cell",
            ItemId::IceBreaker => "ICE Breaker",
        }
    }
}
