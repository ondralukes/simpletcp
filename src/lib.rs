#[cfg(test)]
mod tests {
    use crate::simpletcp::{TcpServer, Error, TcpStream};
    use std::thread;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[test]
    fn can_create_server() {
        TcpServer::new("127.0.0.1:1234").expect("Failed to create server");
    }

    #[test]
    fn accept_none(){
        let server = TcpServer::new("127.0.0.1:1234").expect("Failed to create server");
        match server.accept().expect("Accept failed") {
            None => {},
            Some(_) => {
                panic!("Accept returned Some but None was expected");
            },
        }
    }
}

pub mod simpletcp{
    use std::net;
    use std::fmt;
    use std::io;
    use std::net::{ToSocketAddrs, SocketAddr};
    use std::fmt::Formatter;
    use std::io::ErrorKind;

    pub enum Error{
        NotReady,
        EncryptionError,
        TcpError(io::Error)
    }

    impl fmt::Debug for Error {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            return match self {
                Error::NotReady => {
                    f.write_str("Error::NotReady")
                },
                Error::EncryptionError => {
                    f.write_str("Error::EncryptionError")
                },
                Error::TcpError(io_err) => {
                    f.write_fmt(format_args!("Error::TcpError: {}", io_err))
                },
            }
        }
    }

    impl From<io::Error> for Error{
        fn from(io_err: io::Error) -> Self {
            Error::TcpError(io_err)
        }
    }

    pub struct TcpServer{
        socket: net::TcpListener,
    }

    impl TcpServer{
        pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, Error>{
            let socket = net::TcpListener::bind(addr)?;
            socket.set_nonblocking(true);
            return Ok(TcpServer { socket });
        }

        pub fn accept(&self) -> Result<Option<TcpStream>, Error>{
            match self.socket.accept() {
                Ok((socket, addr)) => {
                    Ok(Some(TcpStream::from_socket(socket)?))
                },
                Err(io_err) => {
                    match io_err.kind() {
                        ErrorKind::WouldBlock => {
                            Ok(None)
                        }
                        _ => {
                            Err(Error::TcpError(io_err))
                        }
                    }
                },
            }
        }
    }

    pub struct TcpStream {
        socket: net::TcpStream
    }

    impl TcpStream {
        fn from_socket(socket: net::TcpStream) -> Result<Self, Error>{
            socket.set_nonblocking(true)?;
            Ok(
                TcpStream{
                    socket
                }
            )
        }
    }
}