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

use std::io;
use std::time::Instant;

use log::warn;

use g3_types::metrics::MetricsName;

use crate::{StatsdMetricsSink, StatsdTagGroup};

mod formatter;

pub struct StatsdClient {
    prefix: MetricsName,
    sink: StatsdMetricsSink,
    tags: StatsdTagGroup,

    create_instant: Instant,
    last_error_report: u64,
}

impl StatsdClient {
    pub(crate) fn new(prefix: MetricsName, sink: StatsdMetricsSink) -> Self {
        StatsdClient {
            prefix,
            sink,
            tags: Default::default(),
            create_instant: Instant::now(),
            last_error_report: 0,
        }
    }

    pub fn with_tag<T: AsRef<str>>(mut self, key: &str, value: T) -> Self {
        self.tags.add_tag(key, value);
        self
    }

    pub fn with_tag_value<T: AsRef<str>>(mut self, value: T) -> Self {
        self.tags.add_tag_value(value);
        self
    }

    pub fn flush_sink(&mut self) {
        if let Err(e) = self.sink.flush() {
            self.handle_emit_error(e);
        }
    }

    fn handle_emit_error(&mut self, e: io::Error) {
        let time_slice = self.create_instant.elapsed().as_secs().rotate_right(6); // every 64s
        if self.last_error_report != time_slice {
            warn!("sending metrics error: {e:?}");
            self.last_error_report = time_slice;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::sync::Mutex;

    #[test]
    fn count_simple() {
        let buf = Rc::new(Mutex::new(Vec::default()));
        let sink = StatsdMetricsSink::buf_with_capacity(buf.clone(), 32);
        let prefix = unsafe { MetricsName::from_str_unchecked("test") };
        let mut client = StatsdClient::new(prefix, sink);
        client.count("count", 20).send();
        client.flush_sink();

        let buf = buf.lock().unwrap();
        assert_eq!(buf.as_slice(), b"test.count:20|c");
    }

    #[test]
    fn gauge_simple() {
        let buf = Rc::new(Mutex::new(Vec::default()));
        let sink = StatsdMetricsSink::buf_with_capacity(buf.clone(), 32);
        let prefix = unsafe { MetricsName::from_str_unchecked("test") };
        let mut client = StatsdClient::new(prefix, sink);
        client.gauge("gauge", 20).send();
        client.flush_sink();

        let buf = buf.lock().unwrap();
        assert_eq!(buf.as_slice(), b"test.gauge:20|g");
    }

    #[test]
    fn gauge_with_tags_no_prefix() {
        let buf = Rc::new(Mutex::new(Vec::default()));
        let sink = StatsdMetricsSink::buf_with_capacity(buf.clone(), 32);
        let mut client = StatsdClient::new(MetricsName::default(), sink);
        client.gauge("gauge", 20).with_tag("t", "v").send();
        client.flush_sink();

        let buf = buf.lock().unwrap();
        assert_eq!(buf.as_slice(), b"gauge:20|g|#t:v");
    }

    #[test]
    fn count_with_tags() {
        let buf = Rc::new(Mutex::new(Vec::default()));
        let sink = StatsdMetricsSink::buf_with_capacity(buf.clone(), 32);
        let prefix = unsafe { MetricsName::from_str_unchecked("test") };
        let mut client = StatsdClient::new(prefix, sink).with_tag("tag1", "1234");
        client.count("count", 20).with_tag("tag2", "a").send();
        client.flush_sink();

        let buf = buf.lock().unwrap();
        assert_eq!(buf.as_slice(), b"test.count:20|c|#tag1:1234,tag2:a");
    }

    #[test]
    fn count_multiple_simple() {
        let buf = Rc::new(Mutex::new(Vec::default()));
        let sink = StatsdMetricsSink::buf_with_capacity(buf.clone(), 32);
        let prefix = unsafe { MetricsName::from_str_unchecked("test") };
        let mut client = StatsdClient::new(prefix, sink);
        client.count("count", 20).send();
        client.count("count", 30).send();
        client.flush_sink();

        let buf = buf.lock().unwrap();
        assert_eq!(buf.as_slice(), b"test.count:20|c\ntest.count:30|c");
    }

    #[test]
    fn count_multiple_with_tags() {
        let buf = Rc::new(Mutex::new(Vec::default()));
        let sink = StatsdMetricsSink::buf_with_capacity(buf.clone(), 64);
        let prefix = unsafe { MetricsName::from_str_unchecked("test") };

        let mut common_tags = StatsdTagGroup::default();
        common_tags.add_tag("c1", "v1");

        let mut client = StatsdClient::new(prefix, sink);
        client
            .count_with_tags("count", 20, &common_tags)
            .with_tag("c2", "v2")
            .send();
        client.count_with_tags("count", 30, &common_tags).send();
        client.flush_sink();

        let buf = buf.lock().unwrap();
        assert_eq!(
            buf.as_slice(),
            b"test.count:20|c|#c1:v1,c2:v2\ntest.count:30|c|#c1:v1"
        );
    }

    #[test]
    fn count_multiple_overflow() {
        let buf = Rc::new(Mutex::new(Vec::default()));
        let sink = StatsdMetricsSink::buf_with_capacity(buf.clone(), 32);
        let prefix = unsafe { MetricsName::from_str_unchecked("test") };

        let mut common_tags = StatsdTagGroup::default();
        common_tags.add_tag("c1", "v1");

        let mut client = StatsdClient::new(prefix, sink);
        client
            .count_with_tags("count", 20, &common_tags)
            .with_tag("c2", "v2")
            .send();
        client.count_with_tags("count", 30, &common_tags).send();
        client.flush_sink();

        let buf = buf.lock().unwrap();
        assert_eq!(
            buf.as_slice(),
            b"test.count:20|c|#c1:v1,c2:v2test.count:30|c|#c1:v1"
        );
    }
}
