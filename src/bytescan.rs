use crate::Result;

pub struct StartCodeScanner {
    buffer: Vec<u8>,
    position: usize,
}

impl StartCodeScanner {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            position: 0,
        }
    }

    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    pub fn find_next_start_code(&mut self) -> Option<(usize, u8)> {
        while self.position + 2 < self.buffer.len() {
            if self.buffer[self.position] == 0x00 && self.buffer[self.position + 1] == 0x00 {
                if self.position + 3 < self.buffer.len()
                    && self.buffer[self.position + 2] == 0x00
                    && self.buffer[self.position + 3] == 0x01
                {
                    let pos = self.position;
                    self.position += 4;
                    return Some((pos, 4));
                } else if self.buffer[self.position + 2] == 0x01 {
                    let pos = self.position;
                    self.position += 3;
                    return Some((pos, 3));
                }
            }
            self.position += 1;
        }
        None
    }

    pub fn next_nal_unit(&mut self) -> Result<Option<NalSpan>> {
        if let Some((start_pos, start_code_len)) = self.find_next_start_code() {
            let data_start = start_pos + start_code_len as usize;

            // Save current position to search for next start code
            let saved_pos = self.position;
            let next_start = self.find_next_start_code();
            
            let data_end = if let Some((next_pos, _)) = next_start {
                // Restore position to the beginning of the next start code
                self.position = next_pos;
                next_pos
            } else {
                // No next start code found, this is the last NAL
                // Keep position at end of buffer
                self.buffer.len()
            };

            if data_start >= data_end {
                // Restore position if NAL is empty
                self.position = saved_pos;
                return Ok(None);
            }

            Ok(Some(NalSpan {
                start_pos,
                start_code_len,
                data_start,
                data_end,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_nal_data(&self, span: &NalSpan) -> &[u8] {
        &self.buffer[span.data_start..span.data_end]
    }

    pub fn consume_processed(&mut self, up_to: usize) {
        if up_to > 0 {
            self.buffer.drain(0..up_to);
            self.position = self.position.saturating_sub(up_to);
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.position = 0;
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

        let nal2 = scanner.next_nal_unit().unwrap().unwrap();
        assert_eq!(nal2.start_code_len, 4);
        assert_eq!(scanner.get_nal_data(&nal2), &[0x43]);
    }

    #[test]
    fn test_streaming() {
        let mut scanner = StartCodeScanner::new();
        
        scanner.push(&[0x00, 0x00]);
        assert!(scanner.next_nal_unit().unwrap().is_none());
        
        scanner.push(&[0x01, 0x42, 0x00]);
        let nal = scanner.next_nal_unit().unwrap();
        assert!(nal.is_some());
        
        scanner.push(&[0x00, 0x01, 0x43]);
        let nal = scanner.next_nal_unit().unwrap().unwrap();
        assert_eq!(scanner.get_nal_data(&nal), &[0x43]);
    }
}