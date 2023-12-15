/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

const TAG_KEY_LOGGER: &str = "logger";
const TAG_KEY_DROP_TYPE: &str = "drop_type";

const METRIC_NAME_MESSAGE_TOTAL: &str = "logger.message.total";
const METRIC_NAME_MESSAGE_PASS: &str = "logger.message.pass";
const METRIC_NAME_TRAFFIC_PASS: &str = "logger.traffic.pass";
const METRIC_NAME_MESSAGE_DROP: &str = "logger.message.drop";

use g3_statsd_client::{StatsdClient, StatsdTagGroup};
use g3_types::log::{LogDropSnapshot, LogDropType, LogIoSnapshot};
use g3_types::stats::StatId;

use super::TAG_KEY_STAT_ID;

pub(crate) trait LoggerMetricExt {
    fn add_logger_tags(&mut self, logger: &str, stat_id: StatId);
}

impl LoggerMetricExt for StatsdTagGroup {
    fn add_logger_tags(&mut self, logger: &str, stat_id: StatId) {
        let mut buffer = itoa::Buffer::new();
        let stat_id = buffer.format(stat_id.as_u64());

        self.add_tag(TAG_KEY_LOGGER, logger);
        self.add_tag(TAG_KEY_STAT_ID, stat_id);
    }
}

pub(crate) fn emit_log_io_stats(
    client: &mut StatsdClient,
    stats: &LogIoSnapshot,
    snap: &mut LogIoSnapshot,
    common_tags: &StatsdTagGroup,
) {
    macro_rules! emit_field {
        ($field:ident, $name:expr) => {
            let new_value = stats.$field;
            let diff_value = new_value.wrapping_sub(snap.$field);
            client
                .count_with_tags($name, diff_value, common_tags)
                .send();
            snap.$field = new_value;
        };
    }

    emit_field!(total, METRIC_NAME_MESSAGE_TOTAL);
    emit_field!(passed, METRIC_NAME_MESSAGE_PASS);
    emit_field!(size, METRIC_NAME_TRAFFIC_PASS);
}

pub(crate) fn emit_log_drop_stats(
    client: &mut StatsdClient,
    stats: &LogDropSnapshot,
    snap: &mut LogDropSnapshot,
    common_tags: &StatsdTagGroup,
) {
    macro_rules! emit_field {
        ($field:ident, $drop_type:expr) => {
            let new_value = stats.$field;
            if new_value != 0 || snap.$field != 0 {
                let diff_value = new_value.wrapping_sub(snap.$field);
                client
                    .count_with_tags(METRIC_NAME_MESSAGE_DROP, diff_value, common_tags)
                    .with_tag(TAG_KEY_DROP_TYPE, $drop_type)
                    .send();
                snap.$field = new_value;
            }
        };
    }

    emit_field!(format_failed, LogDropType::FormatFailed);
    emit_field!(channel_closed, LogDropType::ChannelClosed);
    emit_field!(channel_overflow, LogDropType::ChannelOverflow);
    emit_field!(peer_unreachable, LogDropType::PeerUnreachable);
}
