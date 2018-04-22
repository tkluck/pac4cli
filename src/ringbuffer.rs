use std;
use std::cmp::min;

pub struct RingBuffer<T> {
    buf: Vec<T>,
    start: usize,
    length: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(buf: Vec<T>) -> Self {
        RingBuffer { buf, start: 0, length: 0 }
    }
    pub fn with_next_writeable_chunk<F>(&mut self, f: F) -> Result<(),std::io::Error>
        where F: FnOnce(&mut [T]) -> Result<usize,std::io::Error>
    {
        if self.length < self.buf.len() {
            let begin_chunk = (self.start + self.length) % self.buf.len();
            let end_chunk = if self.start > begin_chunk {
                self.start
            } else {
                self.buf.len()
            };
            if begin_chunk < end_chunk {
                let written = f(&mut self.buf[begin_chunk..end_chunk])?;
                self.length += written;
                if self.length > self.buf.len() {
                    panic!("Producing more than the entire ring buffer");
                }
            }
        }
        Ok(())
    }
    pub fn with_next_readable_chunk<F>(&mut self, f: F) -> Result<(),std::io::Error>
        where F: FnOnce(&[T]) -> Result<usize,std::io::Error>
    {
        let end_chunk = min(self.start + self.length, self.buf.len());
        if end_chunk > self.start {
            let read = f(&self.buf[self.start..end_chunk])?;
            if read > self.length {
                panic!("Consuming more than the entire ring buffer");
            }
            self.start = (self.start + read) % self.buf.len();
            self.length -= read;
        }
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.length
    }
}
