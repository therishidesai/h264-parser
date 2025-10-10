use crate::Result;
use std::collections::VecDeque;

pub struct StartCodeScanner {
    buffer: Vec<u8>,
    ready_spans: VecDeque<NalSpan>,
    pending_start: Option<(usize, u8)>,
}

impl StartCodeScanner {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            ready_spans: VecDeque::new(),
            pending_start: None,
        }
    }

    pub fn push(&mut self, data: &[u8]) {
        let prev_len = self.buffer.len();
        self.buffer.extend_from_slice(data);
        let scan_start = prev_len.saturating_sub(3);
        self.scan_for_new_start_codes(scan_start);
    }

    pub fn next_nal_unit(&mut self) -> Result<Option<NalSpan>> {
        if let Some(span) = self.ready_spans.pop_front() {
            return Ok(Some(span));
        }

        self.scan_for_new_start_codes(self.buffer.len().saturating_sub(3));
        Ok(self.ready_spans.pop_front())
    }

    pub fn finish_pending(&mut self) -> Option<NalSpan> {
        self.scan_for_new_start_codes(self.buffer.len().saturating_sub(3));

        if let Some(span) = self.ready_spans.pop_front() {
            return Some(span);
        }

        if let Some((start_pos, start_code_len)) = self.pending_start.take() {
            let data_start = start_pos + start_code_len as usize;
            if data_start < self.buffer.len() {
                return Some(NalSpan {
                    start_pos,
                    start_code_len,
                    data_start,
                    data_end: self.buffer.len(),
                });
            }
        }

        None
    }

    pub fn get_nal_data(&self, span: &NalSpan) -> &[u8] {
        &self.buffer[span.data_start..span.data_end]
    }

    pub fn has_pending_start(&self) -> bool {
        self.pending_start.is_some()
    }

    pub fn consume_processed(&mut self, up_to: usize) {
        if up_to == 0 || up_to > self.buffer.len() {
            return;
        }

        self.buffer.drain(0..up_to);

        if let Some((start_pos, start_code_len)) = self.pending_start {
            if start_pos >= up_to {
                self.pending_start = Some((start_pos - up_to, start_code_len));
            } else if start_pos + start_code_len as usize > up_to {
                self.pending_start = Some((0, start_code_len));
            } else {
                self.pending_start = None;
            }
        }

        for span in &mut self.ready_spans {
            if span.start_pos >= up_to {
                span.start_pos -= up_to;
                span.data_start -= up_to;
                span.data_end -= up_to;
            } else {
                span.start_pos = 0;
                span.data_start = 0;
                span.data_end = span.data_end.saturating_sub(up_to);
            }
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.ready_spans.clear();
        self.pending_start = None;
    }

    fn scan_for_new_start_codes(&mut self, mut index: usize) {
        if index > self.buffer.len() {
            index = self.buffer.len();
        }

        while index + 2 < self.buffer.len() {
            if self.buffer[index] == 0x00 && self.buffer[index + 1] == 0x00 {
                if index + 3 < self.buffer.len()
                    && self.buffer[index + 2] == 0x00
                    && self.buffer[index + 3] == 0x01
                {
                    self.record_start_code(index, 4);
                    index += 1;
                    continue;
                } else if self.buffer[index + 2] == 0x01 {
                    self.record_start_code(index, 3);
                    index += 1;
                    continue;
                }
            }
            index += 1;
        }
    }

    fn record_start_code(&mut self, start_pos: usize, start_code_len: u8) {
        if let Some((pending_pos, pending_len)) = self.pending_start {
            if pending_pos == start_pos {
                return;
            }

            let data_start = pending_pos + pending_len as usize;
            if start_pos < data_start {
                // Overlapping detection within the current start code prefix; ignore.
                return;
            }

            if data_start < start_pos {
                self.ready_spans.push_back(NalSpan {
                    start_pos: pending_pos,
                    start_code_len: pending_len,
                    data_start,
                    data_end: start_pos,
                });
            }
        }

        self.pending_start = Some((start_pos, start_code_len));
    }
}

#[derive(Debug, Clone)]
pub struct NalSpan {
    pub start_pos: usize,
    pub start_code_len: u8,
    pub data_start: usize,
    pub data_end: usize,
}

impl NalSpan {
    pub fn len(&self) -> usize {
        self.data_end - self.data_start
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_start_codes() {
        let mut scanner = StartCodeScanner::new();
        scanner.push(&[0x00, 0x00, 0x01, 0x42, 0x00, 0x00, 0x00, 0x01, 0x43]);

        let nal1 = scanner.next_nal_unit().unwrap().unwrap();
        assert_eq!(nal1.start_code_len, 3);
        assert_eq!(scanner.get_nal_data(&nal1), &[0x42]);

        scanner.consume_processed(nal1.data_end);
        assert!(scanner.next_nal_unit().unwrap().is_none());

        let nal2 = scanner.finish_pending().unwrap();
        assert_eq!(nal2.start_code_len, 4);
        assert_eq!(scanner.get_nal_data(&nal2), &[0x43]);
    }

    #[test]
    fn test_streaming() {
        let mut scanner = StartCodeScanner::new();

        scanner.push(&[0x00, 0x00]);
        assert!(scanner.next_nal_unit().unwrap().is_none());

        scanner.push(&[0x01, 0x42, 0x00]);
        assert!(scanner.next_nal_unit().unwrap().is_none());

        scanner.push(&[0x00, 0x01, 0x43]);
        let first = scanner.next_nal_unit().unwrap().unwrap();
        assert_eq!(scanner.get_nal_data(&first), &[0x42]);
        scanner.consume_processed(first.data_end);

        let second = scanner.finish_pending().unwrap();
        assert_eq!(scanner.get_nal_data(&second), &[0x43]);
    }

    #[test]
    fn test_finish_pending_at_end_of_stream() {
        let mut scanner = StartCodeScanner::new();
        scanner.push(&[0x00, 0x00, 0x01, 0x43, 0x55]);

        assert!(scanner.next_nal_unit().unwrap().is_none());

        let flushed = scanner.finish_pending().unwrap();
        assert_eq!(flushed.start_code_len, 3);
        assert_eq!(scanner.get_nal_data(&flushed), &[0x43, 0x55]);
    }

    #[test]
    fn test_incomplete_start_code_across_chunks() {
        let mut scanner = StartCodeScanner::new();
        scanner.push(&[0x00, 0x00, 0x00]);
        assert!(scanner.next_nal_unit().unwrap().is_none());

        scanner.push(&[0x01, 0x45, 0x00, 0x00]);
        assert!(scanner.next_nal_unit().unwrap().is_none());

        let nal = scanner.finish_pending().unwrap();
        assert_eq!(nal.start_code_len, 4);
        let data = scanner.get_nal_data(&nal);
        assert_eq!(data.first(), Some(&0x45));
    }
}
