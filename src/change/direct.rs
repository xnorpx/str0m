use crate::sdp::Setup;
use crate::Rtc;
use crate::RtcError;

use super::Change;
use super::ChangeSet;
use super::ChangeStrategy;

/// Direct change strategy.
///
/// Makes immediate changes to the Rtc session without any Sdp OFFER/ANSWER or similar.
pub struct DirectStrategy;

impl ChangeStrategy for DirectStrategy {
    type Apply = Result<(), RtcError>;

    fn apply(
        &self,
        _change_id: usize,
        rtc: &mut crate::Rtc,
        changes: super::ChangesWrapper,
    ) -> Self::Apply {
        let changes = changes.0;
        apply_changes(rtc, changes)
    }
}

impl ChangeSet<'_, DirectStrategy> {
    /// Start the DTLS subsystem.
    pub fn start_dtls(&mut self, active: bool) {
        // Don't start if it's already started.
        if !self.rtc.dtls.is_inited() {
            self.changes.push(Change::StartDtls(active));
        }
    }
}

fn apply_changes(rtc: &mut Rtc, changes: super::Changes) -> Result<(), RtcError> {
    for c in changes.0.into_iter() {
        match c {
            Change::AddMedia(_) => todo!(),
            Change::AddApp(_) => todo!(),
            Change::AddChannel(_, _) => todo!(),
            Change::Direction(_, _) => todo!(),
            Change::StartDtls(active) => start_dtls(rtc, active)?,
        }
    }
    Ok(())
}

fn start_dtls(rtc: &mut Rtc, active: bool) -> Result<(), RtcError> {
    // TODO: Remove this SDP-ism. Need some work to remove Rtc::setup field.
    let remote_setup = if active {
        Setup::Passive
    } else {
        Setup::Active
    };

    rtc.init_dtls(remote_setup)
}
