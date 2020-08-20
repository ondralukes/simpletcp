pub mod simpletcp {
    use std::convert::TryInto;
    use std::fmt;
    use std::fmt::Formatter;
    use std::io;
    use std::io::{ErrorKind, Read, Write};
    use std::net;
    use std::net::ToSocketAddrs;

    #[cfg(test)]
    mod tests;

    pub enum Error {
        NotReady,
        EncryptionError,
        TcpError(io::Error),
    }

    impl fmt::Debug for Error {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            return match self {
                Error::NotReady => f.write_str("Error::NotReady"),
                Error::EncryptionError => f.write_str("Error::EncryptionError"),
                Error::TcpError(io_err) => f.write_fmt(format_args!("Error::TcpError: {}", io_err)),
            };
        }
    }

    impl From<io::Error> for Error {
        fn from(io_err: io::Error) -> Self {
            Error::TcpError(io_err)
        }
    }

    pub struct TcpServer {
        socket: net::TcpListener,
    }

    impl TcpServer {
        pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
            let socket = net::TcpListener::bind(addr)?;
            socket.set_nonblocking(true)?;
            return Ok(Self { socket });
        }

        pub fn accept(&self) -> Result<Option<TcpStream>, Error> {
            match self.socket.accept() {
                Ok((socket, _addr)) => Ok(Some(TcpStream::from_socket(socket)?)),
                Err(io_err) => match io_err.kind() {
                    ErrorKind::WouldBlock => Ok(None),
                    _ => Err(Error::TcpError(io_err)),
                },
            }
        }
    }

    macro_rules! try_io {
        ($r: expr) => {
            match $r {
                Ok(res) => res,
                Err(e) => match e.kind() {
                    ErrorKind::WouldBlock => {
                        return Ok(None);
                    }
                    _ => {
                        return Err(Error::TcpError(e));
                    }
                },
            }
        };
    }

    pub struct TcpStream {
        socket: net::TcpStream,
        buffer: Vec<u8>,
    }

    impl TcpStream {
        fn from_socket(socket: net::TcpStream) -> Result<Self, Error> {
            socket.set_nonblocking(true)?;
            Ok(Self {
                socket,
                buffer: Vec::new(),
            })
        }

        pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
            let socket = net::TcpStream::connect(addr)?;
            socket.set_nonblocking(true)?;

            Ok(Self {
                socket,
                buffer: Vec::new(),
            })
        }

        #[allow(dead_code)]
        fn write_raw(&mut self, msg: &[u8]) -> Result<(), Error> {
            let length = msg.len() as u32;
            self.socket.write(&length.to_le_bytes())?;
            self.socket.write(msg)?;

            Ok(())
        }

        #[allow(dead_code)]
        fn read_raw(&mut self) -> Result<Option<Vec<u8>>, Error> {
            if self.buffer.len() < 4 {
                let start = self.buffer.len();
                self.buffer.resize(4, 0);
                let bytes_read = try_io!(self.socket.read(&mut self.buffer[start..]));

                self.buffer.resize(start + bytes_read, 0);
                if self.buffer.len() != 4 {
                    println!("Not full len");
                    return Ok(None);
                }
            }

            println!("buf: {:?}", self.buffer);
            let len = u32::from_le_bytes(self.buffer[..4].try_into().unwrap()) as usize;
            println!("len = {}", len);

            let start = self.buffer.len();
            self.buffer.resize(4 + len, 0);
            println!("bl = {}", self.buffer[start..].len());
            let bytes_read = self.socket.read(&mut self.buffer[start..])?;
            println!("s{} r {}", start, bytes_read);
            self.buffer.resize(start + bytes_read, 0);
            println!("buf: {:?}", self.buffer);
            if self.buffer.len() == len + 4 {
                println!("buf: {:?}", self.buffer);
                let result = self.buffer[4..].to_vec();
                self.buffer.clear();
                return Ok(Some(result));
            }

            Ok(None)
        }
    }
}
