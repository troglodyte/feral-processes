//! The base report hub's gap flags: one reason per page, `None` when the
//! page is fine. The rail's `!` and the Overview read these and nothing
//! else, so each flag is built from the same call its page makes.

use crate::Game;
use crate::views::BaseReportFlags;

impl Game {
    pub fn base_report_flags(&self) -> BaseReportFlags {
        let unread = self.unread_alerts();
        BaseReportFlags {
            production: None,
            alerts: (unread > 0).then(|| format!("{unread} unread")),
        }
    }
}
