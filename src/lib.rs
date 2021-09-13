use std::{io::{Cursor, Read, Seek, SeekFrom, Write}};
use binread::{BinReaderExt};

pub struct ReversedWords<'a> {
    cursor: Cursor<&'a mut [u8]>,
    word_size: u8,
    len: u64,
}

impl<'a> ReversedWords<'a> {
    pub fn new(ram: &'a mut [u8]) -> ReversedWords {
        let len: u64 = ram.len() as u64;
        ReversedWords {
            cursor: Cursor::new(ram),
            word_size: 4, // read u32 words at a time
            len,
        }
    }

    pub fn new_with_word_size(ram: &'a mut [u8], word_size: u8) -> ReversedWords {
        let len: u64 = ram.len() as u64;
        ReversedWords {
            cursor: Cursor::new(ram),
            word_size,
            len,
        }
    }

}

impl Seek for ReversedWords<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.cursor.seek(pos)
    }

    fn stream_position(&mut self) -> std::io::Result<u64> {
        self.cursor.stream_position()
    }
}

impl Write for ReversedWords<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut misalignment = self.cursor.position() as usize % (self.word_size as usize);
        if misalignment > 0 {
            // back up by the amount of the misalignment
            self.seek(SeekFrom::Current(misalignment as i64 * -1))?;
        }

        let mut writes: Vec<(usize, &u8)> = buf
            .iter()
            .enumerate() // Add index
            .map(|(index, byte)| { // Write a word's bytes in reverse order, use cursor position to determine where we are within a word
                let word_num = (index + misalignment) / (self.word_size as usize);
                let word_start_index = word_num * self.word_size as usize;
                let position_within_unflipped_word = ((index + misalignment) % self.word_size as usize) as usize;
                let position_within_flipped_word = self.word_size as usize - 1 - position_within_unflipped_word;
                (word_start_index + position_within_flipped_word, byte)
            }).collect();

        // sort so the smallest target indices are first.
        writes.sort_by(|(a_index, _), (b_index, _)| a_index.cmp(b_index));

        let start_position = self.cursor.position();
        let mut num_bytes_written = 0;
        for (write_index, write_data) in writes {
            let target_position = write_index as u64 + start_position;
            // If the target position is not the current position + 1, move forward
            if self.cursor.position() < target_position {
                self.cursor.seek(SeekFrom::Current((target_position - self.cursor.position()) as i64))?;
            }
            num_bytes_written += self.cursor.write(&[*write_data])?;
        }
        Ok(num_bytes_written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.cursor.flush()
    }
}

impl Read for ReversedWords<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // test alignment
        let mut misalignment = self.cursor.position() as usize % (self.word_size as usize);
        if misalignment > 0 {
            // back up by the amount of the misalignment
            self.seek(SeekFrom::Current(misalignment as i64 * -1))?;
        }
        let mut write_index = 0;
        loop {
            // Stop reading if we are at the end of the slice, or if the read buffer is full.
            if self.cursor.position() >= self.len || write_index >= buf.len() {
                return Ok(write_index);
            }

            match self.cursor.read_be::<u32>(){
                Ok(word) => {
                    let word = word.to_le_bytes();

                    for i in misalignment..word.len() {
                        if write_index >= buf.len() { // Exit if we would be writing past the end of the read buffer.
                            return Ok(write_index);
                        }
                        buf[write_index] = word[i];

                        if misalignment > 0 {
                            misalignment -= 1;
                        }

                        write_index += 1;
                    }
                },
                Err(e) => match e {
                    binread::Error::Io(e) => {return Err(e);}, // io errors pass through
                    e => {panic!("unexpected binrw error: {:?}", e)} // not expecting to hit any of these since we are simply reading a u32
                }
            }

        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn read_simple_sequential() {
        let mut data: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let mut ram = ReversedWords::new(&mut data);
        let mut out = vec![0u8; 8];
        let result = ram.read(&mut out).unwrap();
        assert_eq!(vec![3, 2, 1, 0, 7, 6, 5, 4], out);
        assert_eq!(data.len(), result);
    }

    #[test]
    fn read_seek_unaligned() {
        let mut data: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let mut ram = ReversedWords::new(&mut data);
        let mut out = vec![0u8; 3]; // just read 3 bytes
        ram.seek(SeekFrom::Start(2)).unwrap();
        let result = ram.read(&mut out).unwrap();
        assert_eq!(vec![1, 0, 7], out);
        assert_eq!(3, result);
    }

    #[test]
    fn write_simple_sequential() {
        let mut source: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let mut target = vec![0u8; 8];
        let mut ram = ReversedWords::new(&mut target);
        let result = ram.write(&mut source).unwrap();
        assert_eq!(vec![3, 2, 1, 0, 7, 6, 5, 4], target);
        assert_eq!(source.len(), result);
    }

    #[test]
    fn write_seek_unaligned() {
        let mut target = vec![0u8; 8];
        let mut ram = ReversedWords::new(&mut target);
        ram.seek(SeekFrom::Start(2)).unwrap();
        let result = ram.write(&[1, 2, 3, 4]).unwrap();
        assert_eq!(vec![2, 1, 0, 0, 0, 0, 4, 3], target);
        assert_eq!(4, result);
    }
    #[test]
    fn write_seek_unaligned_2() {
        let mut target = vec![0u8; 8];
        let mut ram = ReversedWords::new(&mut target);
        ram.seek(SeekFrom::Start(6)).unwrap();
        let result = ram.write(&[16, 32]).unwrap();
        assert_eq!(vec![0, 0, 0, 0, 32, 16, 0, 0], target);
        assert_eq!(2, result);
    }

    #[test]
    fn read_and_write_aligned_block() {
        let mut target = vec![0u8; 128];
        let source: Vec<u8> = (0..128).collect();
        let mut read_buffer = vec![];
        let mut ram = ReversedWords::new(&mut target);
        ram.write_all(&source).unwrap();
        ram.seek(SeekFrom::Start(0)).unwrap();

        ram.read_to_end(&mut read_buffer).unwrap();
        assert_eq!(source, read_buffer);
    }

    #[test]
    fn read_and_write_unaligned_blocks() {
        let mut target = vec![0u8; 128];
        let target_len = target.len();
        let source: Vec<u8> = (0..32).collect();
        let mut ram = ReversedWords::new(&mut target);
        ram.seek(SeekFrom::Start(2)).unwrap();
        ram.write_all(&source).unwrap();
        ram.seek(SeekFrom::Start(31)).unwrap();
        ram.write_all(&source).unwrap();
        ram.seek(SeekFrom::Start(65)).unwrap();
        ram.write_all(&source).unwrap();
        ram.seek(SeekFrom::End(-1)).unwrap();
        ram.write(&[255]).unwrap();

        let mut expected_result = vec![0u8, 0u8];
        expected_result.append(&mut source.clone());
        expected_result.truncate(31);
        expected_result.append(&mut source.clone());
        expected_result.append(&mut vec![0u8, 0u8]);
        expected_result.append(&mut source.clone());
        while expected_result.len() < target_len {
            expected_result.push(0u8);
        }
        expected_result[target_len - 1] = 255;

        let mut read_buffer = vec![];
        ram.seek(SeekFrom::Start(0)).unwrap();
        ram.read_to_end(&mut read_buffer).unwrap();
        assert_eq!(expected_result, read_buffer);
    }
    #[test]
    fn write_past_end_fails() {
        // todo: write
        assert!(true)
    }
}