use std::fmt::Write;

use atty::Stream;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_LENGTH};
use reqwest::{Request, Response};

use crate::utils::{colorize, get_content_type, indent_json, ContentType};
use crate::{Pretty, Theme};

pub struct Printer {
    indent_json: bool,
    color: bool,
    theme: Theme,
    sort_headers: bool,
}

impl Printer {
    pub fn new(pretty: Option<Pretty>, theme: Option<Theme>) -> Printer {
        let pretty = pretty.unwrap_or(if atty::is(Stream::Stdout) {
            Pretty::All
        } else {
            Pretty::None
        });
        let theme = theme.unwrap_or(Theme::Auto);

        match pretty {
            Pretty::All => Printer {
                indent_json: true,
                color: true,
                theme: theme,
                sort_headers: true,
            },
            Pretty::Colors => Printer {
                indent_json: false,
                color: true,
                theme: theme,
                sort_headers: false,
            },
            Pretty::Format => Printer {
                indent_json: true,
                color: false,
                theme: theme,
                sort_headers: true,
            },
            Pretty::None => Printer {
                indent_json: false,
                color: false,
                theme: theme,
                sort_headers: false,
            },
        }
    }

    fn print_json(&self, text: &str) {
        match (self.indent_json, self.color) {
            (true, true) => {
                colorize(&indent_json(text), "json", &self.theme)
                    .for_each(|line| print!("{}", line));
                print!("\x1b[0m");
            }
            (false, true) => {
                colorize(text, "json", &self.theme).for_each(|line| print!("{}", line));
                print!("\x1b[0m");
            }
            (true, false) => print!("{}", indent_json(text)),
            (false, false) => print!("{}", text),
        }
    }

    fn print_xml(&self, text: &str) {
        if self.color {
            colorize(text, "xml", &self.theme).for_each(|line| print!("{}", line));
            print!("\x1b[0m");
        } else {
            print!("{}", text)
        }
    }

    fn print_html(&self, text: &str) {
        if self.color {
            colorize(text, "html", &self.theme).for_each(|line| print!("{}", line));
            print!("\x1b[0m");
        } else {
            print!("{}", text)
        }
    }

    fn print_binary_suppressor(&self) {
        print!("+-----------------------------------------+\n");
        print!("| NOTE: binary data not shown in terminal |\n");
        print!("+-----------------------------------------+");
    }

    fn print_multipart_suppressor(&self) {
        print!("+--------------------------------------------+\n");
        print!("| NOTE: multipart data not shown in terminal |\n");
        print!("+--------------------------------------------+");
    }

    fn headers_to_string(&self, headers: &HeaderMap, sort: bool) -> String {
        let mut headers: Vec<(&HeaderName, &HeaderValue)> = headers.iter().collect();
        if sort {
            headers.sort_by(|(a, _), (b, _)| a.to_string().cmp(&b.to_string()))
        }

        let mut header_string = String::new();
        for (key, value) in headers {
            let key = key.to_string();
            let value = value.to_str().unwrap();
            writeln!(&mut header_string, "{}: {}", key, value).unwrap();
        }
        header_string.pop();

        header_string
    }

    pub fn print_request_headers(&self, request: &Request) {
        let method = request.method();
        let url = request.url();
        let query_string = url.query().map_or(String::from(""), |q| ["?", q].concat());
        let version = reqwest::Version::HTTP_11;
        let mut headers = request.headers().clone();

        // See https://github.com/seanmonstar/reqwest/issues/1030
        if let Some(body) = request.body().and_then(|body| body.as_bytes()) {
            let content_length = HeaderValue::from_str(&body.len().to_string()).unwrap();
            headers.insert(CONTENT_LENGTH, content_length);
        }

        let request_line = format!("{} {}{} {:?}\n", method, url.path(), query_string, version);
        let headers = &self.headers_to_string(&headers, self.sort_headers);

        if self.color {
            colorize(&(request_line + &headers), "http", &self.theme)
                .for_each(|line| print!("{}", line));
            print!("\x1b[0m");
        } else {
            print!("{}", &(request_line + &headers));
        }

        print!("\n\n");
    }

    pub fn print_response_headers(&self, response: &Response) {
        let version = response.version();
        let status = response.status();
        let headers = response.headers();

        let status_line = format!(
            "{:?} {} {}\n",
            version,
            status.as_str(),
            status.canonical_reason().unwrap()
        );
        let headers = self.headers_to_string(headers, self.sort_headers);

        if self.color {
            colorize(&(status_line + &headers), "http", &self.theme)
                .for_each(|line| print!("{}", line));
            print!("\x1b[0m");
        } else {
            print!("{}", &(status_line + &headers));
        }

        print!("\n\n");
    }

    pub fn print_request_body(&self, request: &Request) {
        let get_body = || {
            request
                .body()
                .and_then(|b| b.as_bytes())
                .and_then(|b| String::from_utf8(b.into()).ok())
        };

        match get_content_type(&request.headers()) {
            Some(ContentType::Multipart) => {
                self.print_multipart_suppressor();
                print!("\n\n");
            }
            Some(ContentType::Json) => {
                if let Some(body) = get_body() {
                    self.print_json(&body);
                    print!("\n\n");
                }
            }
            Some(ContentType::UrlencodedForm) | _ => {
                if let Some(body) = get_body() {
                    print!("{}", body);
                    print!("\n\n");
                }
            }
        };
    }

    pub async fn print_response_body(&self, response: Response) {
        match get_content_type(&response.headers()) {
            Some(ContentType::Json) => self.print_json(&response.text().await.unwrap()),
            Some(ContentType::Xml) => self.print_xml(&response.text().await.unwrap()),
            Some(ContentType::Html) => self.print_html(&response.text().await.unwrap()),
            _ => {
                let text = response.text().await.unwrap();
                if atty::is(Stream::Stdin) && text.contains('\0') {
                    self.print_binary_suppressor();
                } else {
                    print!("{}", text);
                }
            }
        };
    }
}
