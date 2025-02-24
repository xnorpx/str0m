//! Googcc Bandwidth Estimation based on TWCC feedback as described in
//! <https://datatracker.ietf.org/doc/html/draft-ietf-rmcat-gcc-02> and implemented in libWebRTC.
//!
//! Much of this code has been ported from the libWebRTC implementations. The complete system has
//! not been ported, only a smaller part that corresponds roughly to the IETF draft is implemented.

mod arrival_group;
pub(crate) mod macros;
mod rate_control;
mod trendline_estimator;

use std::collections::VecDeque;
use std::fmt;
use std::time::{Duration, Instant};

use crate::rtp::{Bitrate, SeqNo, TwccSendRecord};

use arrival_group::{ArrivalGroupAccumulator, InterGroupDelayDelta};
use rate_control::RateControl;
use trendline_estimator::TrendlineEstimator;

const MAX_RTT_HISTORY_WINDOW: usize = 32;

/// Main entry point for the Googcc inspired BWE implementation.
///
/// This takes as input packet statuses recorded at send time and enriched by TWCC reports and produces as its output a periodic
/// estimate of the available send bitrate.
pub struct SendSideBandwithEstimator {
    arrival_group_accumulator: ArrivalGroupAccumulator,
    trendline_estimator: TrendlineEstimator,
    rate_control: RateControl,
    /// Last unpolled bitrate estimate. [`None`] before the first poll and after each poll that,
    /// updated when we get a new estimate.
    next_estimate: Option<Bitrate>,
    /// Last estimate produced, unlike [`next_estimate`] this will always have a value after the
    /// first estimate.
    last_estimate: Option<Bitrate>,
    /// History of the max RTT derived for each TWCC report.
    max_rtt_history: VecDeque<Duration>,
}

impl SendSideBandwithEstimator {
    pub fn new(initial_bitrate: Bitrate) -> Self {
        Self {
            arrival_group_accumulator: ArrivalGroupAccumulator::default(),
            trendline_estimator: TrendlineEstimator::new(20),
            rate_control: RateControl::new(initial_bitrate, Bitrate::kbps(40), Bitrate::gbps(10)),
            next_estimate: None,
            last_estimate: None,
            max_rtt_history: VecDeque::default(),
        }
    }

    /// Record a packet from a TWCC report.
    pub(crate) fn update<'t>(
        &mut self,
        records: impl Iterator<Item = &'t TwccSendRecord>,
        observed_bitrate: Bitrate,
        now: Instant,
    ) {
        let mut max_rtt = None;
        for record in records {
            let Ok(acked_packet) = record.try_into() else {
                continue;
            };
            max_rtt = max_rtt.max(record.rtt());

            if let Some(delay_variation) = self
                .arrival_group_accumulator
                .accumulate_packet(acked_packet)
            {
                crate::packet::bwe::macros::log_delay_variation!(delay_variation.delay_delta);

                // Got a new delay variation, add it to the trendline
                self.trendline_estimator
                    .add_delay_observation(delay_variation, now);
            }
        }
        if let Some(rtt) = max_rtt {
            self.add_max_rtt(rtt);
        }

        let new_hypothesis = self.trendline_estimator.hypothesis();

        self.rate_control.update(
            new_hypothesis.into(),
            observed_bitrate,
            self.mean_max_rtt(),
            now,
        );
        let estimated_rate = self.rate_control.estimated_bitrate();

        self.update_estimate(estimated_rate);
    }

    /// Poll for an estimate.
    pub(crate) fn poll_estimate(&mut self) -> Option<Bitrate> {
        self.next_estimate.take()
    }

    /// Get the latest estimate.
    pub(crate) fn last_estimate(&self) -> Option<Bitrate> {
        self.last_estimate
    }

    pub(crate) fn set_is_probing(&mut self, is_probing: bool) {
        self.rate_control.set_is_probing(is_probing);
    }

    fn add_max_rtt(&mut self, max_rtt: Duration) {
        while self.max_rtt_history.len() > MAX_RTT_HISTORY_WINDOW {
            self.max_rtt_history.pop_front();
        }
        self.max_rtt_history.push_back(max_rtt);
    }

    fn mean_max_rtt(&self) -> Option<Duration> {
        if self.max_rtt_history.is_empty() {
            return None;
        }

        let sum = self
            .max_rtt_history
            .iter()
            .fold(Duration::ZERO, |acc, rtt| acc + *rtt);

        Some(sum / self.max_rtt_history.len() as u32)
    }

    fn update_estimate(&mut self, estimated_rate: Bitrate) {
        crate::packet::bwe::macros::log_bitrate_estimate!(estimated_rate.as_f64());
        self.next_estimate = Some(estimated_rate);
        self.last_estimate = Some(estimated_rate);
    }
}

/// A RTP packet that has been sent and acknowledged by the receiver in a TWCC report.
#[derive(Debug, Copy, Clone)]
pub struct AckedPacket {
    /// The TWCC sequence number
    seq_no: SeqNo,
    /// When we sent the packet
    local_send_time: Instant,
    /// When the packet was received at the remote, note this Instant is only usable with other
    /// instants of the same type i..e those that represent a TWCC reported receive time for this
    /// session.
    remote_recv_time: Instant,
}

impl TryFrom<&TwccSendRecord> for AckedPacket {
    type Error = ();

    fn try_from(value: &TwccSendRecord) -> Result<Self, Self::Error> {
        let Some(remote_recv_time) = value.remote_recv_time() else {
            return Err(());
        };

        Ok(Self {
            seq_no: value.seq(),
            local_send_time: value.local_send_time(),
            remote_recv_time,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BandwithUsage {
    Overuse,
    Normal,
    Underuse,
}

impl fmt::Display for BandwithUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BandwithUsage::Overuse => write!(f, "overuse"),
            BandwithUsage::Normal => write!(f, "normal"),
            BandwithUsage::Underuse => write!(f, "underuse"),
        }
    }
}
