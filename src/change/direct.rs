use sctp_proto::ReliabilityType;

use crate::channel::ChannelId;
use crate::dtls;
use crate::dtls::Fingerprint;
use crate::error::SctpError;
use crate::ice;
use crate::sctp::DcepOpen;
use crate::IceCreds;
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

    /// local ice credentials
    pub fn local_ice_credentials(&self) -> &ice::IceCreds {
        self.rtc.ice.local_credentials()
    }

    /// set ice controlling
    pub fn ice_controlling(&mut self, controlling: bool) {
        self.rtc.ice.set_controlling(controlling);
    }

    /// remote ice credentials
    pub fn remote_ice_credentials(&mut self, remote_ice_credentials: IceCreds) {
        self.rtc.ice.set_remote_credentials(remote_ice_credentials);
    }

    /// remote fingerprint
    pub fn remote_fingerprint(&mut self, dtls_fingerprint: &Fingerprint) {
        self.rtc.remote_fingerprint = Some(dtls_fingerprint.clone());
    }

    /// local dtls fingerprint
    pub fn local_dtls_fingerprint(&self) -> &dtls::Fingerprint {
        self.rtc.dtls.local_fingerprint()
    }

    /// todo....
    pub fn open_prenegotiated_stream(&mut self, id: u16, label: String) -> Result<(), SctpError> {
        let dcep = DcepOpen {
            unordered: false,
            channel_type: ReliabilityType::Reliable,
            reliability_parameter: 0,
            label,
            priority: 0,
            protocol: String::new(),
        };
        self.changes
            .push(Change::AddChannel(ChannelId::from(id), dcep, true));
        Ok(())
    }
}

fn apply_changes(rtc: &mut Rtc, changes: super::Changes) -> Result<(), RtcError> {
    for c in changes.0.into_iter() {
        match c {
            Change::AddMedia(_) => todo!(),
            Change::AddApp(_) => todo!(),
            Change::AddChannel(id, dcep, _) => rtc.sctp.open_prenegotiated_stream(*id, dcep),
            Change::Direction(_, _) => todo!(),
            Change::StartDtls(active) => rtc.init_dtls(active)?,
        }
    }
    Ok(())
}
