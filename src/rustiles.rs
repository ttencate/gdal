extern crate http;

use std::vec::Vec;
use std::io::net::ip::{SocketAddr, Ipv4Addr};
use std::io::Writer;
use http::server::{Config, Server, Request, ResponseWriter};
use http::server::request::AbsolutePath;
use http::status::NotFound;
use http::headers;


#[test]
fn test_nothing() {
    assert_eq!(1, 1);
}


#[deriving(Clone)]
struct TileServer;


impl Server for TileServer {
    fn get_config(&self) -> Config {
        Config {
            bind_address: SocketAddr {
                ip: Ipv4Addr(0, 0, 0, 0),
                port: 8001,
            },
        }
    }

    fn handle_request(&self, r: &Request, w: &mut ResponseWriter) {
        w.headers.content_type = Some(headers::content_type::MediaType {
            type_: ~"text",
            subtype: ~"html",
            parameters: Vec::new(),
        });

        match r.request_uri {
            AbsolutePath(ref url) => {
                if url == &~"/" {
                    w.write(index_html.as_bytes()).unwrap();
                    return;
                }
            },
            _ => {}
        };

        w.status = NotFound;
        w.write("Page not found :(\n".as_bytes()).unwrap();

    }
}


fn main() {
    TileServer.serve_forever();
}


static index_html: &'static str = "<!doctype html>\
<meta charset='utf-8'>\n\
Hello world!\n\
";
