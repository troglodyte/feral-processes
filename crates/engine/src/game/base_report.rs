//! The base report hub's gap flags: one reason per page, `None` when the
//! page is fine. The rail's `!` and the Overview read these and nothing
//! else, so each flag is built from the same call its page makes.

use crate::Game;
use crate::views::{BaseReportFlags, OrderState};

impl Game {
    pub fn base_report_flags(&self) -> BaseReportFlags {
        let unread = self.unread_alerts();
        BaseReportFlags {
            production: None,
            staff: self.staff_flag(),
            work_orders: self.work_orders_flag(),
            staffing: self.staffing_flag(),
            alerts: (unread > 0).then(|| format!("{unread} unread")),
        }
    }

    /// How many orders are stalled ("N stalled"), or `None` when every order is
    /// either moving, waiting its turn or satisfied.
    fn work_orders_flag(&self) -> Option<String> {
        let stalled = self
            .work_order_report()
            .iter()
            .filter(|o| o.state == OrderState::Stalled)
            .count();
        (stalled > 0).then(|| format!("{stalled} stalled"))
    }

    /// Posts the duty table leaves unworked: `unworked_total`, the figure
    /// the Duties screen's header prints.
    fn staffing_flag(&self) -> Option<String> {
        let unworked = self.unworked_total();
        (unworked > 0).then(|| format!("{unworked} unworked"))
    }
}
