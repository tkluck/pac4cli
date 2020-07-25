use tokio::io;
use tokio::prelude::*;

#[derive(Debug,Clone)]
pub struct Preamble {
    pub method: String,
    pub uri: String,
    pub http_version: String,
    pub headers: Vec<String>,
}

impl Preamble {
    pub async fn write<IO: std::marker::Unpin + io::AsyncWrite>(self, io: &mut IO) -> io::Result<()> {
        io.write_all(self.method.as_bytes()).await?;
        io.write_all(b" ").await?;
        io.write_all(self.uri.as_bytes()).await?;
        io.write_all(b" ").await?;
        io.write_all(self.http_version.as_bytes()).await?;
        io.write_all(b"\r\n").await?;
        for header in self.headers {
            io.write_all(header.as_bytes()).await?;
            io.write_all(b"\r\n").await?;
        }
        io.write_all(b"\r\n").await?;
        return Ok(());
    }
}
